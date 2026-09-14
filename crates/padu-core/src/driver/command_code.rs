//! Command Code's headless NDJSON transport.
//!
//! Command Code does not currently expose an ACP server. Its supported machine
//! interface is one `--print --output-format json` invocation per turn, with a
//! persisted session id used to resume the conversation.

use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use anyhow::{Context as _, anyhow};
use crossbeam_channel::{Receiver, Sender, unbounded};
use parking_lot::Mutex;
use serde_json::Value;

use super::{DriverControl, DriverEventSender, DriverStartOptions, SessionOptions};
use crate::model::{ActivityKind, DriverEvent, InteractionMode, ProviderResumeCursor, RuntimeMode};

#[derive(Debug)]
enum CommandMessage {
    Prompt(String),
    Options(SessionOptions),
    Shutdown,
}

#[derive(Clone, Debug)]
struct CommandCodeOptions {
    cwd: std::path::PathBuf,
    model: Option<String>,
    reasoning_effort: Option<String>,
    mode: RuntimeMode,
    interaction_mode: InteractionMode,
    session_id: Option<String>,
}

pub struct CommandCodeDriver {
    commands: Sender<CommandMessage>,
    active_child: Arc<Mutex<Option<Child>>>,
    worker: Option<JoinHandle<()>>,
    options: Arc<Mutex<CommandCodeOptions>>,
}

fn command_code_args(options: &CommandCodeOptions, prompt: &str) -> Vec<String> {
    let mut args = vec!["--print".into(), "--output-format".into(), "json".into()];
    if let Some(session_id) = options.session_id.as_deref() {
        args.extend(["--resume".into(), session_id.into()]);
    }
    if let Some(model) = options.model.as_deref().filter(|value| !value.is_empty()) {
        args.extend(["--model".into(), model.into()]);
    }
    if let Some(effort) = options
        .reasoning_effort
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        args.extend(["--effort".into(), effort.into()]);
    }
    if options.mode == RuntimeMode::Plan || options.interaction_mode == InteractionMode::Plan {
        args.push("--plan".into());
    } else {
        match options.mode {
            RuntimeMode::AutoAcceptEdits => args.push("--auto-accept".into()),
            RuntimeMode::Auto => args.extend(["--permission-mode".into(), "dont-ask".into()]),
            RuntimeMode::FullAccess => args.push("--yolo".into()),
            RuntimeMode::Ask | RuntimeMode::Plan => {}
        }
    }
    args.extend(["--skip-onboarding".into(), "--trust".into(), prompt.into()]);
    args
}

impl CommandCodeDriver {
    pub fn start(options: DriverStartOptions, events: DriverEventSender) -> anyhow::Result<Self> {
        let DriverStartOptions {
            binary,
            cwd,
            mode,
            interaction_mode,
            model,
            reasoning_effort,
            provider_cursor,
            service_tier: _,
            context_window: _,
            agent_preset: _,
            computer_use_enabled: _,
        } = options;
        let session_id = match provider_cursor {
            Some(ProviderResumeCursor::CommandCode { session_id }) => Some(session_id),
            Some(cursor) => {
                return Err(anyhow!(
                    "cannot resume Command Code from a {} cursor",
                    cursor.provider().display_name()
                ));
            }
            None => None,
        };
        let state = Arc::new(Mutex::new(CommandCodeOptions {
            cwd,
            model,
            reasoning_effort,
            mode,
            interaction_mode,
            session_id,
        }));
        let active_child = Arc::new(Mutex::new(None));
        let (commands, receiver) = unbounded();
        let worker_state = state.clone();
        let worker_child = active_child.clone();
        let worker = thread::Builder::new()
            .name("padu-command-code-worker".into())
            .spawn(move || worker_loop(binary, receiver, worker_state, worker_child, events))?;
        Ok(Self {
            commands,
            active_child,
            worker: Some(worker),
            options: state,
        })
    }
}

fn worker_loop(
    binary: std::path::PathBuf,
    receiver: Receiver<CommandMessage>,
    options: Arc<Mutex<CommandCodeOptions>>,
    active_child: Arc<Mutex<Option<Child>>>,
    events: DriverEventSender,
) {
    while let Ok(message) = receiver.recv() {
        match message {
            CommandMessage::Prompt(prompt) => {
                let current = options.lock().clone();
                let _ = events.send(DriverEvent::TurnStarted);
                let mut current = current;
                if let Err(error) = run_turn(&binary, &mut current, &active_child, &events, &prompt)
                {
                    let _ = events.send(DriverEvent::Error(error.to_string()));
                    let _ = events.send(DriverEvent::TurnFinished {
                        success: false,
                        summary: Some(error.to_string()),
                    });
                }
                options.lock().session_id = current.session_id;
            }
            CommandMessage::Options(next) => {
                let mut current = options.lock();
                current.model = next.model;
                current.reasoning_effort = next.reasoning_effort;
                current.mode = next.mode;
                current.interaction_mode = next.interaction_mode;
            }
            CommandMessage::Shutdown => break,
        }
    }
}

fn run_turn(
    binary: &std::path::Path,
    options: &mut CommandCodeOptions,
    active_child: &Arc<Mutex<Option<Child>>>,
    events: &DriverEventSender,
    prompt: &str,
) -> anyhow::Result<()> {
    let mut command = crate::command_env::command(binary);
    command
        .current_dir(&options.cwd)
        .args(command_code_args(options, prompt))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = crate::command_env::spawn(&mut command).context("failed to start Command Code")?;
    *active_child.lock() = Some(child);
    let (stdout, stderr) = {
        let mut active = active_child.lock();
        let child = active
            .as_mut()
            .context("Command Code child process unavailable")?;
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                drop(active);
                terminate_and_reap_child(active_child);
                anyhow::bail!("Command Code stdout unavailable");
            }
        };
        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                drop(active);
                terminate_and_reap_child(active_child);
                anyhow::bail!("Command Code stderr unavailable");
            }
        };
        (stdout, stderr)
    };
    let stderr_thread = thread::spawn(move || {
        BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
    });
    let mut completed = false;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if handle_frame(&value, events, options) {
            completed = true;
        }
    }
    let mut child = active_child
        .lock()
        .take()
        .context("Command Code child process unavailable")?;
    let status = child.wait().context("waiting for Command Code")?;
    let stderr = stderr_thread.join().unwrap_or_default();
    if !status.success() && !completed {
        let detail = stderr.last().cloned().unwrap_or_else(|| status.to_string());
        anyhow::bail!("Command Code failed: {detail}");
    }
    if !completed {
        anyhow::bail!("Command Code exited without a result");
    }
    Ok(())
}

fn terminate_and_reap_child(active_child: &Arc<Mutex<Option<Child>>>) {
    let mut child = active_child.lock().take();
    if let Some(child) = child.as_mut() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn handle_frame(
    value: &Value,
    events: &DriverEventSender,
    options: &mut CommandCodeOptions,
) -> bool {
    if value.get("type").and_then(Value::as_str) == Some("event") {
        let event = value.get("event").unwrap_or(value);
        let event_type = event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if event_type.contains("tool") {
            let title = event
                .get("toolName")
                .or_else(|| event.get("description"))
                .and_then(Value::as_str)
                .unwrap_or("Command Code tool")
                .to_owned();
            let _ = events.send(DriverEvent::Activity {
                id: event
                    .get("toolCallId")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                kind: ActivityKind::Tool,
                title,
                detail: None,
                complete: matches!(
                    event_type,
                    "tool_completed" | "tool_errored" | "tool_denied" | "tool_hook_blocked"
                ),
            });
        }
        return false;
    }
    if value.get("type").and_then(Value::as_str) != Some("result") {
        return false;
    }
    if let Some(session_id) = value.get("sessionId").and_then(Value::as_str) {
        options.session_id = Some(session_id.to_owned());
        let _ = events.send(DriverEvent::Connected {
            provider_cursor: Some(ProviderResumeCursor::CommandCode {
                session_id: session_id.to_owned(),
            }),
        });
    }
    if let Some(text) = value
        .get("finalText")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        let _ = events.send(DriverEvent::TextDelta(text.to_owned()));
    }
    let success = value.get("subtype").and_then(Value::as_str) == Some("success");
    if !success {
        let message = value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Command Code returned an error")
            .to_owned();
        let _ = events.send(DriverEvent::Error(message.clone()));
    }
    let _ = options;
    let _ = events.send(DriverEvent::TurnFinished {
        success,
        summary: None,
    });
    true
}

impl DriverControl for CommandCodeDriver {
    fn prompt(&self, prompt: String) {
        let _ = self.commands.send(CommandMessage::Prompt(prompt));
    }

    fn cancel(&self) {
        if let Some(child) = self.active_child.lock().as_mut() {
            let _ = child.kill();
        }
    }

    fn respond(&self, _request_id: String, _option_id: String) {}

    fn apply_options(&self, options: SessionOptions) -> bool {
        let _ = self.commands.send(CommandMessage::Options(options.clone()));
        let mut current = self.options.lock();
        current.model = options.model;
        current.reasoning_effort = options.reasoning_effort;
        current.mode = options.mode;
        current.interaction_mode = options.interaction_mode;
        true
    }

    fn rollback(&self, _turns: usize) -> anyhow::Result<Option<ProviderResumeCursor>> {
        Err(anyhow!(
            "conversation rollback is not supported by Command Code yet"
        ))
    }
}

impl Drop for CommandCodeDriver {
    fn drop(&mut self) {
        if let Some(child) = self.active_child.lock().as_mut() {
            let _ = child.kill();
        }
        let _ = self.commands.send(CommandMessage::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> CommandCodeOptions {
        CommandCodeOptions {
            cwd: "/tmp".into(),
            model: Some("deepseek/deepseek-v4-flash".into()),
            reasoning_effort: Some("high".into()),
            mode: RuntimeMode::FullAccess,
            interaction_mode: InteractionMode::Build,
            session_id: Some("session-1".into()),
        }
    }

    #[test]
    fn args_resume_and_pin_model() {
        let args = command_code_args(&options(), "Reply with PONG");
        assert_eq!(
            args[0..7],
            [
                "--print",
                "--output-format",
                "json",
                "--resume",
                "session-1",
                "--model",
                "deepseek/deepseek-v4-flash"
            ]
        );
        assert!(args.contains(&"--effort".into()));
        assert!(args.contains(&"--yolo".into()));
        assert_eq!(args.last().unwrap(), "Reply with PONG");
    }

    #[test]
    fn auto_mode_uses_permission_mode_without_yolo() {
        let mut options = options();
        options.mode = RuntimeMode::Auto;
        let args = command_code_args(&options, "Reply with PONG");

        assert!(
            args.windows(2)
                .any(|window| window == ["--permission-mode", "dont-ask"])
        );
        assert!(!args.contains(&"--yolo".into()));
    }

    #[test]
    fn result_frame_is_successful() {
        let (events, received) = super::super::test_event_channel();
        let value = serde_json::json!({
            "type": "result",
            "subtype": "success",
            "sessionId": "abc",
            "finalText": "done"
        });
        let mut options = options();
        assert!(handle_frame(&value, &events, &mut options));
        assert_eq!(options.session_id.as_deref(), Some("abc"));
        let values = received.try_iter().collect::<Vec<_>>();
        assert!(
            values
                .iter()
                .any(|event| matches!(event, DriverEvent::TextDelta(text) if text == "done"))
        );
        assert!(
            values
                .iter()
                .any(|event| matches!(event, DriverEvent::TurnFinished { success: true, .. }))
        );
    }
}
