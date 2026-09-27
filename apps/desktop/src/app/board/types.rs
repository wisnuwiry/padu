use crate::model::{AgentSession, ProviderKind};
use crate::theme::Theme;
use padu_client::kanban::TaskStatus;

pub const BOARD_COLUMNS: [TaskStatus; 5] = [
    TaskStatus::Backlog,
    TaskStatus::Queued,
    TaskStatus::Running,
    TaskStatus::Review,
    TaskStatus::Done,
];

/// Curated assignee shortlist for the new-task modal. The detail drawer
/// offers the full `ProviderKind::ALL` registry like the web client.
pub const BOARD_MODAL_PROVIDERS: [ProviderKind; 6] = [
    ProviderKind::Agy,
    ProviderKind::Claude,
    ProviderKind::Codex,
    ProviderKind::Cursor,
    ProviderKind::DeepSeek,
    ProviderKind::OpenCode,
];

pub fn task_status_label(status: TaskStatus) -> String {
    match status {
        TaskStatus::Backlog => tr!("board.backlog"),
        TaskStatus::Queued => tr!("board.queued"),
        TaskStatus::Running => tr!("board.running"),
        TaskStatus::Review => tr!("board.review"),
        TaskStatus::Done => tr!("board.done"),
    }
}

pub fn task_status_color(status: TaskStatus, theme: &Theme) -> gpui::Hsla {
    match status {
        TaskStatus::Backlog => theme.text_tertiary,
        TaskStatus::Queued => theme.accent,
        TaskStatus::Running => theme.warning,
        TaskStatus::Review => theme.gauge,
        TaskStatus::Done => theme.success,
    }
}

/// Split the drawer's comma-separated labels input, mirroring the web
/// client's label parsing in `board-page.tsx`.
pub fn split_board_labels(input: &str) -> Vec<String> {
    input
        .split([',', '，', ';'])
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Live usage badges for a board card (P1-08).
///
/// The board joins each card's `session_id` to the in-memory `AgentSession`
/// catalog, which already streams `UsageUpdated` / turn events at commit
/// cadence (≤ ~8.3 Hz). Badges therefore refresh with the transcript: no UI
/// polling, no I/O in `render` — a missing session simply means "not known
/// yet".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaskLiveBadges {
    pub tokens: Option<u64>,
    pub duration_secs: Option<u64>,
}

/// Calculate live usage badges for a linked session.
pub fn task_live_badges(session: Option<&AgentSession>, now: u64) -> TaskLiveBadges {
    let Some(session) = session else {
        return TaskLiveBadges::default();
    };
    let mut tokens = session.context_usage.map(|usage| usage.tokens);
    if let Some(goal) = session.thread_goal.as_ref() {
        let goal_tokens = goal.tokens_used.max(0) as u64;
        tokens = Some(match tokens {
            Some(current) => current.max(goal_tokens),
            None => goal_tokens,
        });
    }
    let tokens = tokens.filter(|count| *count > 0);

    let mut duration_secs: Option<u64> = if session.turns.is_empty() {
        None
    } else {
        Some(
            session
                .turns
                .iter()
                .map(|turn| {
                    turn.completed_at
                        .unwrap_or(now)
                        .saturating_sub(turn.started_at)
                })
                .fold(0u64, |total, span| total.saturating_add(span)),
        )
    };
    if duration_secs.is_none()
        && let Some(goal) = session.thread_goal.as_ref()
        && goal.time_used_seconds > 0
    {
        duration_secs = Some(goal.time_used_seconds.max(0) as u64);
    }
    if duration_secs.is_none() {
        let end = if session.status.is_busy() {
            now
        } else {
            session.last_reply_at.unwrap_or(session.updated_at)
        };
        duration_secs = Some(end.saturating_sub(session.created_at));
    }
    let duration_secs = duration_secs.filter(|secs| *secs > 0);

    TaskLiveBadges {
        tokens,
        duration_secs,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardAgentPickTarget {
    NewTask,
    Edit,
}
