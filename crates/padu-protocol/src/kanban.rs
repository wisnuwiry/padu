//! Kanban board wire model (PRD §5).
//!
//! Fleshed out in Phase 1 (P1-04): a `Task` is the unit of work, an
//! `AgentSession` is one execution attempt. A task links to at most one
//! active session (`session_id: None` while in Backlog). Not referenced from
//! `Command` / `ServerMessage` yet — commands land in P1-05.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::git_integration::{LinkedIssue, LinkedPr};
use crate::model::{ProviderKind, SessionWorkspace};

/// Board column (PRD §5.2). Snake-case spelling matches the
/// `tasks.status` column (`backlog | queued | running | review | done`).
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Backlog,
    Queued,
    Running,
    Review,
    Done,
}

/// Workspace kind snapshot taken at queue time (PRD §5.4). Mirrors
/// [`SessionWorkspace`] without its paths: the live workspace stays on the
/// session, the task keeps only how it was queued.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TaskWorkspaceKind {
    #[default]
    Local,
    NewWorktree,
    Worktree,
}

impl TaskWorkspaceKind {
    pub fn from_session_workspace(workspace: &SessionWorkspace) -> Self {
        match workspace {
            SessionWorkspace::Local => Self::Local,
            SessionWorkspace::NewWorktree { .. } => Self::NewWorktree,
            SessionWorkspace::Worktree { .. } => Self::Worktree,
        }
    }
}

/// Kanban card (PRD §5.4). `assigned_agent` references
/// `AgentProfile.agent_id`, never free text.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub labels: Vec<String>,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_agent: Option<ProviderKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_kind: Option<TaskWorkspaceKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_issue: Option<LinkedIssue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_pr: Option<LinkedPr>,
    pub needs_attention: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_failed: Option<String>,
    #[serde(default)]
    pub idempotency_keys: Vec<String>,
    /// Optimistic-concurrency guard, checked on every update (P1-05).
    pub version: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub archived: bool,
}

/// Board-card projection for `ListTasks` (P1-05), mirroring `NoteSummary`:
/// everything a card renders, none of the heavy detail. Full rows,
/// checkpoint bodies, and logs arrive via `HydrateTask`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description_preview: String,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_agent: Option<ProviderKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<Uuid>,
    #[serde(default)]
    pub labels: Vec<String>,
    pub needs_attention: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_failed: Option<String>,
    pub updated_at: u64,
    pub version: u64,
    pub archived: bool,
}

impl TaskSummary {
    pub fn from_task(task: &Task) -> Self {
        Self {
            id: task.id,
            project_id: task.project_id,
            title: task.title.clone(),
            description_preview: task_preview(&task.description),
            status: task.status,
            assigned_agent: task.assigned_agent,
            session_id: task.session_id,
            labels: task.labels.clone(),
            needs_attention: task.needs_attention,
            sync_failed: task.sync_failed.clone(),
            updated_at: task.updated_at,
            version: task.version,
            archived: task.archived,
        }
    }
}

/// Whitespace-normalized 160-char preview, mirroring `note_preview` in
/// `padu-core` (which cannot be shared: this crate has no daemon dependency).
fn task_preview(description: &str) -> String {
    const PREVIEW_CHARS: usize = 160;
    let normalized = description.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= PREVIEW_CHARS {
        normalized
    } else {
        format!(
            "{}…",
            normalized
                .chars()
                .take(PREVIEW_CHARS - 1)
                .collect::<String>()
        )
    }
}

/// `CreateTask` input (P1-05). The daemon assigns id, backlog status,
/// version 1, and timestamps; everything else defaults to empty/unset.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CreateTask {
    pub project_id: Uuid,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_agent: Option<ProviderKind>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_integration::{GitProvider, PrState};

    fn task() -> Task {
        Task {
            id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            title: "Fix the parser".to_owned(),
            description: "details".to_owned(),
            labels: vec!["bug".to_owned()],
            status: TaskStatus::Queued,
            assigned_agent: Some(ProviderKind::Codex),
            session_id: Some(Uuid::new_v4()),
            workspace_kind: Some(TaskWorkspaceKind::NewWorktree),
            linked_issue: Some(LinkedIssue {
                provider: GitProvider::GitHub,
                id: "7".to_owned(),
                url: "https://github.com/example/repo/issues/7".to_owned(),
            }),
            linked_pr: None,
            needs_attention: false,
            sync_failed: None,
            idempotency_keys: vec!["key-one".to_owned()],
            version: 1,
            created_at: 100,
            updated_at: 200,
            archived: false,
        }
    }

    #[test]
    fn task_serializes_with_camel_case_keys_and_snake_case_status() {
        let original = task();
        let value = serde_json::to_value(&original).unwrap();
        assert_eq!(value["status"], serde_json::json!("queued"));
        assert_eq!(value["assignedAgent"], serde_json::json!("codex"));
        assert_eq!(value["needsAttention"], serde_json::json!(false));
        assert_eq!(value["workspaceKind"], serde_json::json!("new_worktree"));
        assert_eq!(value["idempotencyKeys"], serde_json::json!(["key-one"]));
        assert!(value.get("linkedPr").is_none(), "None fields are skipped");
        assert!(value.get("syncFailed").is_none(), "None fields are skipped");
        let round_tripped: Task = serde_json::from_value(value).unwrap();
        assert_eq!(round_tripped, original);
    }

    #[test]
    fn workspace_kind_mirrors_session_workspace() {
        assert_eq!(
            TaskWorkspaceKind::from_session_workspace(&SessionWorkspace::Local),
            TaskWorkspaceKind::Local
        );
        assert_eq!(
            TaskWorkspaceKind::from_session_workspace(&SessionWorkspace::NewWorktree {
                base_branch: None
            }),
            TaskWorkspaceKind::NewWorktree
        );
        assert_eq!(
            TaskWorkspaceKind::from_session_workspace(&SessionWorkspace::Worktree {
                path: std::path::PathBuf::from("/tmp/wt"),
                branch: "padu/x".to_owned(),
            }),
            TaskWorkspaceKind::Worktree
        );
    }

    #[test]
    fn linked_pr_state_spelling_matches_prd() {
        let state: PrState = serde_json::from_value(serde_json::json!("merged")).unwrap();
        assert_eq!(state, PrState::Merged);
    }

    #[test]
    fn summary_projection_hides_detail_but_keeps_card_fields() {
        let mut full = task();
        full.description = "word ".repeat(100);
        let summary = TaskSummary::from_task(&full);
        assert_eq!(summary.id, full.id);
        assert_eq!(summary.status, TaskStatus::Queued);
        assert_eq!(summary.version, full.version);
        assert!(
            summary.description_preview.chars().count() <= 160,
            "preview truncates long descriptions"
        );
        assert!(summary.description_preview.ends_with('…'));
        let value = serde_json::to_value(&summary).unwrap();
        assert!(value.get("idempotencyKeys").is_none());
        assert!(value.get("description").is_none());
        let round_tripped: TaskSummary = serde_json::from_value(value).unwrap();
        assert_eq!(round_tripped, summary);
    }

    #[test]
    fn short_descriptions_preview_verbatim() {
        let summary = TaskSummary::from_task(&task());
        assert_eq!(summary.description_preview, "details");
    }
}
