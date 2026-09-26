//! GitHub/GitLab integration wire shells (PRD §8).
//!
//! Phase 0 scaffolding only: reserves the `git_integration` module (named to
//! avoid clashing with the existing local-Git `git` module) and its codegen
//! roots so Phase 2/3 can flesh out `linked_repo`, connection status, and
//! issue/PR handles. Not referenced from `Command` / `ServerMessage` yet, so
//! there is no behavior change.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Skeleton for `Project.linked_repo` (PRD §5.4 extension).
///
/// Fleshed out in Phase 2 with `provider`, `owner`, and `repo`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LinkedRepo {}
