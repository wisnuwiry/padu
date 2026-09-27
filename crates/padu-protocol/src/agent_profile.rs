//! Agent Profile wire model (PRD §6).
//!
//! Registry over all 14 providers Padu supports, seeded from
//! `ProviderKind::ALL` (P1-01). Kanban assignment and automation resolution
//! both read from here; `enabled` is the source of truth that replaces
//! `DaemonSettings.disabled_providers` (§6.3).
//!
//! Two representations of a provider identity coexist deliberately:
//! - On the wire, `agent_id` serializes as [`ProviderKind`] (serde
//!   camelCase, e.g. `"deepSeek"`), matching `disabled_providers`.
//! - In daemon SQLite (`agent_profiles.agent_id`), the stable lowercase
//!   [`ProviderKind::id`] slug (e.g. `"deepseek"`) is stored, so a serde
//!   rename never orphans seeded rows.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::ProviderKind;

/// Cost tier bucket (PRD §6.2). Serializes lowercase (`"low"`, `"medium"`,
/// `"high"`), matching the `agent_profiles.cost_tier` column.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum CostTier {
    Low,
    #[default]
    Medium,
    High,
}

impl CostTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl std::str::FromStr for CostTier {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            unknown => Err(format!("unknown cost tier {unknown:?}")),
        }
    }
}

/// Registry entry for one provider (PRD §6.2).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentProfile {
    pub agent_id: ProviderKind,
    pub role_tags: Vec<String>,
    pub cost_tier: CostTier,
    /// Lower runs first in "next available" / fallback chains. Seed order
    /// follows `ProviderKind::ALL` (the canonical probe order).
    pub priority: u32,
    pub max_retry_before_escalate: u32,
    pub enabled: bool,
    /// Optimistic-concurrency guard (P1-02). Bumped on every write; updates
    /// carry `expected_version` and fail when it no longer matches.
    pub version: u32,
}

impl AgentProfile {
    /// Display names always come from [`ProviderKind::display_name`]: the
    /// registry stores no name of its own, so renames propagate for free.
    pub fn display_name(&self) -> &'static str {
        self.agent_id.display_name()
    }
}

/// Full-replace profile update (P1-02), mirroring [`crate::notes::UpdateNote`].
/// `expected_version` must equal the stored [`AgentProfile::version`]; on
/// mismatch the daemon rejects with a version conflict and the client
/// re-fetches via `ListAgentProfiles`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentProfile {
    pub agent_id: ProviderKind,
    pub role_tags: Vec<String>,
    pub cost_tier: CostTier,
    pub priority: u32,
    pub max_retry_before_escalate: u32,
    pub enabled: bool,
    pub expected_version: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_serializes_with_camel_case_keys_and_lowercase_tier() {
        let profile = AgentProfile {
            agent_id: ProviderKind::DeepSeek,
            role_tags: vec!["docs".to_owned(), "test".to_owned()],
            cost_tier: CostTier::Low,
            priority: 5,
            max_retry_before_escalate: 3,
            enabled: true,
            version: 1,
        };
        let value = serde_json::to_value(&profile).unwrap();
        // `agent_id` follows the ProviderKind wire spelling shared with
        // `DaemonSettings.disabled_providers`, not the lowercase DB slug.
        assert_eq!(value["agentId"], serde_json::json!("deepSeek"));
        assert_eq!(value["roleTags"], serde_json::json!(["docs", "test"]));
        assert_eq!(value["costTier"], serde_json::json!("low"));
        assert_eq!(value["maxRetryBeforeEscalate"], serde_json::json!(3));
        let round_tripped: AgentProfile = serde_json::from_value(value).unwrap();
        assert_eq!(round_tripped, profile);
    }

    #[test]
    fn cost_tier_parses_the_seeded_spellings() {
        use std::str::FromStr as _;
        assert_eq!(CostTier::from_str("low").unwrap(), CostTier::Low);
        assert_eq!(CostTier::from_str("medium").unwrap(), CostTier::Medium);
        assert_eq!(CostTier::from_str("high").unwrap(), CostTier::High);
        assert!(CostTier::from_str("ultra").is_err());
    }

    #[test]
    fn display_name_delegates_to_provider_kind() {
        let profile = AgentProfile {
            agent_id: ProviderKind::Claude,
            role_tags: Vec::new(),
            cost_tier: CostTier::High,
            priority: 2,
            max_retry_before_escalate: 3,
            enabled: true,
            version: 1,
        };
        assert_eq!(profile.display_name(), ProviderKind::Claude.display_name());
    }
}
