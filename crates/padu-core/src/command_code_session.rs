//! Command Code native-session transcript helpers.
//!
//! `command-code --print --output-format json` persists one JSONL transcript
//! per session under `~/.commandcode/projects/<cwd-slug>/<session-id>.jsonl`:
//! a `session` header, then one `message` line per user prompt, assistant
//! answer, and tool result, linked by `parentId`. The CLI scopes `--resume`
//! to the current working directory's slug, while `--session <path>` accepts
//! any transcript on disk — the driver prefers `--resume` and falls back to
//! `--session` so resume keeps working when a task moved directories.
//!
//! Forks are transcript prefixes copied to a fresh session id. The CLI
//! continues the `parentId` chain on the next `--resume` (verified against
//! the real CLI, 1.64.0), so rollback is the same operation with the turns to
//! drop converted to a retained prefix. Checkpoints (the CLI's own `/rewind`
//! file snapshots) are deliberately not copied: they belong to the source
//! session id, and Padu-level rewind restores the workspace through its own
//! turn checkpoints instead.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, anyhow, bail};
use chrono::SecondsFormat;
use serde_json::Value;
use uuid::Uuid;

use crate::fs_ext::canonical_display_string;
use crate::model::{
    AgentTurn, Message, MessageRole, ProviderResumeCursor, ProviderSessionHistory,
    ProviderSessionSummary, TurnStatus,
};

fn command_code_home() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".commandcode"))
}

fn projects_root() -> Option<PathBuf> {
    command_code_home().map(|home| home.join("projects"))
}

/// The directory slug the CLI files a transcript under: the canonicalized
/// working directory, lowercased, with every run of non-alphanumeric
/// characters folded to one dash (`/private/tmp/probe-a` →
/// `private-tmp-probe-a`).
pub fn slug_for_cwd(cwd: &Path) -> String {
    slugify(&canonical_display_string(cwd))
}

fn slugify(raw: &str) -> String {
    let lower = raw.to_lowercase();
    let mut slug = String::with_capacity(lower.len());
    let mut dash_pending = false;
    for ch in lower.chars() {
        if ch.is_alphanumeric() {
            if dash_pending && !slug.is_empty() {
                slug.push('-');
            }
            dash_pending = false;
            slug.push(ch);
        } else if !slug.is_empty() {
            dash_pending = true;
        }
    }
    slug
}

fn reject_bad_session_id(session_id: &str) -> anyhow::Result<()> {
    if session_id.is_empty()
        || session_id.contains('/')
        || session_id.contains('\\')
        || session_id.contains("..")
    {
        bail!("Command Code session {session_id} was not found on disk");
    }
    Ok(())
}

/// Transcript path for a session id inside any projects root.
fn find_session_file_in(root: &Path, session_id: &str) -> Option<PathBuf> {
    reject_bad_session_id(session_id).ok()?;
    let entries = fs::read_dir(root).ok()?;
    for slug_dir in entries.flatten() {
        let candidate = slug_dir.path().join(format!("{session_id}.jsonl"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn find_session_file(session_id: &str) -> Option<PathBuf> {
    find_session_file_in(&projects_root()?, session_id)
}

/// How the next `--print` invocation should address its session.
#[derive(Debug, PartialEq)]
pub enum CommandCodeResume {
    Fresh,
    ResumeId(String),
    SessionFile(PathBuf),
}

/// Resolve the resume target for `session_id` from `cwd`: `--resume` when the
/// transcript sits under the current slug, `--session <path>` when it lives
/// under another slug, and `--resume` (which the CLI rejects honestly) when
/// the id is unknown to disk.
pub fn resume_target(cwd: &Path, session_id: Option<&str>) -> CommandCodeResume {
    let Some(root) = projects_root() else {
        return match session_id {
            Some(id) => CommandCodeResume::ResumeId(id.to_owned()),
            None => CommandCodeResume::Fresh,
        };
    };
    resume_target_in(&root, cwd, session_id)
}

fn resume_target_in(root: &Path, cwd: &Path, session_id: Option<&str>) -> CommandCodeResume {
    let Some(session_id) = session_id else {
        return CommandCodeResume::Fresh;
    };
    if reject_bad_session_id(session_id).is_err() {
        return CommandCodeResume::ResumeId(session_id.to_owned());
    }
    if slug_session_path(root, cwd, session_id).is_file() {
        return CommandCodeResume::ResumeId(session_id.to_owned());
    }
    match find_session_file_in(root, session_id) {
        Some(path) => CommandCodeResume::SessionFile(path),
        None => CommandCodeResume::ResumeId(session_id.to_owned()),
    }
}

/// Preferred transcript path for a session started from `cwd`.
fn slug_session_path(root: &Path, cwd: &Path, session_id: &str) -> PathBuf {
    root.join(slug_for_cwd(cwd))
        .join(format!("{session_id}.jsonl"))
}

fn read_transcript_lines(path: &Path) -> anyhow::Result<Vec<String>> {
    let text =
        fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;
    Ok(text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect())
}

fn parse_line(line: &str, path: &Path) -> anyhow::Result<Value> {
    serde_json::from_str(line)
        .with_context(|| format!("Command Code history at {} is invalid JSON", path.display()))
}

/// Native turn count of a session: one per user prompt in its transcript.
pub fn count_native_turns(session_id: &str) -> anyhow::Result<usize> {
    reject_bad_session_id(session_id)?;
    let path = find_session_file(session_id)
        .ok_or_else(|| anyhow!("Command Code session {session_id} was not found on disk"))?;
    Ok(read_transcript_lines(&path)?
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(is_user_turn)
        .count())
}

/// One Padu turn is one user prompt: a `message` line whose role is `user`
/// and whose source is `user`. Tool results ride `role: user` too, but with
/// `source: tool`, and must not count as turns.
fn is_user_turn(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("message")
        && value.pointer("/message/role").and_then(Value::as_str) == Some("user")
        && value
            .pointer("/message/meta/source")
            .and_then(Value::as_str)
            .is_none_or(|source| source == "user")
}

fn message_text(value: &Value) -> Option<String> {
    let text = value
        .pointer("/message/content")
        .and_then(Value::as_array)?
        .iter()
        .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n\n");
    (!text.trim().is_empty()).then_some(text)
}

fn is_assistant_message(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("message")
        && value.pointer("/message/role").and_then(Value::as_str) == Some("assistant")
}

fn line_timestamp(value: &Value) -> u64 {
    value
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .and_then(|value| u64::try_from(value.timestamp()).ok())
        .unwrap_or_default()
}

/// Raw transcript lines through the end of the `retained_turns`-th user turn.
/// A zero count keeps only the session header.
fn truncate_to_turns(
    lines: &[String],
    path: &Path,
    retained_turns: usize,
) -> anyhow::Result<Vec<String>> {
    if lines.is_empty() {
        bail!("Command Code transcript at {} is empty", path.display());
    }
    let mut kept = Vec::new();
    let mut turns = 0;
    let mut complete = retained_turns == 0;
    for line in lines {
        let value = parse_line(line, path)?;
        if kept.is_empty() {
            kept.push(line.clone());
            continue;
        }
        if is_user_turn(&value) {
            if turns == retained_turns {
                complete = true;
                break;
            }
            turns += 1;
        }
        kept.push(line.clone());
    }
    if !complete {
        let total = lines
            .iter()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(is_user_turn)
            .count();
        if total < retained_turns {
            bail!("Command Code has only {total} native turns, but Padu needs {retained_turns}");
        }
    }
    Ok(kept)
}

/// File a fresh, header-only session under `cwd` and return its cursor.
/// Forking before the first turn has no transcript to copy; the empty session
/// still resumes (the CLI appends the first prompt to the bare header), which
/// keeps `fork(0)` total the way the other drivers' is.
pub fn create_empty_session(cwd: &Path) -> anyhow::Result<ProviderResumeCursor> {
    let root = projects_root()
        .ok_or_else(|| anyhow!("Command Code's home directory could not be located"))?;
    create_empty_session_in(&root, cwd)
}

fn create_empty_session_in(root: &Path, cwd: &Path) -> anyhow::Result<ProviderResumeCursor> {
    let fork_id = Uuid::new_v4().to_string();
    let header = serde_json::to_string(&serde_json::json!({
        "type": "session",
        "version": 3,
        "id": fork_id,
        "timestamp": chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        "cwd": canonical_display_string(cwd),
    }))?;
    write_fork_files(root, cwd, &fork_id, &[header])?;
    Ok(ProviderResumeCursor::CommandCode {
        session_id: fork_id,
    })
}

fn write_fork_files(
    root: &Path,
    cwd: &Path,
    fork_id: &str,
    lines: &[String],
) -> anyhow::Result<PathBuf> {
    let dir = root.join(slug_for_cwd(cwd));
    fs::create_dir_all(&dir).with_context(|| format!("could not create {}", dir.display()))?;
    let path = dir.join(format!("{fork_id}.jsonl"));
    fs::write(&path, lines.join("\n") + "\n")
        .with_context(|| format!("could not write Command Code fork {fork_id}"))?;
    fs::write(
        dir.join(format!("{fork_id}.meta.json")),
        "{\"entrypoint\":\"print\",\"traceIds\":[]}",
    )
    .with_context(|| format!("could not write Command Code fork metadata {fork_id}"))?;
    Ok(path)
}

/// Copy the first `retained_turns` native turns of a transcript to a fresh
/// session id filed under `cwd`, returning the cursor that resumes it.
pub fn fork_session_at_turn(
    cwd: &Path,
    source_session_id: &str,
    retained_turns: usize,
) -> anyhow::Result<ProviderResumeCursor> {
    let root = projects_root()
        .ok_or_else(|| anyhow!("Command Code's home directory could not be located"))?;
    fork_session_at_turn_in(&root, cwd, source_session_id, retained_turns)
}

fn fork_session_at_turn_in(
    root: &Path,
    cwd: &Path,
    source_session_id: &str,
    retained_turns: usize,
) -> anyhow::Result<ProviderResumeCursor> {
    reject_bad_session_id(source_session_id)?;
    let source = find_session_file_in(root, source_session_id)
        .ok_or_else(|| anyhow!("Command Code session {source_session_id} was not found on disk"))?;
    let lines = read_transcript_lines(&source)?;
    let kept = truncate_to_turns(&lines, &source, retained_turns)?;
    let fork_id = Uuid::new_v4().to_string();

    let mut kept = kept;
    if let Some(header) = kept.first_mut() {
        let mut value: Value = serde_json::from_str(header)
            .context("Command Code's session header is invalid JSON")?;
        if let Some(object) = value.as_object_mut() {
            object.insert("id".into(), Value::String(fork_id.clone()));
            object.insert(
                "timestamp".into(),
                Value::String(chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)),
            );
            let resolved = canonical_display_string(cwd);
            object.insert("cwd".into(), Value::String(resolved));
            *header = serde_json::to_string(&value)?;
        }
    }

    write_fork_files(root, cwd, &fork_id, &kept)?;
    Ok(ProviderResumeCursor::CommandCode {
        session_id: fork_id,
    })
}

fn history_from_transcript(path: &Path) -> anyhow::Result<ProviderSessionHistory> {
    let mut history = ProviderSessionHistory::default();
    for line in read_transcript_lines(path)? {
        let value = parse_line(&line, path)?;
        if is_user_turn(&value) {
            let at = line_timestamp(&value);
            history.turns.push(AgentTurn {
                id: Uuid::new_v4(),
                turn_count: history.turns.len() + 1,
                status: TurnStatus::Completed,
                provider_turn_started: true,
                provider_resume_at: None,
                started_at: at,
                completed_at: Some(at),
                checkpoint: None,
            });
            if let Some(text) = message_text(&value)
                && let Some(turn) = history.turns.last()
            {
                let mut message = Message::new_for_turn(MessageRole::User, text, turn.id);
                message.created_at = at;
                history.messages.push(message);
            }
            continue;
        }
        if !is_assistant_message(&value) {
            continue;
        }
        let Some(text) = message_text(&value) else {
            continue;
        };
        let Some(turn) = history.turns.last() else {
            continue;
        };
        let at = line_timestamp(&value);
        if let Some(previous) = history.messages.last_mut().filter(|message| {
            message.role == MessageRole::Assistant && message.turn_id == Some(turn.id)
        }) {
            if !previous.content.is_empty() {
                previous.content.push_str("\n\n");
            }
            previous.content.push_str(&text);
        } else {
            let mut message = Message::new_for_turn(MessageRole::Assistant, text, turn.id);
            message.created_at = at;
            history.messages.push(message);
        }
        if let Some(turn) = history.turns.last_mut() {
            turn.completed_at = Some(turn.completed_at.unwrap_or(at).max(at));
        }
    }
    Ok(history)
}

pub fn provider_session_history(
    session_id: &str,
    visible_turn_limit: usize,
) -> anyhow::Result<ProviderSessionHistory> {
    reject_bad_session_id(session_id)?;
    let path = find_session_file(session_id)
        .ok_or_else(|| anyhow!("Command Code session {session_id} was not found on disk"))?;
    let mut history = history_from_transcript(&path)?;
    truncate_history_to_visible_turns(&mut history, visible_turn_limit);
    Ok(history)
}

fn truncate_history_to_visible_turns(
    history: &mut ProviderSessionHistory,
    visible_turn_limit: usize,
) {
    let retained = history
        .turns
        .iter()
        .rev()
        .take(visible_turn_limit)
        .map(|turn| turn.id)
        .collect::<HashSet<_>>();
    history
        .messages
        .retain(|message| message.turn_id.is_some_and(|id| retained.contains(&id)));
}

fn summary_for_transcript(path: &Path) -> Option<ProviderSessionSummary> {
    let stem = path.file_stem()?.to_str()?;
    Uuid::parse_str(stem).ok()?;
    let lines = read_transcript_lines(path).ok()?;
    let header = lines
        .first()
        .and_then(|line| serde_json::from_str::<Value>(line).ok())?;
    let session_id = header
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| *id == stem)?;
    let created_at = line_timestamp(&header);
    let cwd = PathBuf::from(header.get("cwd").and_then(Value::as_str)?);
    if !cwd.is_absolute() {
        return None;
    }
    let mut first_prompt = None;
    let mut updated_at = created_at;
    for line in &lines {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        updated_at = updated_at.max(line_timestamp(&value));
        if first_prompt.is_none() && is_user_turn(&value) {
            first_prompt = message_text(&value);
        }
    }
    let title = first_prompt
        .as_deref()
        .and_then(crate::model::normalize_session_title)
        .unwrap_or_else(|| {
            cwd.file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .map(|name| format!("Command Code session in {name}"))
                .unwrap_or_else(|| format!("Command Code session {}", &stem[..8]))
        });
    Some(ProviderSessionSummary {
        cursor: ProviderResumeCursor::CommandCode {
            session_id: session_id.to_owned(),
        },
        title,
        cwd,
        created_at,
        updated_at,
    })
}

pub fn list_provider_sessions(limit: usize) -> anyhow::Result<Vec<ProviderSessionSummary>> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let Some(root) = projects_root() else {
        return Ok(Vec::new());
    };
    list_provider_sessions_in(&root, limit)
}

fn list_provider_sessions_in(
    root: &Path,
    limit: usize,
) -> anyhow::Result<Vec<ProviderSessionSummary>> {
    let mut sessions = Vec::new();
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("could not read {}", root.display()));
        }
    };
    for slug_dir in entries.flatten() {
        let Ok(children) = fs::read_dir(slug_dir.path()) else {
            continue;
        };
        for child in children.flatten() {
            let path = child.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }
            if let Some(summary) = summary_for_transcript(&path) {
                sessions.push(summary);
            }
        }
    }
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    sessions.truncate(limit);
    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("padu-command-code-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn header(id: &str, cwd: &str) -> String {
        json!({"type": "session", "version": 3, "id": id, "timestamp": "2026-09-23T08:00:00.000Z", "cwd": cwd}).to_string()
    }

    fn user_message(id: &str, parent: Option<&str>, text: &str) -> String {
        let mut value = json!({
            "type": "message",
            "id": id,
            "timestamp": "2026-09-23T08:01:00.000Z",
            "message": {
                "role": "user",
                "content": [{"type": "text", "text": text}],
                "meta": {"source": "user", "createdAt": 1_790_153_510_000_i64, "messageId": id}
            }
        });
        value["parentId"] = parent
            .map(|id| Value::String(id.to_owned()))
            .unwrap_or(Value::Null);
        value.to_string()
    }

    fn assistant_message(id: &str, parent: &str, text: &str) -> String {
        json!({
            "type": "message",
            "id": id,
            "parentId": parent,
            "timestamp": "2026-09-23T08:02:00.000Z",
            "message": {
                "role": "assistant",
                "content": [{"type": "text", "text": text}],
                "meta": {"source": "model", "createdAt": 1_790_153_520_000_i64, "messageId": id}
            },
            "usage": {"inputTokens": 100, "outputTokens": 10},
            "model": "deepseek/deepseek-v4-flash"
        })
        .to_string()
    }

    fn tool_result_message(id: &str, parent: &str) -> String {
        json!({
            "type": "message",
            "id": id,
            "parentId": parent,
            "timestamp": "2026-09-23T08:01:30.000Z",
            "message": {
                "role": "user",
                "content": [{"type": "tool_result", "tool_use_id": "call_1", "content": [{"type": "text", "text": "ok"}]}],
                "meta": {"source": "tool", "messageId": id}
            }
        })
        .to_string()
    }

    fn two_turn_lines(session_id: &str) -> Vec<String> {
        vec![
            header(session_id, "/work"),
            user_message("u1", None, "first question"),
            tool_result_message("t1", "u1"),
            assistant_message("a1", "t1", "first answer"),
            user_message("u2", Some("a1"), "second question"),
            assistant_message("a2", "u2", "second answer"),
        ]
    }

    fn write_transcript(root: &Path, slug: &str, session_id: &str, lines: &[String]) -> PathBuf {
        let dir = root.join(slug);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{session_id}.jsonl"));
        fs::write(&path, lines.join("\n") + "\n").unwrap();
        path
    }

    #[test]
    fn slug_folds_separators_and_lowercases() {
        assert_eq!(slugify("/private/tmp/probe-a"), "private-tmp-probe-a");
        assert_eq!(
            slugify("/Users/wisnusaputra/Documents/Project"),
            "users-wisnusaputra-documents-project"
        );
        assert_eq!(slugify("///"), "");
    }

    #[test]
    fn verbatim_prefixes_fold_out_of_slugs() {
        use crate::fs_ext::strip_verbatim_prefix;

        assert_eq!(
            slugify(strip_verbatim_prefix(r"\\?\C:\Users\dev")),
            "c-users-dev"
        );
    }

    #[test]
    fn resume_prefers_the_current_slug_then_any_transcript() {
        let root = temp_root();
        let session_id = Uuid::new_v4().to_string();
        let here = root.join("here");
        fs::create_dir_all(&here).unwrap();
        let lines = vec![
            header(&session_id, "/away"),
            user_message("u1", None, "hello"),
        ];
        let away_path = write_transcript(&root, "away", &session_id, &lines);

        // Transcript lives under another slug: address it by path.
        assert_eq!(
            resume_target_in(&root, &here, Some(&session_id)),
            CommandCodeResume::SessionFile(away_path)
        );
        // Filed under the current slug: plain `--resume`.
        let here_slug = slug_for_cwd(&here);
        write_transcript(&root, &here_slug, &session_id, &lines);
        assert_eq!(
            resume_target_in(&root, &here, Some(&session_id)),
            CommandCodeResume::ResumeId(session_id.clone())
        );
        // Unknown id and no session: fresh or an honest `--resume` failure.
        assert_eq!(
            resume_target_in(&root, &here, Some("missing-session")),
            CommandCodeResume::ResumeId("missing-session".into())
        );
        assert_eq!(
            resume_target_in(&root, &here, None),
            CommandCodeResume::Fresh
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn truncation_keeps_whole_turns_and_rejects_over_retention() {
        let path = PathBuf::from("transcript.jsonl");
        let lines = two_turn_lines("s1");
        let kept = truncate_to_turns(&lines, &path, 1).unwrap();
        assert_eq!(kept.len(), 4);
        assert!(kept[1].contains("first question"));
        // Tool results ride along with their turn and never count as one.
        assert!(kept[2].contains("tool_result"));

        let kept = truncate_to_turns(&lines, &path, 0).unwrap();
        assert_eq!(kept.len(), 1);

        let error = truncate_to_turns(&lines, &path, 3).unwrap_err();
        assert!(error.to_string().contains("only 2 native turns"));
    }

    #[test]
    fn fork_rewrites_the_header_and_files_the_prefix_under_cwd() {
        let root = temp_root();
        let source_id = Uuid::new_v4().to_string();
        let lines = two_turn_lines(&source_id);
        write_transcript(&root, "elsewhere", &source_id, &lines);
        let cwd = root.join("work");
        fs::create_dir_all(&cwd).unwrap();

        let cursor = fork_session_at_turn_in(&root, &cwd, &source_id, 1).unwrap();
        let ProviderResumeCursor::CommandCode {
            session_id: fork_id,
        } = cursor
        else {
            panic!("expected a Command Code fork cursor");
        };
        assert_ne!(fork_id, source_id);
        let fork_path = root
            .join(slug_for_cwd(&cwd))
            .join(format!("{fork_id}.jsonl"));
        let forked = read_transcript_lines(&fork_path).unwrap();
        assert_eq!(forked.len(), 4);
        let header: Value = serde_json::from_str(&forked[0]).unwrap();
        assert_eq!(
            header.get("id").and_then(Value::as_str),
            Some(fork_id.as_str())
        );
        assert!(forked[1].contains("first question"));
        // Sibling metadata is written; file-restore checkpoints stay behind.
        assert!(
            root.join(slug_for_cwd(&cwd))
                .join(format!("{fork_id}.meta.json"))
                .is_file()
        );

        let error = fork_session_at_turn_in(&root, &cwd, &source_id, 5).unwrap_err();
        assert!(error.to_string().contains("only 2 native turns"));
        assert!(
            fork_session_at_turn_in(&root, &cwd, "missing", 0)
                .unwrap_err()
                .to_string()
                .contains("was not found")
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn history_skips_tool_traffic_and_limits_visible_turns() {
        let root = temp_root();
        let path = root.join("history.jsonl");
        let lines = two_turn_lines("s1");
        fs::write(&path, lines.join("\n") + "\n").unwrap();

        let mut history = history_from_transcript(&path).unwrap();
        assert_eq!(history.turns.len(), 2);
        assert_eq!(history.turns[0].turn_count, 1);
        assert!(history.turns.iter().all(|turn| turn.provider_turn_started));
        let roles = history
            .messages
            .iter()
            .map(|message| (message.role, message.content.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            roles,
            [
                (MessageRole::User, "first question".into()),
                (MessageRole::Assistant, "first answer".into()),
                (MessageRole::User, "second question".into()),
                (MessageRole::Assistant, "second answer".into()),
            ]
        );

        truncate_history_to_visible_turns(&mut history, 1);
        assert_eq!(history.turns.len(), 2);
        assert_eq!(history.messages.len(), 2);
        assert!(
            history
                .messages
                .iter()
                .all(|message| message.content.contains("second"))
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn summaries_skip_non_session_files_and_prefer_the_first_prompt() {
        let root = temp_root();
        let session_id = Uuid::new_v4().to_string();
        let path = write_transcript(&root, "slug", &session_id, &two_turn_lines(&session_id));
        // Checkpoint sidecars share the extension but never parse as sessions.
        fs::write(
            root.join("slug")
                .join(format!("{session_id}.checkpoints.jsonl")),
            "{}\n",
        )
        .unwrap();

        let summary = summary_for_transcript(&path).expect("session transcript");
        assert_eq!(summary.title, "first question");
        assert_eq!(summary.cwd, PathBuf::from("/work"));
        assert!(summary.updated_at >= summary.created_at);
        assert!(summary.cursor.native_id() == session_id);

        let sessions = list_provider_sessions_in(&root, 10).unwrap();
        assert_eq!(sessions.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_sessions_file_a_resumable_header() {
        let root = temp_root();
        let cwd = root.join("work");
        fs::create_dir_all(&cwd).unwrap();

        let cursor = create_empty_session_in(&root, &cwd).unwrap();
        let ProviderResumeCursor::CommandCode { session_id } = cursor else {
            panic!("expected a Command Code cursor");
        };
        let path = root
            .join(slug_for_cwd(&cwd))
            .join(format!("{session_id}.jsonl"));
        let lines = read_transcript_lines(&path).unwrap();
        assert_eq!(lines.len(), 1);
        let header: Value = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(
            header.get("id").and_then(Value::as_str),
            Some(session_id.as_str())
        );
        let _ = fs::remove_dir_all(&root);
    }
}
