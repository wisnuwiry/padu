//! Append-only audit trail for automation (P0-04, PRD §7.7).
//!
//! Every rule evaluation and execution appends one row to `audit_log`,
//! user-visible per rule and per card from Phase 4 on. The trail is
//! append-only by construction: this module exposes inserts and reads,
//! never updates or deletes. Callers must [`crate::redact`] any secrets
//! out of `detail` before appending — the writer stores exactly what it
//! is given, and tests below prove redacted input stays redacted at rest.

use rusqlite::{Connection, params};
use uuid::Uuid;

/// One audit row, as read back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditEntry {
    pub id: i64,
    pub rule_id: Option<String>,
    pub task_id: Option<String>,
    pub event: String,
    pub detail: Option<String>,
    pub created_at: i64,
}

/// Append one entry; returns the `AUTOINCREMENT` row id.
pub fn append_audit(
    connection: &Connection,
    rule_id: Option<&Uuid>,
    task_id: Option<&Uuid>,
    event: &str,
    detail: Option<&str>,
) -> rusqlite::Result<i64> {
    connection.execute(
        "INSERT INTO audit_log(rule_id, task_id, event, detail, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5)",
        params![
            rule_id.map(Uuid::to_string),
            task_id.map(Uuid::to_string),
            event,
            detail,
            crate::model::unix_time() as i64,
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

fn entry_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuditEntry> {
    Ok(AuditEntry {
        id: row.get("id")?,
        rule_id: row.get("rule_id")?,
        task_id: row.get("task_id")?,
        event: row.get("event")?,
        detail: row.get("detail")?,
        created_at: row.get("created_at")?,
    })
}

/// Latest entries first, optionally scoped to one rule and/or one card.
pub fn list_audit(
    connection: &Connection,
    rule_id: Option<&Uuid>,
    task_id: Option<&Uuid>,
    limit: usize,
) -> rusqlite::Result<Vec<AuditEntry>> {
    let mut statement = connection.prepare(
        "SELECT id, rule_id, task_id, event, detail, created_at FROM audit_log
         WHERE (?1 IS NULL OR rule_id = ?1)
           AND (?2 IS NULL OR task_id = ?2)
         ORDER BY id DESC LIMIT ?3",
    )?;
    let rows = statement.query_map(
        params![
            rule_id.map(Uuid::to_string),
            task_id.map(Uuid::to_string),
            limit as i64,
        ],
        entry_from_row,
    )?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::apply_migrations;
    use crate::redact::Redactor;

    fn migrated() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        apply_migrations(&connection).unwrap();
        connection
    }

    #[test]
    fn entries_append_with_monotonic_ids() {
        let connection = migrated();
        let rule = Uuid::from_u128(1);
        let task = Uuid::from_u128(2);

        let first = append_audit(
            &connection,
            Some(&rule),
            Some(&task),
            "rule_triggered",
            Some("{}"),
        )
        .unwrap();
        let second = append_audit(&connection, Some(&rule), None, "action_executed", None).unwrap();
        assert!(second > first, "ids increase in append order");

        let entries = list_audit(&connection, None, None, 10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, second, "latest first");
        assert_eq!(entries[0].event, "action_executed");
        assert_eq!(entries[0].task_id, None);
        let task_id = task.to_string();
        assert_eq!(entries[1].task_id.as_deref(), Some(task_id.as_str()));
    }

    #[test]
    fn listing_scopes_by_rule_and_task() {
        let connection = migrated();
        let rule_a = Uuid::from_u128(1);
        let rule_b = Uuid::from_u128(2);
        let task = Uuid::from_u128(3);

        append_audit(
            &connection,
            Some(&rule_a),
            Some(&task),
            "rule_triggered",
            None,
        )
        .unwrap();
        append_audit(
            &connection,
            Some(&rule_b),
            Some(&task),
            "rule_triggered",
            None,
        )
        .unwrap();
        append_audit(&connection, Some(&rule_a), None, "action_executed", None).unwrap();

        assert_eq!(
            list_audit(&connection, Some(&rule_a), None, 10)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            list_audit(&connection, None, Some(&task), 10)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            list_audit(&connection, Some(&rule_b), Some(&task), 10)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(list_audit(&connection, None, None, 2).unwrap().len(), 2);
    }

    #[test]
    fn redacted_detail_stays_redacted_at_rest() {
        let connection = migrated();
        let secret = "ghp_example_secret_token_abc123";
        let redactor = Redactor::new([secret]);
        let detail = redactor.redact(&format!("created with token {secret} done"));

        append_audit(&connection, None, None, "issue_created", Some(&detail)).unwrap();

        let entries = list_audit(&connection, None, None, 10).unwrap();
        let stored = entries[0].detail.as_deref().unwrap();
        assert!(!stored.contains(secret), "no secret in the database");
        assert!(stored.contains(crate::redact::REDACTED));
    }
}
