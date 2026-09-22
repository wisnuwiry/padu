//! Grok native-session metadata and ACP helpers.

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use anyhow::{Context as _, anyhow, bail};
use chrono::{SecondsFormat, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::model::{
    ProviderModel, ProviderModelOption, ProviderResumeCursor, ProviderSessionSummary,
};

const RPC_TIMEOUT: Duration = Duration::from_secs(30);

fn summary_timestamp(value: Option<&Value>) -> u64 {
    value
        .and_then(Value::as_str)
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .and_then(|value| u64::try_from(value.timestamp()).ok())
        .unwrap_or_default()
}

fn provider_summary(value: &Value) -> Option<ProviderSessionSummary> {
    let kind = value.get("session_kind").and_then(Value::as_str);
    let hidden = value
        .get("hidden")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| kind.is_some_and(|kind| kind.starts_with("subagent")));
    if hidden || kind == Some("headless") {
        return None;
    }
    let session_id = value
        .pointer("/info/id")
        .and_then(Value::as_str)
        .or_else(|| value.get("id").and_then(Value::as_str))?
        .trim();
    Uuid::parse_str(session_id).ok()?;
    let cwd = PathBuf::from(value.pointer("/info/cwd").and_then(Value::as_str)?);
    if !cwd.is_absolute() {
        return None;
    }
    let title = ["generated_title", "session_summary", "title"]
        .into_iter()
        .find_map(|key| {
            value
                .get(key)
                .and_then(Value::as_str)
                .and_then(crate::model::normalize_session_title)
        })
        .unwrap_or_else(|| {
            cwd.file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Grok session {}", &session_id[..8]))
        });
    let created_at = summary_timestamp(value.get("created_at"));
    let updated_at = ["last_active_at", "updated_at", "created_at"]
        .into_iter()
        .filter_map(|key| value.get(key))
        .map(|value| summary_timestamp(Some(value)))
        .max()
        .unwrap_or(created_at)
        .max(created_at);
    Some(ProviderSessionSummary {
        cursor: ProviderResumeCursor::Grok {
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
    let root = grok_home_directory()?.join("sessions");
    let mut sessions = Vec::new();
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("could not read {}", root.display()));
        }
    };
    for workspace in entries.flatten() {
        let Ok(children) = fs::read_dir(workspace.path()) else {
            continue;
        };
        for child in children.flatten() {
            let path = child.path().join("summary.json");
            let Ok(bytes) = fs::read(path) else {
                continue;
            };
            let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
                continue;
            };
            if let Some(summary) = provider_summary(&value) {
                sessions.push(summary);
            }
        }
    }
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    sessions.truncate(limit);
    Ok(sessions)
}

pub fn generated_title(session_id: &str) -> anyhow::Result<Option<String>> {
    generated_title_in(&grok_home_directory()?, session_id)
}

pub fn generated_title_in(grok_home: &Path, session_id: &str) -> anyhow::Result<Option<String>> {
    Uuid::parse_str(session_id).context("Grok returned an invalid session ID")?;
    let summary_path = find_session_directory_in(grok_home, session_id)?.join("summary.json");
    let summary: Value = serde_json::from_slice(
        &fs::read(&summary_path)
            .with_context(|| format!("could not read {}", summary_path.display()))?,
    )
    .context("Grok's session summary is invalid JSON")?;
    Ok(summary
        .get("generated_title")
        .and_then(Value::as_str)
        .and_then(crate::model::normalize_session_title))
}

/// Ascending effort order Padu renders across providers, so Grok's menu keeps
/// the same low-to-high shape as its plain-text fallback.
const EFFORT_ORDER: [&str; 8] = [
    "none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra",
];

/// Discovers Grok's catalog over ACP.
///
/// Grok advertises every model's reasoning menu — effort ids, labels,
/// descriptions, and the provider's default — in the `initialize` response's
/// `_meta.modelState`, so the picker follows the installed CLI instead of a
/// hardcoded list of built-ins.
pub fn discover_models(binary: &Path) -> anyhow::Result<Vec<ProviderModel>> {
    let mut client = GrokRpc::start_with_args(binary, &["agent", "--no-leader", "stdio"])?;
    let response = client.request(
        1,
        "initialize",
        json!({
            "protocolVersion": 1,
            "clientCapabilities": {
                "fs": {"readTextFile": false, "writeTextFile": false},
                "terminal": false
            }
        }),
    )?;
    Ok(parse_model_state(&response))
}

fn parse_model_state(response: &Value) -> Vec<ProviderModel> {
    let Some(state) = response.pointer("/result/_meta/modelState") else {
        return Vec::new();
    };
    let current = state.get("currentModelId").and_then(Value::as_str);
    state
        .get("availableModels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| model_from_state(value, current))
        .collect()
}

fn model_from_state(value: &Value, current: Option<&str>) -> Option<ProviderModel> {
    let id = value.get("modelId").and_then(Value::as_str)?.trim();
    if id.is_empty() {
        return None;
    }
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| crate::model_catalog::display_name_from_slug(id));
    let mut model = ProviderModel::new(id, name);
    model.is_default = current == Some(id);
    if let Some((efforts, default)) = reasoning_menu(value) {
        model = model.reasoning(efforts, default);
    }
    Some(model)
}

fn reasoning_menu(value: &Value) -> Option<(Vec<ProviderModelOption>, String)> {
    let meta = value.get("_meta")?;
    if meta.get("supportsReasoningEffort").and_then(Value::as_bool) == Some(false) {
        return None;
    }
    let mut efforts = meta
        .get("reasoningEfforts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(provider_option)
        .collect::<Vec<_>>();
    if efforts.is_empty() {
        return None;
    }
    efforts.sort_by_key(|option| {
        EFFORT_ORDER
            .iter()
            .position(|known| *known == option.id)
            .unwrap_or(usize::MAX)
    });
    let default = default_effort(meta, &efforts);
    Some((efforts, default))
}

fn provider_option(value: &Value) -> Option<ProviderModelOption> {
    let id = effort_id(value)?;
    if id.is_empty() {
        return None;
    }
    let mut option = ProviderModelOption::new(id, crate::model_catalog::reasoning_effort_label(id));
    if let Some(description) = value.get("description").and_then(Value::as_str) {
        option = option.description(description);
    }
    Some(option)
}

fn effort_id(value: &Value) -> Option<&str> {
    ["id", "value"]
        .into_iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .map(str::trim)
}

fn default_effort(meta: &Value, efforts: &[ProviderModelOption]) -> String {
    let flagged = meta
        .get("reasoningEfforts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|effort| effort.get("default").and_then(Value::as_bool) == Some(true))
        .and_then(effort_id);
    flagged
        .map(str::to_owned)
        .or_else(|| {
            meta.get("reasoningEffort")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .filter(|id| efforts.iter().any(|option| option.id == *id))
        .unwrap_or_else(|| efforts[0].id.clone())
}

pub fn fork_session_at_turn(
    binary: &Path,
    cwd: &Path,
    source_session_id: &str,
    retained_turns: usize,
) -> anyhow::Result<ProviderResumeCursor> {
    Uuid::parse_str(source_session_id).context("Grok returned an invalid source session ID")?;
    let source_dir = find_session_directory(source_session_id)?;
    let fork_id = fork_native_session(binary, cwd, source_session_id)?;
    let fork_dir = source_dir
        .parent()
        .ok_or_else(|| anyhow!("Grok's source session directory has no parent"))?
        .join(&fork_id);
    let result = truncate_fork(&fork_dir, retained_turns);
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&fork_dir);
        return Err(error);
    }
    Ok(ProviderResumeCursor::Grok {
        session_id: fork_id,
    })
}

fn fork_native_session(
    binary: &Path,
    cwd: &Path,
    source_session_id: &str,
) -> anyhow::Result<String> {
    let mut client = GrokRpc::start(binary)?;
    client.request(
        1,
        "initialize",
        json!({
            "protocolVersion": 1,
            "clientCapabilities": {
                "fs": {"readTextFile": true, "writeTextFile": true},
                "terminal": true
            }
        }),
    )?;
    let response = client.request(
        2,
        "_x.ai/session/fork",
        json!({
            "sourceSessionId": source_session_id,
            "sourceCwd": cwd,
            "newCwd": cwd
        }),
    )?;
    let session_id = response
        .pointer("/result/newSessionId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| anyhow!("Grok returned no forked session ID"))?;
    Uuid::parse_str(session_id).context("Grok returned an invalid forked session ID")?;
    Ok(session_id.to_owned())
}

fn find_session_directory(session_id: &str) -> anyhow::Result<PathBuf> {
    find_session_directory_in(&grok_home_directory()?, session_id)
}

fn grok_home_directory() -> anyhow::Result<PathBuf> {
    std::env::var_os("GROK_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".grok")))
        .ok_or_else(|| anyhow!("Grok's home directory could not be located"))
}

fn find_session_directory_in(grok_home: &Path, session_id: &str) -> anyhow::Result<PathBuf> {
    let sessions = grok_home.join("sessions");
    for entry in fs::read_dir(&sessions)
        .with_context(|| format!("could not read Grok sessions at {}", sessions.display()))?
    {
        let Ok(entry) = entry else {
            continue;
        };
        let candidate = entry.path().join(session_id);
        if candidate.join("summary.json").is_file() {
            return Ok(candidate);
        }
    }
    bail!("Grok session {session_id} was not found on disk")
}

fn truncate_fork(session_dir: &Path, retained_turns: usize) -> anyhow::Result<()> {
    let chat_path = session_dir.join("chat_history.jsonl");
    let updates_path = session_dir.join("updates.jsonl");
    let chat = read_json_lines(&chat_path)?;
    let updates = read_json_lines(&updates_path)?;
    let chat = truncate_at_turn(&chat, retained_turns, is_chat_prompt)?;
    let updates = truncate_at_turn(&updates, retained_turns, is_update_prompt)?;
    write_json_lines(&chat_path, &chat)?;
    write_json_lines(&updates_path, &updates)?;

    let summary_path = session_dir.join("summary.json");
    let mut summary: Value = serde_json::from_slice(
        &fs::read(&summary_path)
            .with_context(|| format!("could not read {}", summary_path.display()))?,
    )
    .context("Grok's fork summary is invalid JSON")?;
    summary["num_messages"] = json!(updates.len());
    summary["num_chat_messages"] = json!(chat.len());
    summary["updated_at"] = json!(Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true));
    write_json(&summary_path, &summary)
}

fn truncate_at_turn(
    values: &[Value],
    retained_turns: usize,
    is_prompt: fn(&Value) -> bool,
) -> anyhow::Result<Vec<Value>> {
    let mut turns = 0;
    let cutoff = values.iter().position(|value| {
        if !is_prompt(value) {
            return false;
        }
        if turns == retained_turns {
            true
        } else {
            turns += 1;
            false
        }
    });
    if cutoff.is_none() {
        turns = values.iter().filter(|value| is_prompt(value)).count();
        if turns < retained_turns {
            bail!("Grok has only {turns} native turns, but Padu needs {retained_turns}");
        }
    }
    Ok(values[..cutoff.unwrap_or(values.len())].to_vec())
}

fn is_chat_prompt(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("user")
        && value.get("prompt_index").and_then(Value::as_u64).is_some()
}

fn is_update_prompt(value: &Value) -> bool {
    value
        .pointer("/params/update/sessionUpdate")
        .and_then(Value::as_str)
        == Some("user_message_chunk")
}

fn read_json_lines(path: &Path) -> anyhow::Result<Vec<Value>> {
    let file = fs::File::open(path)
        .with_context(|| format!("could not open Grok history at {}", path.display()))?;
    BufReader::new(file)
        .lines()
        .map(|line| {
            let line = line?;
            serde_json::from_str(&line).context("Grok's history contains invalid JSON")
        })
        .collect()
}

fn write_json_lines(path: &Path, values: &[Value]) -> anyhow::Result<()> {
    write_atomic(path, |writer| {
        for value in values {
            serde_json::to_writer(&mut *writer, value)?;
            writer.write_all(b"\n")?;
        }
        Ok(())
    })
}

fn write_json(path: &Path, value: &Value) -> anyhow::Result<()> {
    write_atomic(path, |writer| {
        serde_json::to_writer_pretty(&mut *writer, value)?;
        writer.write_all(b"\n")?;
        Ok(())
    })
}

fn write_atomic(
    path: &Path,
    write: impl FnOnce(&mut BufWriter<fs::File>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Grok history path has no parent"))?;
    let temp = parent.join(format!(".padu-{}.tmp", Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let file = options
        .open(&temp)
        .with_context(|| format!("could not create {}", temp.display()))?;
    let mut writer = BufWriter::new(file);
    if let Err(error) = write(&mut writer).and_then(|()| {
        writer.flush()?;
        writer.get_ref().sync_all()?;
        Ok(())
    }) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    fs::rename(&temp, path)
        .with_context(|| format!("could not replace Grok history at {}", path.display()))
}

struct GrokRpc {
    child: Child,
    stdin: ChildStdin,
    responses: Receiver<Value>,
}

impl GrokRpc {
    fn start(binary: &Path) -> anyhow::Result<Self> {
        Self::start_with_args(
            binary,
            &["agent", "--always-approve", "--no-leader", "stdio"],
        )
    }

    fn start_with_args(binary: &Path, args: &[&str]) -> anyhow::Result<Self> {
        let mut command = crate::command_env::command(binary);
        let command = command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child =
            crate::command_env::spawn(command).context("failed to start Grok's ACP server")?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Grok ACP stdin is unavailable"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Grok ACP stdout is unavailable"))?;
        let (tx, responses) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(value) = serde_json::from_str(&line) {
                    let _ = tx.send(value);
                }
            }
        });
        Ok(Self {
            child,
            stdin,
            responses,
        })
    }

    fn request(&mut self, id: u64, method: &str, params: Value) -> anyhow::Result<Value> {
        serde_json::to_writer(
            &mut self.stdin,
            &json!({"jsonrpc":"2.0", "id":id, "method":method, "params":params}),
        )?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        loop {
            let response = self
                .responses
                .recv_timeout(RPC_TIMEOUT)
                .with_context(|| format!("timed out waiting for Grok {method}"))?;
            if response.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = response.get("error") {
                bail!("Grok {method} failed: {error}");
            }
            return Ok(response);
        }
    }
}

impl Drop for GrokRpc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn truncates_chat_history_before_the_first_excluded_prompt() {
        let history = vec![
            json!({"type":"system"}),
            json!({"type":"user","prompt_index":0}),
            json!({"type":"assistant"}),
            json!({"type":"user","synthetic_reason":"reminder"}),
            json!({"type":"user","prompt_index":1}),
            json!({"type":"assistant"}),
        ];
        let truncated = truncate_at_turn(&history, 1, is_chat_prompt).unwrap();
        assert_eq!(truncated.len(), 4);
        assert_eq!(truncated.last().unwrap()["synthetic_reason"], "reminder");
    }

    #[test]
    fn truncates_native_updates_at_the_matching_user_chunk() {
        let update = |kind| json!({"params":{"update":{"sessionUpdate":kind}}});
        let history = vec![
            update("hook_execution"),
            update("user_message_chunk"),
            update("agent_message_chunk"),
            update("turn_completed"),
            update("user_message_chunk"),
            update("agent_message_chunk"),
        ];
        let truncated = truncate_at_turn(&history, 1, is_update_prompt).unwrap();
        assert_eq!(truncated.len(), 4);
    }

    #[test]
    fn rejects_a_checkpoint_beyond_groks_history() {
        let error = truncate_at_turn(
            &[json!({"type":"user","prompt_index":0})],
            2,
            is_chat_prompt,
        )
        .unwrap_err();
        assert!(error.to_string().contains("only 1 native turns"));
    }

    #[test]
    fn reads_groks_generated_title_from_native_metadata() {
        let root = std::env::temp_dir().join(format!("padu-grok-title-{}", Uuid::new_v4()));
        let session_id = Uuid::new_v4().to_string();
        let session = root
            .join("sessions")
            .join("%2Ftmp%2Fproject")
            .join(&session_id);
        fs::create_dir_all(&session).unwrap();
        fs::write(
            session.join("summary.json"),
            serde_json::to_vec(&json!({
                "generated_title": "  Fix provider task titles  ",
                "session_summary": "A much longer summary"
            }))
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            generated_title_in(&root, &session_id).unwrap().as_deref(),
            Some("Fix provider task titles")
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn maps_visible_grok_summaries_and_hides_internal_sessions() {
        let session_id = Uuid::new_v4().to_string();
        let cwd = std::env::temp_dir().join("project");
        let summary = provider_summary(&json!({
            "info": {"id": session_id, "cwd": cwd},
            "generated_title": "Resume Grok",
            "session_summary": "fallback",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:01Z",
            "last_active_at": "2026-01-01T00:00:02Z"
        }))
        .unwrap();
        assert_eq!(summary.title, "Resume Grok");
        assert_eq!(summary.cwd, cwd);
        assert!(summary.updated_at > summary.created_at);

        for value in [
            json!({"info":{"id":Uuid::new_v4(),"cwd":std::env::temp_dir()},"session_kind":"headless"}),
            json!({"info":{"id":Uuid::new_v4(),"cwd":std::env::temp_dir()},"session_kind":"subagent_fork"}),
            json!({"info":{"id":Uuid::new_v4(),"cwd":std::env::temp_dir()},"hidden":true}),
        ] {
            assert!(provider_summary(&value).is_none());
        }
    }

    fn model_state() -> Value {
        json!({
            "result": {
                "_meta": {
                    "modelState": {
                        "currentModelId": "grok-4.7",
                        "availableModels": [
                            {
                                "modelId": "grok-4.7",
                                "name": "Grok 4.7",
                                "description": "SpaceXAI's latest frontier model",
                                "_meta": {
                                    "supportsReasoningEffort": true,
                                    "reasoningEffort": "high",
                                    "reasoningEfforts": [
                                        {
                                            "id": "xhigh",
                                            "label": "Extra High",
                                            "description": "Maximum reasoning for the hardest tasks.",
                                            "default": false
                                        },
                                        {
                                            "id": "high",
                                            "label": "High",
                                            "description": "Recommended.",
                                            "default": true
                                        },
                                        {"id": "medium", "label": "Medium", "default": false},
                                        {"id": "low", "label": "Low", "default": false}
                                    ]
                                }
                            },
                            {
                                "modelId": "grok-legacy",
                                "name": "Grok Legacy",
                                "_meta": {
                                    "supportsReasoningEffort": false,
                                    "reasoningEffort": "high"
                                }
                            },
                            {
                                "modelId": "grok-unlabeled",
                                "_meta": {
                                    "supportsReasoningEffort": true,
                                    "reasoningEffort": "medium",
                                    "reasoningEfforts": [{"value": "medium"}, {"value": "low"}]
                                }
                            }
                        ]
                    }
                }
            }
        })
    }

    #[test]
    fn parses_grok_reasoning_menu_and_defaults_from_acp_model_state() {
        let models = parse_model_state(&model_state());
        assert_eq!(models.len(), 3);

        let default = &models[0];
        assert_eq!(default.id, "grok-4.7");
        assert_eq!(default.name, "Grok 4.7");
        assert!(default.is_default);
        // The agent lists efforts highest-first; Padu renders them ascending.
        assert_eq!(
            default
                .reasoning_efforts
                .iter()
                .map(|option| option.id.as_str())
                .collect::<Vec<_>>(),
            ["low", "medium", "high", "xhigh"]
        );
        assert_eq!(default.default_reasoning_effort.as_deref(), Some("high"));
        assert_eq!(
            default.reasoning_efforts[3].description.as_deref(),
            Some("Maximum reasoning for the hardest tasks.")
        );
    }

    #[test]
    fn grok_reasoning_menu_falls_back_to_the_current_effort() {
        let models = parse_model_state(&model_state());

        // No `default: true` marker, so the agent's current effort wins.
        let unlabeled = &models[2];
        assert_eq!(unlabeled.name, "Grok Unlabeled");
        assert_eq!(
            unlabeled
                .reasoning_efforts
                .iter()
                .map(|option| option.id.as_str())
                .collect::<Vec<_>>(),
            ["low", "medium"]
        );
        assert_eq!(
            unlabeled.default_reasoning_effort.as_deref(),
            Some("medium")
        );

        // A model that opts out of effort gets no menu at all.
        let legacy = &models[1];
        assert!(legacy.reasoning_efforts.is_empty());
        assert_eq!(legacy.default_reasoning_effort, None);
    }
}
