//! Agent Profile wire shells (PRD §6).
//!
//! Phase 0 scaffolding only: reserves the `agent_profile` module and its
//! codegen roots so Phase 1 (P1-01..P1-03) can flesh out the registry seeded
//! from `ProviderKind::ALL`. Not referenced from `Command` / `ServerMessage`
//! yet, so there is no behavior change.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Skeleton for the `AgentProfile` registry entry (PRD §6.2).
///
/// Fleshed out in Phase 1 with `agent_id`, `role_tags`, `cost_tier`,
/// `priority`, `max_retry_before_escalate`, and `enabled`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentProfile {}
