//! Idempotency keys for automation actions (P0-04, PRD §7.7).
//!
//! Every trigger event carries a stable `event_id` (see the `ServerMessage`
//! catalog in `padu-protocol`). Deriving the action key as
//! `idempotency_key = hash(rule_id, trigger_event_id)` means retries and
//! daemon-restart replays of the same trigger map to the same key, so the
//! unique index on `automation_outbox.idempotency_key` turns a duplicate
//! into a no-op instead of a duplicate remote object.
//!
//! Phase 0 provides the derivation plus the read-side dedup check. Claiming
//! outbox rows and executing actions land with the engine in Phase 4.

use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Derive the idempotency key for one rule firing on one trigger event.
///
/// SHA-256 over the raw bytes of both ids, hex-encoded. Deterministic
/// across restarts and processes; 256 bits so accidental collisions are
/// not a practical concern (unlike the FNV fingerprints used elsewhere
/// for change detection, which must never back correctness).
pub fn idempotency_key(rule_id: &Uuid, trigger_event_id: &Uuid) -> String {
    let mut hasher = Sha256::new();
    hasher.update(rule_id.as_bytes());
    hasher.update(trigger_event_id.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// True when `key` already has an outbox row, i.e. the action was claimed
/// before and must not run again.
pub fn is_duplicate(connection: &Connection, key: &str) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM automation_outbox WHERE idempotency_key = ?1)",
        params![key],
        |row| row.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::apply_migrations;

    #[test]
    fn derivation_is_deterministic_hex() {
        let rule = Uuid::from_u128(0x1111);
        let event = Uuid::from_u128(0x2222);

        let first = idempotency_key(&rule, &event);
        assert_eq!(first, idempotency_key(&rule, &event));
        assert_eq!(first.len(), 64, "256-bit hex digest");
        assert!(
            first.chars().all(|c| c.is_ascii_hexdigit()),
            "hex only: {first}"
        );
    }

    #[test]
    fn distinct_inputs_give_distinct_keys() {
        let rule_a = Uuid::from_u128(1);
        let rule_b = Uuid::from_u128(2);
        let event = Uuid::from_u128(3);

        assert_ne!(
            idempotency_key(&rule_a, &event),
            idempotency_key(&rule_b, &event),
            "different rules never share a key"
        );
        assert_ne!(
            idempotency_key(&rule_a, &event),
            idempotency_key(&rule_a, &rule_b),
            "different trigger events never share a key"
        );
        assert_ne!(
            idempotency_key(&rule_a, &rule_b),
            idempotency_key(&rule_b, &rule_a),
            "argument order matters"
        );
    }

    #[test]
    fn outbox_rows_back_the_dedup_check() {
        let connection = Connection::open_in_memory().unwrap();
        apply_migrations(&connection).unwrap();

        let key = idempotency_key(&Uuid::from_u128(7), &Uuid::from_u128(9));
        assert!(!is_duplicate(&connection, &key).unwrap());

        connection
            .execute(
                "INSERT INTO automation_outbox(
                    id, rule_id, idempotency_key, action, status,
                    attempts, created_at, updated_at
                 ) VALUES(?1, ?2, ?3, '[]', 'pending', 0, 0, 0)",
                params![
                    Uuid::new_v4().to_string(),
                    Uuid::from_u128(7).to_string(),
                    key,
                ],
            )
            .unwrap();
        assert!(is_duplicate(&connection, &key).unwrap());

        // A fresh trigger event is a fresh key, unaffected by the claim.
        let other = idempotency_key(&Uuid::from_u128(7), &Uuid::from_u128(10));
        assert!(!is_duplicate(&connection, &other).unwrap());
    }
}
