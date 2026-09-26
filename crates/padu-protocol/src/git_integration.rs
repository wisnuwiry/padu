//! GitHub/GitLab integration wire types (PRD §5.4 handles, §8).
//!
//! Fleshed out in Phase 1 (P1-04) with the remote handles the Kanban `Task`
//! carries (`linked_issue`, `linked_pr`) and the `Project.linked_repo`
//! extension. Connection status, auth, and sync behavior land in Phase 2/3.
//! Not referenced from `Command` / `ServerMessage` yet.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Remote forge provider. Explicit renames (not `snake_case`, which would
/// give `git_hub`): the wire and DB spelling is `github` / `gitlab`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
pub enum GitProvider {
    #[default]
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "gitlab")]
    GitLab,
}

/// Remote issue handle (PRD §5.4). Local-only Padu backlink metadata lives
/// beside it; the remote body is never edited for bookkeeping.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LinkedIssue {
    pub provider: GitProvider,
    pub id: String,
    pub url: String,
}

/// Remote PR/MR state (PRD §5.4).
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PrState {
    #[default]
    Open,
    Draft,
    Merged,
    Closed,
}

/// Remote PR/MR handle (PRD §5.4).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LinkedPr {
    pub provider: GitProvider,
    pub id: String,
    pub url: String,
    pub state: PrState,
}

/// Repository a project is connected to (PRD §5.4 `Project.linked_repo`
/// extension). Connection health and auth live here in Phase 2.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LinkedRepo {
    pub provider: GitProvider,
    pub owner: String,
    pub repo: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_spells_github_and_gitlab() {
        assert_eq!(
            serde_json::to_value(GitProvider::GitHub).unwrap(),
            serde_json::json!("github")
        );
        assert_eq!(
            serde_json::to_value(GitProvider::GitLab).unwrap(),
            serde_json::json!("gitlab")
        );
    }

    #[test]
    fn pr_handle_round_trips_with_snake_case_state() {
        let handle = LinkedPr {
            provider: GitProvider::GitHub,
            id: "42".to_owned(),
            url: "https://github.com/example/repo/pull/42".to_owned(),
            state: PrState::Draft,
        };
        let value = serde_json::to_value(&handle).unwrap();
        assert_eq!(value["provider"], serde_json::json!("github"));
        assert_eq!(value["state"], serde_json::json!("draft"));
        let round_tripped: LinkedPr = serde_json::from_value(value).unwrap();
        assert_eq!(round_tripped, handle);
    }

    #[test]
    fn linked_repo_round_trips() {
        let repo = LinkedRepo {
            provider: GitProvider::GitLab,
            owner: "example".to_owned(),
            repo: "padu".to_owned(),
        };
        let value = serde_json::to_value(&repo).unwrap();
        assert_eq!(value["owner"], serde_json::json!("example"));
        let round_tripped: LinkedRepo = serde_json::from_value(value).unwrap();
        assert_eq!(round_tripped, repo);
    }
}
