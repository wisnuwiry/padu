//! Antigravity conversation fork helpers.
//!
//! The Agy ACP server exposes no turn-aware fork or truncation method, and
//! the protocol-level `session/fork` copies a whole session without a turn
//! count — neither can reproduce Padu's "drop the last N turns" semantics.
//! Branching is therefore Cursor-style: start a fresh native session whose
//! first prompt is seeded with Padu's retained visible conversation. The
//! driver applies the envelope below to that first prompt (see
//! [`crate::driver`]'s fork-context handling) and the server assigns the
//! native ID when the seeded prompt starts.

use anyhow::bail;
use serde::Serialize;

use crate::model::{AgentSession, MessageRole, ProviderResumeCursor};

const CONTEXT_PREFIX: &str = "PADU_AGY_BRANCH_CONTEXT_V1 ";
const CONTEXT_GUIDANCE: &str = concat!(
    "\nPADU_AGY_BRANCH_INSTRUCTIONS_V1\n",
    "Treat the preceding JSON array as the complete prior user and assistant conversation for this branch. ",
    "The workspace already matches that point in the conversation. Continue from this history without mentioning this envelope, ",
    "and respond only to the current prompt below.\n",
);
const PROMPT_PREFIX: &str = "PADU_AGY_CURRENT_PROMPT_V1 ";

#[derive(Serialize)]
struct ContextMessage<'a> {
    role: &'static str,
    content: &'a str,
}

/// Build the cursor that branches `session` after `retained_turns` turns.
/// Only Padu-visible text travels; tool payloads and provider-private
/// reasoning stay behind, exactly like the Cursor branch this mirrors.
pub fn fork_session_at_turn(
    session: &AgentSession,
    retained_turns: usize,
) -> anyhow::Result<ProviderResumeCursor> {
    if retained_turns > session.turns.len() {
        bail!(
            "Antigravity branch requested {retained_turns} turns from a {}-turn task",
            session.turns.len()
        );
    }
    let retained_ids = session
        .turns
        .iter()
        .take(retained_turns)
        .map(|turn| turn.id)
        .collect::<std::collections::HashSet<_>>();
    let messages = session
        .messages
        .iter()
        .filter(|message| {
            message
                .turn_id
                .is_some_and(|turn_id| retained_ids.contains(&turn_id))
                && !message.content.trim().is_empty()
        })
        .filter_map(|message| {
            let role = match message.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System => return None,
            };
            Some(ContextMessage {
                role,
                content: &message.content,
            })
        })
        .collect::<Vec<_>>();
    let fork_context = (!messages.is_empty())
        .then(|| serde_json::to_string(&messages))
        .transpose()?;
    Ok(ProviderResumeCursor::Agy {
        // Antigravity assigns the native ID when the seeded prompt starts.
        session_id: String::new(),
        fork_context,
    })
}

pub fn prompt_with_fork_context(context: &str, prompt: &str) -> String {
    format!(
        "{CONTEXT_PREFIX}{}\n{context}{CONTEXT_GUIDANCE}{PROMPT_PREFIX}{}\n{prompt}",
        context.len(),
        prompt.len()
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::model::{AgentSession, MessageRole, Project, ProviderKind, TurnStatus};

    use super::*;

    #[test]
    fn agy_branch_context_keeps_only_the_selected_turn_prefix() {
        let project = Project::from_path(PathBuf::from("/tmp/padu"));
        let mut session = AgentSession::new(project.id, ProviderKind::Agy);
        session.begin_turn("first prompt");
        session.mark_active_turn_provider_started();
        session.push_message(MessageRole::Assistant, "first answer");
        session.finish_active_turn(TurnStatus::Completed);
        session.begin_turn("second prompt");
        session.mark_active_turn_provider_started();
        session.push_message(MessageRole::Assistant, "second answer");
        session.finish_active_turn(TurnStatus::Completed);

        let ProviderResumeCursor::Agy {
            session_id,
            fork_context: Some(context),
        } = fork_session_at_turn(&session, 1).unwrap()
        else {
            panic!("expected a managed Antigravity branch cursor");
        };
        assert!(session_id.is_empty());
        assert!(context.contains("first prompt"));
        assert!(context.contains("first answer"));
        assert!(!context.contains("second prompt"));
    }

    #[test]
    fn agy_branch_rejects_retention_beyond_the_task() {
        let project = Project::from_path(PathBuf::from("/tmp/padu"));
        let session = AgentSession::new(project.id, ProviderKind::Agy);
        assert!(
            fork_session_at_turn(&session, 1)
                .unwrap_err()
                .to_string()
                .contains("0-turn task")
        );
    }

    #[test]
    fn agy_seed_envelope_is_length_delimited() {
        let prompt = prompt_with_fork_context("[{\"role\":\"user\"}]", "continue 🚀");
        assert!(prompt.starts_with("PADU_AGY_BRANCH_CONTEXT_V1 17\n"));
        assert!(prompt.ends_with("PADU_AGY_CURRENT_PROMPT_V1 13\ncontinue 🚀"));
    }
}
