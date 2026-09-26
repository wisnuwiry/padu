//! Automation rule-engine wire shells (PRD §7).
//!
//! Phase 0 scaffolding only: reserves the `automation` module and its codegen
//! roots so Phase 4 (P4-01..P4-05) can flesh out `Rule`, triggers, actions,
//! and approvals. Not referenced from `Command` / `ServerMessage` yet, so
//! there is no behavior change.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Skeleton for an automation `Rule` (PRD §7.2).
///
/// Fleshed out in Phase 4 with `trigger`, `condition`, `action`,
/// `requires_approval`, `cooldown_secs`, and chain-depth guards.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AutomationRule {}
