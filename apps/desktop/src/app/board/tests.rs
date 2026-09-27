use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::model::{
    AgentSession, AgentTurn, ContextUsage, InteractionMode, ProviderKind, RuntimeMode,
    SessionStatus, SessionWorkspace, ThreadGoal, ThreadGoalStatus, TurnStatus,
};
use padu_client::kanban::{TaskStatus, TaskSummary};
use uuid::Uuid;

use super::types::{TaskLiveBadges, split_board_labels, task_live_badges};

fn test_session() -> AgentSession {
    AgentSession {
        id: Uuid::new_v4(),
        title: String::from("test session"),
        auto_title: None,
        project_id: Uuid::new_v4(),
        workspace: SessionWorkspace::Local,
        provider: ProviderKind::Codex,
        model: None,
        runtime_mode: RuntimeMode::FullAccess,
        interaction_mode: InteractionMode::Build,
        reasoning_effort: None,
        service_tier: None,
        context_window: None,
        agent_preset: None,
        status: SessionStatus::Working,
        created_at: 1_000,
        updated_at: 2_000,
        last_reply_at: None,
        pinned_at: None,
        archived_at: None,
        provider_cursor: None,
        available_commands: Vec::new(),
        thread_goal: None,
        context_usage: None,
        runtime_event_cursor: None,
        provider_session_id: None,
        messages: Vec::new(),
        transcript_blocks: Vec::new(),
        turns: Vec::new(),
        queued_messages: Vec::new(),
        detail_loaded: true,
    }
}

fn test_turn(started_at: u64, completed_at: Option<u64>) -> AgentTurn {
    AgentTurn {
        id: Uuid::new_v4(),
        turn_count: 1,
        status: TurnStatus::Completed,
        provider_turn_started: false,
        provider_resume_at: None,
        started_at,
        completed_at,
        checkpoint: None,
    }
}

fn test_summary(index: u64, session_id: Option<Uuid>) -> TaskSummary {
    TaskSummary {
        id: Uuid::from_u128(index as u128),
        project_id: Uuid::nil(),
        title: format!("task {index}"),
        description_preview: String::new(),
        status: TaskStatus::Running,
        assigned_agent: None,
        model: None,
        session_id,
        labels: Vec::new(),
        needs_attention: false,
        sync_failed: None,
        updated_at: 2_000,
        version: 1,
        archived: false,
    }
}

#[test]
fn live_badges_absent_without_session() {
    assert_eq!(task_live_badges(None, 5_000), TaskLiveBadges::default());
}

#[test]
fn live_badges_hide_zero_usage_but_tick_busy_wall_clock() {
    let session = test_session();
    assert_eq!(
        task_live_badges(Some(&session), 5_000),
        TaskLiveBadges {
            tokens: None,
            duration_secs: Some(4_000),
        }
    );
}

#[test]
fn live_badges_read_context_usage_and_turns() {
    let mut session = test_session();
    session.context_usage = Some(ContextUsage {
        tokens: 12_500,
        window: Some(200_000),
    });
    session.turns = vec![test_turn(1_000, Some(1_060)), test_turn(2_000, None)];
    // The open turn ticks against `now`.
    assert_eq!(
        task_live_badges(Some(&session), 2_030),
        TaskLiveBadges {
            tokens: Some(12_500),
            duration_secs: Some(90),
        }
    );
}

#[test]
fn live_badges_prefer_larger_goal_ledger() {
    let mut session = test_session();
    session.status = SessionStatus::Idle;
    session.context_usage = Some(ContextUsage {
        tokens: 100,
        window: None,
    });
    session.thread_goal = Some(ThreadGoal {
        objective: String::from("goal"),
        status: ThreadGoalStatus::Active,
        token_budget: Some(50_000),
        tokens_used: 4_000,
        time_used_seconds: 0,
    });
    let badges = task_live_badges(Some(&session), 9_000);
    assert_eq!(badges.tokens, Some(4_000));
    // No turns and no goal time: idle wall clock from `updated_at`.
    assert_eq!(badges.duration_secs, Some(1_000));
}

#[test]
fn drawer_labels_split_on_commas_and_trim() {
    assert!(split_board_labels("").is_empty());
    assert_eq!(
        split_board_labels("bug, feature ,,  refactor "),
        vec![
            "bug".to_owned(),
            "feature".to_owned(),
            "refactor".to_owned()
        ]
    );
}

#[test]
fn live_badges_stay_interactive_with_500_cards() {
    let now = 1_000_000u64;
    let mut sessions = Vec::with_capacity(500);
    let mut summaries = Vec::with_capacity(500);
    for index in 0..500u64 {
        let session_id = Uuid::from_u128(1_000_000 + index as u128);
        let mut session = test_session();
        session.id = session_id;
        session.context_usage = Some(ContextUsage {
            tokens: 1_000 + index,
            window: None,
        });
        session.turns = vec![test_turn(now - 300, Some(now - 240))];
        sessions.push(session);
        summaries.push(test_summary(index, Some(session_id)));
    }

    let started = Instant::now();
    // One pass per frame, then O(1) lookups per visible row — the same
    // shape as `render_board_page`.
    let live: HashMap<Uuid, TaskLiveBadges> = sessions
        .iter()
        .map(|session| (session.id, task_live_badges(Some(session), now)))
        .collect();
    let mut rendered = 0u64;
    for summary in &summaries {
        if let Some(badges) = summary.session_id.and_then(|id| live.get(&id).copied()) {
            rendered += u64::from(badges.tokens.is_some());
        }
    }
    let elapsed = started.elapsed();

    assert_eq!(rendered, 500);
    assert!(
        elapsed < Duration::from_millis(500),
        "500-card badge join took {elapsed:?}"
    );
}
