//! Kanban board wire shells (PRD §5).
//!
//! Phase 0 scaffolding only: reserves the `kanban` module and its codegen
//! roots so later phases (P1-04..P1-06) can flesh out the full `Task` entity
//! without re-wiring `export_types`. Not referenced from `Command` /
//! `ServerMessage` yet, so there is no behavior change.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Skeleton for the Kanban `Task` entity (PRD §5.4).
///
/// Fleshed out in Phase 1 with `project_id`, `status`, `session_id`,
/// `linked_issue`/`linked_pr`, `version`, flags, and timestamps.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Task {}
