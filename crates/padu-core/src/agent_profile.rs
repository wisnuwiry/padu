//! Agent Profile registry seed (PRD §6, P1-01).
//!
//! On daemon startup, [`seed_agent_profiles`] inserts one row per
//! [`ProviderKind::ALL`] into `agent_profiles` with curated defaults below.
//! The insert is `ON CONFLICT DO NOTHING`, so user edits (priority reorder,
//! toggles, tag changes) persist across restarts untouched.
//!
//! The legacy `DaemonSettings.disabled_providers` import applies only to
//! rows inserted by the current seed run: a provider listed there seeds
//! with `enabled = false`, exactly once. Re-enabling afterwards sticks,
//! because the row already exists on the next run.
//!
//! `enabled` otherwise defaults to `true` for every provider. Per §6.2,
//! "when CLI probed present" governs runtime candidacy (P1-02), not the
//! stored default: probing 14 CLIs at startup would stall every launch,
//! and an absent CLI today must not destroy the user's stored toggle.

use std::io;
use std::str::FromStr as _;

use rusqlite::{Connection, params};

use crate::model::ProviderKind;
use padu_protocol::agent_profile::{AgentProfile, CostTier};

/// Curated seed defaults for one provider: role tags from provider
/// capability (frontier coding flagships get the full fix/refactor set,
/// fast budget harnesses get docs/test/fix), cost tier from catalog
/// knowledge. Editable after seeding; the seed never overwrites.
pub struct SeedDefaults {
    pub role_tags: &'static [&'static str],
    pub cost_tier: CostTier,
}

/// Seed defaults per provider (PRD §6.2).
pub fn seed_defaults(provider: ProviderKind) -> SeedDefaults {
    match provider {
        // Frontier coding flagships: full fix/refactor/review/plan/test set.
        ProviderKind::Claude | ProviderKind::Codex | ProviderKind::Cursor => SeedDefaults {
            role_tags: &["fix", "refactor", "review", "plan", "test"],
            cost_tier: CostTier::High,
        },
        // Full agentic CLIs and routers, mid cost. OpenCode routes across
        // models, so it gets the broadest tag set.
        ProviderKind::OpenCode => SeedDefaults {
            role_tags: &["fix", "refactor", "docs", "test", "review", "plan"],
            cost_tier: CostTier::Medium,
        },
        ProviderKind::Agy | ProviderKind::Amp | ProviderKind::CommandCode | ProviderKind::Qoder => {
            SeedDefaults {
                role_tags: &["fix", "refactor", "test", "review"],
                cost_tier: CostTier::Medium,
            }
        }
        // Lightweight terminal agents, mid cost.
        ProviderKind::Pi | ProviderKind::OhMyPi => SeedDefaults {
            role_tags: &["fix", "test", "docs"],
            cost_tier: CostTier::Medium,
        },
        // Fast budget harnesses: docs/test/fix.
        ProviderKind::DeepSeek | ProviderKind::Fx | ProviderKind::Grok | ProviderKind::Kimi => {
            SeedDefaults {
                role_tags: &["docs", "test", "fix"],
                cost_tier: CostTier::Low,
            }
        }
    }
}

/// Default `max_retry_before_escalate` for every seeded profile (PRD §6.2).
pub const DEFAULT_MAX_RETRY_BEFORE_ESCALATE: u32 = 3;

/// Seeds the registry; returns how many rows were inserted (0 on a warm
/// restart where every provider already has a row).
pub fn seed_agent_profiles(
    connection: &Connection,
    disabled_providers: &[ProviderKind],
) -> io::Result<usize> {
    let transaction = connection
        .unchecked_transaction()
        .map_err(crate::persistence::to_io_error)?;
    let mut inserted = 0;
    for (priority, provider) in ProviderKind::ALL.into_iter().enumerate() {
        let defaults = seed_defaults(provider);
        let role_tags = serde_json::to_string(&defaults.role_tags)
            .map_err(|error| io::Error::other(format!("could not encode role tags: {error}")))?;
        let enabled = !disabled_providers.contains(&provider);
        let changed = transaction
            .execute(
                "INSERT INTO agent_profiles(agent_id, role_tags, cost_tier, priority, \
                 max_retry_before_escalate, enabled, version) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1) \
                 ON CONFLICT(agent_id) DO NOTHING",
                params![
                    provider.id(),
                    role_tags,
                    defaults.cost_tier.as_str(),
                    priority as u32,
                    DEFAULT_MAX_RETRY_BEFORE_ESCALATE,
                    enabled,
                ],
            )
            .map_err(crate::persistence::to_io_error)?;
        inserted += changed;
    }
    transaction
        .commit()
        .map_err(crate::persistence::to_io_error)?;
    Ok(inserted)
}

/// Reads the registry ordered by priority (lowest first).
pub fn list_agent_profiles(connection: &Connection) -> io::Result<Vec<AgentProfile>> {
    let mut statement = connection
        .prepare(
            "SELECT agent_id, role_tags, cost_tier, priority, max_retry_before_escalate, enabled, version \
             FROM agent_profiles ORDER BY priority ASC",
        )
        .map_err(crate::persistence::to_io_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u32>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, bool>(5)?,
                row.get::<_, u32>(6)?,
            ))
        })
        .map_err(crate::persistence::to_io_error)?;
    let mut profiles = Vec::new();
    for row in rows {
        profiles.push(parse_profile(
            row.map_err(crate::persistence::to_io_error)?,
        )?);
    }
    Ok(profiles)
}

/// Reads one profile by provider. `None` means never seeded.
pub fn get_agent_profile(
    connection: &Connection,
    provider: ProviderKind,
) -> io::Result<Option<AgentProfile>> {
    use rusqlite::OptionalExtension as _;
    connection
        .query_row(
            "SELECT agent_id, role_tags, cost_tier, priority, max_retry_before_escalate, enabled, version \
             FROM agent_profiles WHERE agent_id = ?1",
            params![provider.id()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u32>(3)?,
                    row.get::<_, u32>(4)?,
                    row.get::<_, bool>(5)?,
                    row.get::<_, u32>(6)?,
                ))
            },
        )
        .optional()
        .map_err(crate::persistence::to_io_error)?
        .map(parse_profile)
        .transpose()
}

/// Candidacy check (P1-02): a missing row (pre-seed database) reads as
/// enabled, preserving pre-P1-02 behavior until the startup seed runs.
pub fn agent_profile_enabled(connection: &Connection, provider: ProviderKind) -> io::Result<bool> {
    Ok(get_agent_profile(connection, provider)?.is_none_or(|profile| profile.enabled))
}

type ProfileRow = (String, String, String, u32, u32, bool, u32);

fn parse_profile(
    (agent_id, role_tags, cost_tier, priority, max_retry_before_escalate, enabled, version): ProfileRow,
) -> io::Result<AgentProfile> {
    let provider = ProviderKind::ALL
        .into_iter()
        .find(|provider| provider.id() == agent_id)
        .ok_or_else(|| io::Error::other(format!("unknown agent_id {agent_id:?}")))?;
    let role_tags: Vec<String> = serde_json::from_str(&role_tags)
        .map_err(|error| io::Error::other(format!("bad role tags for {agent_id:?}: {error}")))?;
    let cost_tier = CostTier::from_str(&cost_tier)
        .map_err(|error| io::Error::other(format!("bad cost tier for {agent_id:?}: {error}")))?;
    Ok(AgentProfile {
        agent_id: provider,
        role_tags,
        cost_tier,
        priority,
        max_retry_before_escalate,
        enabled,
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::apply_migrations;

    fn migrated_memory() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        apply_migrations(&connection).unwrap();
        connection
    }

    #[test]
    fn fresh_database_seeds_all_fourteen_profiles() {
        let connection = migrated_memory();
        assert_eq!(seed_agent_profiles(&connection, &[]).unwrap(), 14);

        let profiles = list_agent_profiles(&connection).unwrap();
        assert_eq!(profiles.len(), ProviderKind::ALL.len());

        let ids: Vec<&str> = profiles
            .iter()
            .map(|profile| profile.agent_id.id())
            .collect();
        let expected: Vec<&str> = ProviderKind::ALL
            .iter()
            .map(|provider| provider.id())
            .collect();
        assert_eq!(ids, expected);

        for profile in &profiles {
            assert!(
                profile.enabled,
                "{:?} should seed enabled",
                profile.agent_id
            );
            assert_eq!(
                profile.version, 1,
                "{:?} should seed version 1",
                profile.agent_id
            );
            assert_eq!(
                profile.max_retry_before_escalate, DEFAULT_MAX_RETRY_BEFORE_ESCALATE,
                "{:?} should seed max_retry_before_escalate = 3",
                profile.agent_id
            );
            assert!(
                !profile.role_tags.is_empty(),
                "{:?} needs role tags",
                profile.agent_id
            );
            assert_eq!(
                profile.display_name(),
                profile.agent_id.display_name(),
                "display names must come from ProviderKind::display_name()"
            );
        }
        // Priority follows probe order: unique and ascending in ALL order.
        let priorities: Vec<u32> = profiles.iter().map(|profile| profile.priority).collect();
        let mut sorted = priorities.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(priorities.len(), sorted.len(), "priorities must be unique");
        assert!(
            priorities.windows(2).all(|pair| pair[0] < pair[1]),
            "profiles list in priority order"
        );
    }

    #[test]
    fn seed_defaults_cover_every_provider() {
        for provider in ProviderKind::ALL {
            let defaults = seed_defaults(provider);
            assert!(
                !defaults.role_tags.is_empty(),
                "{provider:?} needs role tags"
            );
            for tag in defaults.role_tags {
                assert!(
                    ["fix", "refactor", "docs", "test", "review", "plan"].contains(tag),
                    "{provider:?} has an unknown role tag {tag:?}"
                );
            }
        }
    }

    #[test]
    fn legacy_disabled_providers_import_as_disabled_exactly_once() {
        let connection = migrated_memory();
        let disabled = [ProviderKind::Claude, ProviderKind::Kimi];
        assert_eq!(seed_agent_profiles(&connection, &disabled).unwrap(), 2 + 12);

        let profiles = list_agent_profiles(&connection).unwrap();
        for profile in &profiles {
            assert_eq!(
                profile.enabled,
                !disabled.contains(&profile.agent_id),
                "{:?} enabled mismatch after legacy import",
                profile.agent_id
            );
        }

        // The user re-enables Claude and reorders it; the next restart (same
        // legacy settings on disk) must not undo either edit.
        connection
            .execute(
                "UPDATE agent_profiles SET enabled = 1, priority = 0 WHERE agent_id = 'claude'",
                [],
            )
            .unwrap();
        assert_eq!(seed_agent_profiles(&connection, &disabled).unwrap(), 0);

        let profiles = list_agent_profiles(&connection).unwrap();
        let claude = profiles
            .iter()
            .find(|profile| profile.agent_id == ProviderKind::Claude)
            .unwrap();
        assert!(claude.enabled, "re-enable must survive a reseed");
        assert_eq!(claude.priority, 0, "priority edit must survive a reseed");
        let kimi = profiles
            .iter()
            .find(|profile| profile.agent_id == ProviderKind::Kimi)
            .unwrap();
        assert!(!kimi.enabled, "untouched import stays disabled");
    }

    #[test]
    fn reseed_never_overwrites_user_edits() {
        let connection = migrated_memory();
        seed_agent_profiles(&connection, &[]).unwrap();
        connection
            .execute(
                "UPDATE agent_profiles SET role_tags = '[\"plan\"]', cost_tier = 'high', \
                 max_retry_before_escalate = 7, enabled = 0 WHERE agent_id = 'pi'",
                [],
            )
            .unwrap();

        assert_eq!(seed_agent_profiles(&connection, &[]).unwrap(), 0);
        let profiles = list_agent_profiles(&connection).unwrap();
        let pi = profiles
            .iter()
            .find(|profile| profile.agent_id == ProviderKind::Pi)
            .unwrap();
        assert_eq!(pi.role_tags, ["plan"]);
        assert_eq!(pi.cost_tier, CostTier::High);
        assert_eq!(pi.max_retry_before_escalate, 7);
        assert!(!pi.enabled);
    }
}
