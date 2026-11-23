//! OpenCode v2's `/api/*` transport.
//!
//! v2 is a distinct provider from the legacy OpenCode server: routes, payloads,
//! events, session model, permissions, and forms all have different contracts.
//! Keeping this driver separate means a protocol change in either generation
//! cannot silently reinterpret the other generation's session.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use anyhow::{Context as _, anyhow};
use crossbeam_channel::{Sender, unbounded};
use parking_lot::Mutex;
use serde_json::{Map, Value, json};

use super::activity;
use crate::driver::{
    DriverControl, DriverEventSender, DriverEventSink, DriverStartOptions, SessionOptions,
};
use crate::model::{
    ActivityKind, ContextWindowSource, DriverEvent, InteractionMode, PermissionOption,
    ProviderResumeCursor, RuntimeMode, UserInputAnswer, UserInputOption, UserInputQuestion,
};
use crate::opencode_pool::PooledServer;
use crate::opencode_session::{OpenCodeServer, encode_path_segment};

enum CommandMessage {
    Prompt(String),
    Steer(String),
    Cancel,
    Respond {
        request_id: String,
        option_id: String,
    },
    RespondUserInput {
        request_id: String,
        answers: Vec<UserInputAnswer>,
    },
    Shutdown,
}

fn model_ref(model: Option<&str>, reasoning_effort: Option<&str>) -> Option<Value> {
    let (provider, model) = model?.split_once('/')?;
    let mut value = json!({"providerID": provider, "id": model});
    if let Some(effort) = reasoning_effort.filter(|effort| !effort.trim().is_empty()) {
        value["variant"] = Value::String(effort.to_owned());
    }
    Some(value)
}

fn permission_ruleset(mode: RuntimeMode, interaction_mode: InteractionMode) -> Value {
    let plan = interaction_mode == InteractionMode::Plan || mode == RuntimeMode::Plan;
    let rule =
        |action: &str, effect: &str| json!({"action": action, "resource": "*", "effect": effect});
    match mode {
        RuntimeMode::Plan | RuntimeMode::Ask => {
            let mut rules = vec![rule("bash", "ask")];
            if !plan {
                rules.push(rule("edit", "ask"));
            }
            Value::Array(rules)
        }
        RuntimeMode::AutoAcceptEdits => {
            let mut rules = vec![rule("bash", "ask")];
            if !plan {
                rules.push(rule("edit", "allow"));
            }
            Value::Array(rules)
        }
        RuntimeMode::Auto | RuntimeMode::FullAccess if plan => {
            Value::Array(vec![rule("bash", "allow")])
        }
        RuntimeMode::Auto | RuntimeMode::FullAccess => Value::Array(vec![rule("*", "allow")]),
    }
}

fn prompt_body(text: &str, delivery: Option<&str>) -> Value {
    let mut body = json!({"text": text});
    if let Some(delivery) = delivery {
        body["delivery"] = Value::String(delivery.to_owned());
    }
    body
}

fn context_tokens(tokens: &Value) -> Option<u64> {
    let total = ["input", "output", "reasoning"]
        .into_iter()
        .filter_map(|key| tokens.get(key).and_then(Value::as_u64))
        .chain(
            [
                tokens.pointer("/cache/read").and_then(Value::as_u64),
                tokens.pointer("/cache/write").and_then(Value::as_u64),
            ]
            .into_iter()
            .flatten(),
        )
        .fold(0_u64, u64::saturating_add);
    (total > 0).then_some(total)
}

fn model_key(value: &Value) -> Option<String> {
    let provider = value.get("providerID").and_then(Value::as_str)?;
    let model = value
        .get("modelID")
        .or_else(|| value.get("id"))
        .or_else(|| value.pointer("/model/id"))
        .and_then(Value::as_str)?;
    Some(format!("{provider}/{model}"))
}

fn stream_part_key(data: &Value, kind: &str) -> Option<String> {
    data.get("assistantMessageID")
        .or_else(|| data.get("messageID"))
        .and_then(Value::as_str)
        .map(|message| {
            let ordinal = data.get("ordinal").and_then(Value::as_u64).unwrap_or(0);
            format!("{message}:{kind}:{ordinal}")
        })
        .or_else(|| {
            data.get("partID")
                .or_else(|| data.get("id"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
}

fn latest_context_usage(response: &Value) -> Option<(u64, Option<String>)> {
    response
        .get("data")
        .and_then(Value::as_array)?
        .iter()
        .rev()
        .filter(|message| message.get("type").and_then(Value::as_str) == Some("assistant"))
        .find_map(|message| {
            let tokens = message.get("tokens").and_then(context_tokens)?;
            Some((tokens, model_key(message)))
        })
}

fn model_windows(response: &Value) -> HashMap<String, u64> {
    response
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| {
            let key = model_key(model)?;
            let window = model
                .pointer("/limit/context")
                .and_then(Value::as_u64)
                .filter(|window| *window > 0)?;
            Some((key, window))
        })
        .collect()
}

#[derive(Default)]
struct UsageMetadata {
    windows: Mutex<HashMap<String, u64>>,
    last_model: Mutex<Option<String>>,
}

impl UsageMetadata {
    fn current_window(&self) -> Option<u64> {
        let model = self.last_model.lock().clone()?;
        self.windows.lock().get(&model).copied()
    }
}

pub struct OpenCodeV2Driver {
    server: Option<PooledServer>,
    session_id: String,
    commands: Sender<CommandMessage>,
    event_stream: Arc<OpenCodeEventStreamControl>,
    mode: RuntimeMode,
    interaction_mode: InteractionMode,
    model: Mutex<Option<String>>,
    reasoning_effort: Mutex<Option<String>>,
    computer_use: Option<super::support::HeadlessComputerUseRuntime>,
}

impl OpenCodeV2Driver {
    pub fn start(options: DriverStartOptions, events: DriverEventSender) -> anyhow::Result<Self> {
        let DriverStartOptions {
            binary,
            cwd,
            mode,
            interaction_mode,
            model,
            reasoning_effort,
            service_tier: _,
            context_window: _,
            agent_preset: _,
            computer_use_enabled,
            provider_cursor,
        } = options;
        let resume_session_id = match provider_cursor {
            Some(ProviderResumeCursor::OpenCodeV2 { session_id }) => {
                (!session_id.is_empty()).then_some(session_id)
            }
            Some(cursor) => {
                return Err(anyhow!(
                    "cannot resume OpenCode v2 from a {} cursor",
                    cursor.provider().display_name()
                ));
            }
            None => None,
        };

        let computer_use = computer_use_enabled
            .then(|| {
                super::support::HeadlessComputerUseRuntime::start(
                    crate::model::ProviderKind::OpenCodeV2,
                    events.clone(),
                )
            })
            .transpose()?;
        let environment = computer_use
            .as_ref()
            .map(|runtime| super::support::opencode_computer_use_environment(&runtime.config))
            .unwrap_or_default();
        let server = if environment.is_empty() {
            crate::opencode_pool::acquire_v2(&binary, &cwd)?
        } else {
            PooledServer::dedicated(OpenCodeServer::start_v2_with_env(
                &binary,
                &cwd,
                &environment,
            )?)
        };

        let agent = if interaction_mode == InteractionMode::Plan || mode == RuntimeMode::Plan {
            "plan"
        } else {
            "build"
        };
        let session_id = match resume_session_id {
            Some(session_id) => session_id,
            None => {
                let mut body = json!({
                    "agent": agent,
                    "permissions": permission_ruleset(mode, interaction_mode),
                });
                if let Some(model_ref) = model_ref(model.as_deref(), reasoning_effort.as_deref()) {
                    body["model"] = model_ref;
                }
                let created = server
                    .request("POST", "/api/session", Some(&body))
                    .context("could not create an OpenCode v2 session")?;
                created
                    .pointer("/data/id")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| anyhow!("OpenCode v2 returned no session ID"))?
            }
        };

        let _ = events.send(DriverEvent::Connected {
            provider_cursor: Some(ProviderResumeCursor::OpenCodeV2 {
                session_id: session_id.clone(),
            }),
        });

        let usage_metadata = Arc::new(UsageMetadata::default());
        if let Some(model) = model.as_ref() {
            *usage_metadata.last_model.lock() = Some(model.clone());
        }
        let metadata_port = server.port;
        let metadata_events = events.clone();
        let metadata_usage = usage_metadata.clone();
        thread::Builder::new()
            .name("padu-opencode-v2-models".into())
            .spawn(move || {
                let Ok(response) = crate::opencode_session::request_json_on_port(
                    metadata_port,
                    "GET",
                    "/api/model",
                    None,
                    Duration::from_secs(30),
                ) else {
                    return;
                };
                *metadata_usage.windows.lock() = model_windows(&response);
                if let Some(window) = metadata_usage.current_window() {
                    let _ = metadata_events.send(DriverEvent::UsageUpdated {
                        context_tokens: None,
                        context_window: Some(window),
                        context_window_source: Some(ContextWindowSource::Provider),
                    });
                }
            })?;

        let (commands, command_rx) = unbounded();
        let turn_active = Arc::new(Mutex::new(false));
        let forms = Arc::new(Mutex::new(HashMap::<String, Vec<Value>>::new()));
        let event_stream = Arc::new(OpenCodeEventStreamControl::default());
        let auto_approve = matches!(mode, RuntimeMode::Auto | RuntimeMode::FullAccess);

        let stream_port = server.port;
        let stream_session = session_id.clone();
        let stream_events = events.clone();
        let stream_commands = commands.clone();
        let stream_turn = turn_active.clone();
        let stream_usage = usage_metadata;
        let stream_forms = forms.clone();
        let stream_control = event_stream.clone();
        thread::Builder::new()
            .name("padu-opencode-v2-events".into())
            .spawn(move || {
                let mut state = OpenCodeV2StreamState::default();
                let mut reconnect_failures = 0_u32;
                let mut first_stream = true;
                loop {
                    if stream_control.is_cancelled() {
                        break;
                    }
                    match open_event_stream(stream_port, "/api/event", stream_control.as_ref()) {
                        Ok(Some(stream)) => {
                            refresh_v2_usage(
                                stream_port,
                                &stream_session,
                                &stream_events,
                                &stream_usage,
                            );
                            let active = rehydrate_v2_active_session(
                                stream_port,
                                &stream_session,
                                &stream_events,
                                &stream_turn,
                                &mut state,
                            );
                            rehydrate_v2_messages(
                                stream_port,
                                &stream_session,
                                &stream_events,
                                &stream_commands,
                                &stream_turn,
                                &stream_usage,
                                &stream_forms,
                                auto_approve,
                                &mut state,
                                !first_stream,
                            );
                            if active != Some(true) {
                                rehydrate_v2_execution_state(
                                    stream_port,
                                    &stream_session,
                                    &stream_events,
                                    &stream_turn,
                                    &mut state,
                                    !first_stream,
                                );
                            }
                            first_stream = false;
                            rehydrate_v2_prompts(
                                stream_port,
                                &stream_session,
                                &stream_events,
                                &stream_commands,
                                &stream_forms,
                                auto_approve,
                                &mut state,
                            );
                            let mut received_event = false;
                            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                                if stream_control.is_cancelled() {
                                    break;
                                }
                                let Some(payload) = line.strip_prefix("data:") else {
                                    continue;
                                };
                                let Ok(value) = serde_json::from_str::<Value>(payload.trim())
                                else {
                                    continue;
                                };
                                let event_session =
                                    value.pointer("/data/sessionID").and_then(Value::as_str);
                                if event_session.is_some_and(|session| session != stream_session) {
                                    continue;
                                }
                                handle_v2_event(
                                    &value,
                                    &stream_events,
                                    &stream_commands,
                                    &stream_turn,
                                    &stream_usage,
                                    &stream_forms,
                                    auto_approve,
                                    &mut state,
                                );
                                if !received_event {
                                    received_event = true;
                                    reconnect_failures = 0;
                                }
                            }
                            if !received_event {
                                reconnect_failures = reconnect_failures.saturating_add(1);
                                if reconnect_failures >= 5 {
                                    let _ = stream_events.send(DriverEvent::Error(tr!(
                                        "errors.read_provider_event_stream",
                                        provider = "OpenCode v2",
                                        error = "event stream closed before delivering an event"
                                    )));
                                    let _ = stream_events.send(DriverEvent::ProcessExited);
                                    break;
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(error) => {
                            reconnect_failures = reconnect_failures.saturating_add(1);
                            if reconnect_failures >= 5 {
                                let _ = stream_events.send(DriverEvent::Error(tr!(
                                    "errors.read_provider_event_stream",
                                    provider = "OpenCode v2",
                                    error = error
                                )));
                                let _ = stream_events.send(DriverEvent::ProcessExited);
                                break;
                            }
                        }
                    }
                    if stream_control.is_cancelled() {
                        break;
                    }
                    refresh_v2_usage(stream_port, &stream_session, &stream_events, &stream_usage);
                    thread::sleep(Duration::from_millis(
                        250_u64.saturating_mul(u64::from(reconnect_failures.max(1))),
                    ));
                }
                stream_control.clear();
            })?;

        let worker_server = server.clone();
        let worker_session = session_id.clone();
        let worker_events = events;
        let worker_turn = turn_active;
        let worker_forms = forms;
        thread::Builder::new()
            .name("padu-opencode-v2-worker".into())
            .spawn(move || {
                while let Ok(message) = command_rx.recv() {
                    match message {
                        CommandMessage::Prompt(text) => {
                            *worker_turn.lock() = true;
                            let _ = worker_events.send(DriverEvent::TurnStarted);
                            let path = format!(
                                "/api/session/{}/prompt",
                                encode_path_segment(&worker_session)
                            );
                            if let Err(error) = worker_server.request(
                                "POST",
                                &path,
                                Some(&prompt_body(&text, None)),
                            ) {
                                let _ = worker_events.send(DriverEvent::Error(tr!(
                                    "errors.provider_rejected_prompt_detail",
                                    provider = "OpenCode v2",
                                    error = error
                                )));
                                if std::mem::take(&mut *worker_turn.lock()) {
                                    let _ = worker_events.send(DriverEvent::TurnFinished {
                                        success: false,
                                        summary: Some(tr!(
                                            "errors.provider_start_turn",
                                            provider = "OpenCode v2"
                                        )),
                                    });
                                }
                            }
                        }
                        CommandMessage::Steer(text) => {
                            if !*worker_turn.lock() {
                                let _ = worker_events.send(DriverEvent::SteerRejected {
                                    message: text,
                                    reason: tr!(
                                        "errors.provider_no_active_turn",
                                        provider = "OpenCode v2"
                                    ),
                                });
                                continue;
                            }
                            let path = format!(
                                "/api/session/{}/prompt",
                                encode_path_segment(&worker_session)
                            );
                            match worker_server.request(
                                "POST",
                                &path,
                                Some(&prompt_body(&text, Some("steer"))),
                            ) {
                                Ok(_) => {
                                    let _ = worker_events
                                        .send(DriverEvent::SteerAccepted { message: text });
                                }
                                Err(error) => {
                                    let _ = worker_events.send(DriverEvent::SteerRejected {
                                        message: text,
                                        reason: tr!(
                                            "errors.provider_rejected_steer",
                                            provider = "OpenCode v2",
                                            error = error
                                        ),
                                    });
                                }
                            }
                        }
                        CommandMessage::Cancel => {
                            let path = format!(
                                "/api/session/{}/interrupt",
                                encode_path_segment(&worker_session)
                            );
                            if let Err(error) = worker_server.request("POST", &path, None) {
                                let _ = worker_events.send(DriverEvent::Error(tr!(
                                    "errors.stop_provider",
                                    provider = "OpenCode v2",
                                    error = error
                                )));
                            }
                        }
                        CommandMessage::Respond {
                            request_id,
                            option_id,
                        } => {
                            let decision = match option_id.as_str() {
                                "once" | "always" | "reject" => option_id.as_str(),
                                "deny" => "reject",
                                _ => "reject",
                            };
                            let path = format!(
                                "/api/session/{}/permission/{}/reply",
                                encode_path_segment(&worker_session),
                                encode_path_segment(&request_id)
                            );
                            if let Err(error) = worker_server.request(
                                "POST",
                                &path,
                                Some(&json!({"decision": decision})),
                            ) {
                                let _ = worker_events.send(DriverEvent::Error(tr!(
                                    "errors.answer_provider_permission",
                                    provider = "OpenCode v2",
                                    error = error
                                )));
                            }
                        }
                        CommandMessage::RespondUserInput {
                            request_id,
                            answers,
                        } => {
                            let fields =
                                worker_forms.lock().remove(&request_id).unwrap_or_default();
                            let answer = form_answer(&fields, answers);
                            let path = format!(
                                "/api/session/{}/form/{}/reply",
                                encode_path_segment(&worker_session),
                                encode_path_segment(&request_id)
                            );
                            if let Err(error) = worker_server.request(
                                "POST",
                                &path,
                                Some(&json!({"answer": answer})),
                            ) {
                                let _ = worker_events.send(DriverEvent::Error(tr!(
                                    "errors.answer_provider_question",
                                    provider = "OpenCode v2",
                                    error = error
                                )));
                            }
                        }
                        CommandMessage::Shutdown => break,
                    }
                }
            })
            .inspect_err(|_| event_stream.cancel())?;

        Ok(Self {
            server: Some(server),
            session_id,
            commands,
            event_stream,
            mode,
            interaction_mode,
            model: Mutex::new(model),
            reasoning_effort: Mutex::new(reasoning_effort),
            computer_use,
        })
    }
}

impl DriverControl for OpenCodeV2Driver {
    fn prompt(&self, prompt: String) {
        let _ = self.commands.send(CommandMessage::Prompt(prompt));
    }

    fn supports_steer(&self) -> bool {
        true
    }

    fn steer(&self, prompt: String) {
        let _ = self.commands.send(CommandMessage::Steer(prompt));
    }

    fn cancel(&self) {
        let _ = self.commands.send(CommandMessage::Cancel);
    }

    fn cancel_computer_use(&self) {
        if let Some(computer_use) = self.computer_use.as_ref() {
            computer_use.stop();
        }
    }

    fn respond(&self, request_id: String, option_id: String) {
        let _ = self.commands.send(CommandMessage::Respond {
            request_id,
            option_id,
        });
    }

    fn respond_user_input(&self, request_id: String, answers: Vec<UserInputAnswer>) {
        let _ = self.commands.send(CommandMessage::RespondUserInput {
            request_id,
            answers,
        });
    }

    fn apply_options(&self, options: SessionOptions) -> bool {
        if options.mode != self.mode
            || options.interaction_mode != self.interaction_mode
            || options.reasoning_effort != *self.reasoning_effort.lock()
        {
            return false;
        }
        if options.model == *self.model.lock() {
            return true;
        }
        let Some(server) = self.server.as_deref() else {
            return false;
        };
        let Some(model) = model_ref(
            options.model.as_deref(),
            options.reasoning_effort.as_deref(),
        ) else {
            return false;
        };
        let path = format!(
            "/api/session/{}/model",
            encode_path_segment(&self.session_id)
        );
        if server
            .request("POST", &path, Some(&json!({"model": model})))
            .is_err()
        {
            return false;
        }
        *self.model.lock() = options.model;
        true
    }

    fn rollback(&self, turns: usize) -> anyhow::Result<Option<ProviderResumeCursor>> {
        if turns == 0 {
            return Ok(None);
        }
        self.fork(turns).map(Some)
    }

    fn fork(&self, turns_to_remove: usize) -> anyhow::Result<ProviderResumeCursor> {
        let server = self
            .server
            .as_deref()
            .ok_or_else(|| anyhow!("OpenCode v2 driver is shutting down"))?;
        fork_session_removing_turns(server, &self.session_id, turns_to_remove)
    }

    fn rename_native(&self, title: &str) -> anyhow::Result<bool> {
        let server = self
            .server
            .as_deref()
            .ok_or_else(|| anyhow!("OpenCode v2 driver is shutting down"))?;
        let path = format!("/api/session/{}", encode_path_segment(&self.session_id));
        server.request("PATCH", &path, Some(&json!({"title": title})))?;
        Ok(true)
    }
}

impl Drop for OpenCodeV2Driver {
    fn drop(&mut self) {
        self.cancel_computer_use();
        self.event_stream.cancel();
        drop(self.server.take());
        let _ = self.commands.send(CommandMessage::Shutdown);
    }
}

#[derive(Default)]
struct OpenCodeEventStreamControl {
    cancelled: AtomicBool,
    socket: Mutex<Option<TcpStream>>,
}

impl OpenCodeEventStreamControl {
    fn attach(&self, stream: &TcpStream) -> std::io::Result<bool> {
        let socket = stream.try_clone()?;
        let mut active = self.socket.lock();
        if self.cancelled.load(Ordering::Acquire) {
            let _ = socket.shutdown(Shutdown::Both);
            return Ok(false);
        }
        *active = Some(socket);
        Ok(true)
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(socket) = self.socket.lock().take() {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }

    fn clear(&self) {
        self.socket.lock().take();
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

fn open_event_stream(
    port: u16,
    path: &str,
    control: &OpenCodeEventStreamControl,
) -> anyhow::Result<Option<TcpStream>> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .with_context(|| format!("could not connect to OpenCode v2 on local port {port}"))?;
    if !control.attach(&stream)? {
        return Ok(None);
    }
    stream.set_read_timeout(Some(Duration::from_millis(100)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: text/event-stream\r\nConnection: keep-alive\r\n\r\n"
    )?;
    stream.flush()?;
    let mut response_head = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => return Err(anyhow!("OpenCode v2 closed the event stream during setup")),
            Ok(_) => {
                response_head.push(byte[0]);
                if response_head.ends_with(b"\r\n\r\n") || response_head.ends_with(b"\n\n") {
                    break;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                if control.is_cancelled() {
                    return Ok(None);
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
    let status = String::from_utf8_lossy(&response_head);
    if !status.starts_with("HTTP/1.1 2") && !status.starts_with("HTTP/1.0 2") {
        return Err(anyhow!(
            "OpenCode v2 rejected the event stream: {}",
            status.lines().next().unwrap_or("unknown status")
        ));
    }
    stream.set_read_timeout(None)?;
    Ok(Some(stream))
}

#[derive(Default)]
struct OpenCodeV2StreamState {
    tools: HashMap<String, (ActivityKind, String)>,
    completed_tools: HashSet<String>,
    text_parts: HashMap<String, String>,
    reasoning_parts: HashMap<String, String>,
    part_watermarks: HashMap<String, u64>,
    permissions: HashSet<String>,
    session_idle: bool,
}

fn refresh_v2_usage(
    port: u16,
    session_id: &str,
    events: &impl DriverEventSink,
    usage: &UsageMetadata,
) {
    let path = format!("/api/session/{}/context", encode_path_segment(session_id));
    let Ok(response) = crate::opencode_session::request_json_on_port(
        port,
        "GET",
        &path,
        None,
        Duration::from_secs(5),
    ) else {
        return;
    };
    if let Some((tokens, model)) = latest_context_usage(&response) {
        if let Some(model) = model {
            *usage.last_model.lock() = Some(model);
        }
        let window = usage.current_window();
        let _ = events.send(DriverEvent::UsageUpdated {
            context_tokens: Some(tokens),
            context_window: window,
            context_window_source: window.map(|_| ContextWindowSource::Provider),
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn rehydrate_v2_messages(
    port: u16,
    session_id: &str,
    events: &impl DriverEventSink,
    commands: &Sender<CommandMessage>,
    turn_active: &Mutex<bool>,
    usage: &UsageMetadata,
    forms: &Mutex<HashMap<String, Vec<Value>>>,
    auto_approve: bool,
    state: &mut OpenCodeV2StreamState,
    emit_missing: bool,
) {
    let path = format!("/api/session/{}/message", encode_path_segment(session_id));
    let Ok(response) = crate::opencode_session::request_json_on_port(
        port,
        "GET",
        &path,
        None,
        Duration::from_secs(10),
    ) else {
        return;
    };
    let Some(messages) = response.get("data").and_then(Value::as_array) else {
        return;
    };
    for message in messages {
        if message.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let message_id = message
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("assistant");
        let streamed_at = message
            .pointer("/time/streamed")
            .and_then(Value::as_u64)
            .or_else(|| message.pointer("/time/completed").and_then(Value::as_u64));
        let Some(parts) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        let mut ordinals = HashMap::<&str, u64>::new();
        for part in parts {
            match part.get("type").and_then(Value::as_str) {
                Some(kind @ ("text" | "reasoning")) => {
                    let Some(text) = part.get("text").and_then(Value::as_str) else {
                        continue;
                    };
                    let ordinal = ordinals.entry(kind).or_default();
                    let key = format!("{message_id}:{kind}:{ordinal}");
                    *ordinal += 1;
                    if let Some(streamed_at) = streamed_at {
                        state
                            .part_watermarks
                            .entry(key.clone())
                            .and_modify(|watermark| *watermark = (*watermark).max(streamed_at))
                            .or_insert(streamed_at);
                    }
                    let seen = if kind == "text" {
                        state.text_parts.get(&key)
                    } else {
                        state.reasoning_parts.get(&key)
                    }
                    .cloned()
                    .unwrap_or_default();
                    if emit_missing && let Some(delta) = unseen_suffix(&seen, text) {
                        let _ = if kind == "text" {
                            events.send(DriverEvent::TextDelta(delta.to_owned()))
                        } else {
                            events.send(DriverEvent::ReasoningDelta(delta.to_owned()))
                        };
                    }
                    if !seen.starts_with(text) {
                        if kind == "text" {
                            state.text_parts.insert(key, text.to_owned());
                        } else {
                            state.reasoning_parts.insert(key, text.to_owned());
                        }
                    }
                }
                Some("tool") => rehydrate_v2_tool_part(
                    part,
                    session_id,
                    events,
                    commands,
                    turn_active,
                    usage,
                    forms,
                    auto_approve,
                    state,
                    emit_missing,
                ),
                _ => {}
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn rehydrate_v2_tool_part(
    part: &Value,
    session_id: &str,
    events: &impl DriverEventSink,
    commands: &Sender<CommandMessage>,
    turn_active: &Mutex<bool>,
    usage: &UsageMetadata,
    forms: &Mutex<HashMap<String, Vec<Value>>>,
    auto_approve: bool,
    state: &mut OpenCodeV2StreamState,
    emit_missing: bool,
) {
    let id = part
        .get("toolCallID")
        .or_else(|| part.get("callID"))
        .or_else(|| part.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let status = part
        .pointer("/state/status")
        .or_else(|| part.get("status"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let event_kind = if matches!(status, "completed" | "succeeded" | "success") {
        Some("session.tool.success")
    } else if matches!(status, "failed" | "error" | "interrupted" | "cancelled") {
        Some("session.tool.failed")
    } else if matches!(status, "running" | "pending" | "called") {
        Some("session.tool.called")
    } else {
        None
    };
    let Some(event_kind) = event_kind else {
        return;
    };
    if !emit_missing && event_kind != "session.tool.called" {
        if let Some(id) = id {
            state.completed_tools.insert(id);
        }
        return;
    }
    if !emit_missing && let Some(id) = id {
        let wire_title = part
            .get("name")
            .or_else(|| part.get("tool"))
            .and_then(Value::as_str)
            .unwrap_or("tool");
        let input = part
            .get("input")
            .or_else(|| part.get("arguments"))
            .or_else(|| part.get("parameters"))
            .or_else(|| part.pointer("/state/input"));
        let activity_kind = super::support::classify_tool(wire_title);
        let title = activity::input_title(input).unwrap_or_else(|| wire_title.to_owned());
        state.tools.insert(id, (activity_kind, title));
        return;
    }
    handle_v2_event(
        &json!({
            "type": event_kind,
            "data": {"sessionID": session_id, "tool": part},
        }),
        events,
        commands,
        turn_active,
        usage,
        forms,
        auto_approve,
        state,
    );
}

fn unseen_suffix<'a>(seen: &str, complete: &'a str) -> Option<&'a str> {
    let common = seen
        .chars()
        .zip(complete.chars())
        .take_while(|(left, right)| left == right)
        .count();
    let byte = complete
        .char_indices()
        .nth(common)
        .map(|(index, _)| index)
        .unwrap_or(complete.len());
    (byte < complete.len()).then(|| &complete[byte..])
}

fn rehydrate_v2_active_session(
    port: u16,
    session_id: &str,
    events: &impl DriverEventSink,
    turn_active: &Mutex<bool>,
    state: &mut OpenCodeV2StreamState,
) -> Option<bool> {
    let response = crate::opencode_session::request_json_on_port(
        port,
        "GET",
        "/api/session/active",
        None,
        Duration::from_secs(5),
    )
    .ok()?;
    let running = response
        .pointer("/data")
        .and_then(Value::as_object)
        .and_then(|sessions| sessions.get(session_id))
        .and_then(|active| active.get("type"))
        .and_then(Value::as_str)
        == Some("running");
    if running {
        state.session_idle = false;
        let mut active = turn_active.lock();
        if !*active {
            *active = true;
            drop(active);
            let _ = events.send(DriverEvent::TurnStarted);
        }
    }
    Some(running)
}

fn rehydrate_v2_execution_state(
    port: u16,
    session_id: &str,
    events: &impl DriverEventSink,
    turn_active: &Mutex<bool>,
    state: &mut OpenCodeV2StreamState,
    apply: bool,
) {
    if !apply {
        return;
    }
    let path = format!("/api/session/{}", encode_path_segment(session_id));
    let Ok(response) = crate::opencode_session::request_json_on_port(
        port,
        "GET",
        &path,
        None,
        Duration::from_secs(5),
    ) else {
        return;
    };
    let data = response.get("data").unwrap_or(&Value::Null);
    let idle_at = data.pointer("/time/idle").and_then(Value::as_u64);
    let updated_at = data.pointer("/time/updated").and_then(Value::as_u64);
    let reached_idle = idle_at.is_some_and(|idle| updated_at.is_none_or(|updated| idle >= updated));
    let status = data.get("outcome").and_then(Value::as_str).or_else(|| {
        [
            "/status",
            "/state/status",
            "/execution/status",
            "/status/type",
        ]
        .into_iter()
        .find_map(|pointer| data.pointer(pointer).and_then(Value::as_str))
    });
    match status {
        Some("running" | "busy" | "working" | "started") => {
            state.session_idle = false;
        }
        Some(
            "idle" | "completed" | "succeeded" | "failed" | "interrupted" | "cancelled"
            | "canceled",
        ) if reached_idle => {
            state.session_idle = true;
            if std::mem::take(&mut *turn_active.lock()) {
                let success = matches!(status, Some("idle" | "completed" | "succeeded"));
                let _ = events.send(DriverEvent::TurnFinished {
                    success,
                    summary: None,
                });
            }
        }
        _ => {}
    }
}

fn rehydrate_v2_prompts(
    port: u16,
    session_id: &str,
    events: &impl DriverEventSink,
    commands: &Sender<CommandMessage>,
    forms: &Mutex<HashMap<String, Vec<Value>>>,
    auto_approve: bool,
    state: &mut OpenCodeV2StreamState,
) {
    let permission_path = format!(
        "/api/session/{}/permission",
        encode_path_segment(session_id)
    );
    if let Ok(response) = crate::opencode_session::request_json_on_port(
        port,
        "GET",
        &permission_path,
        None,
        Duration::from_secs(5),
    ) {
        for request in response
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            request_permission(request, events, commands, auto_approve, state);
        }
    }
    let form_path = format!("/api/session/{}/form", encode_path_segment(session_id));
    if let Ok(response) = crate::opencode_session::request_json_on_port(
        port,
        "GET",
        &form_path,
        None,
        Duration::from_secs(5),
    ) {
        for form in response
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            request_form(form, events, forms);
        }
    }
}

fn handle_v2_event(
    value: &Value,
    events: &impl DriverEventSink,
    commands: &Sender<CommandMessage>,
    turn_active: &Mutex<bool>,
    usage: &UsageMetadata,
    forms: &Mutex<HashMap<String, Vec<Value>>>,
    auto_approve: bool,
    state: &mut OpenCodeV2StreamState,
) {
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let data = value.get("data").unwrap_or(&Value::Null);
    match kind {
        "session.text.started" | "session.reasoning.started" => {
            let part_kind = if kind == "session.text.started" {
                "text"
            } else {
                "reasoning"
            };
            if let Some(key) = stream_part_key(data, part_kind) {
                let parts = if part_kind == "text" {
                    &mut state.text_parts
                } else {
                    &mut state.reasoning_parts
                };
                parts.entry(key).or_default();
            }
        }
        "session.text.delta" | "session.reasoning.delta" => {
            let part_kind = if kind == "session.text.delta" {
                "text"
            } else {
                "reasoning"
            };
            let Some(key) = stream_part_key(data, part_kind) else {
                return;
            };
            if value
                .get("created")
                .and_then(Value::as_u64)
                .is_some_and(|created| {
                    state
                        .part_watermarks
                        .get(&key)
                        .is_some_and(|watermark| created <= *watermark)
                })
            {
                return;
            }
            if let Some(delta) = data
                .get("delta")
                .and_then(Value::as_str)
                .filter(|delta| !delta.is_empty())
            {
                if part_kind == "text" {
                    state.text_parts.entry(key).or_default().push_str(delta);
                    let _ = events.send(DriverEvent::TextDelta(delta.to_owned()));
                } else {
                    state
                        .reasoning_parts
                        .entry(key)
                        .or_default()
                        .push_str(delta);
                    let _ = events.send(DriverEvent::ReasoningDelta(delta.to_owned()));
                }
            }
        }
        "session.text.ended" | "session.reasoning.ended" => {
            let part_kind = if kind == "session.text.ended" {
                "text"
            } else {
                "reasoning"
            };
            let Some(key) = stream_part_key(data, part_kind) else {
                return;
            };
            if value
                .get("created")
                .and_then(Value::as_u64)
                .is_some_and(|created| {
                    state
                        .part_watermarks
                        .get(&key)
                        .is_some_and(|watermark| created <= *watermark)
                })
            {
                return;
            }
            let Some(text) = data
                .get("text")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
            else {
                return;
            };
            let seen = if part_kind == "text" {
                state.text_parts.get(&key)
            } else {
                state.reasoning_parts.get(&key)
            }
            .cloned()
            .unwrap_or_default();
            if let Some(delta) = unseen_suffix(&seen, text) {
                let _ = if part_kind == "text" {
                    events.send(DriverEvent::TextDelta(delta.to_owned()))
                } else {
                    events.send(DriverEvent::ReasoningDelta(delta.to_owned()))
                };
            }
            if !seen.starts_with(text) {
                if part_kind == "text" {
                    state.text_parts.insert(key, text.to_owned());
                } else {
                    state.reasoning_parts.insert(key, text.to_owned());
                }
            }
        }
        "session.tool.called"
        | "session.tool.input.started"
        | "session.tool.input.delta"
        | "session.tool.input.ended"
        | "session.tool.progress"
        | "session.tool.success"
        | "session.tool.failed" => {
            let complete = kind != "session.tool.called";
            let complete = complete
                && !matches!(
                    kind,
                    "session.tool.input.started"
                        | "session.tool.input.delta"
                        | "session.tool.input.ended"
                        | "session.tool.progress"
                );
            let failed = kind == "session.tool.failed";
            let call = data.get("tool").unwrap_or(data);
            let wire_title = call
                .get("name")
                .or_else(|| call.get("tool"))
                .and_then(Value::as_str)
                .unwrap_or("tool")
                .to_owned();
            let id = call
                .get("toolCallID")
                .or_else(|| call.get("callID"))
                .or_else(|| call.get("id"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            if id.is_none() && kind.starts_with("session.tool.input.") {
                return;
            }
            let stored = id.as_ref().and_then(|id| {
                if complete {
                    state.tools.remove(id)
                } else {
                    state.tools.get(id).cloned()
                }
            });
            if complete
                && id
                    .as_ref()
                    .is_some_and(|id| !state.completed_tools.insert(id.clone()))
            {
                return;
            }
            let activity_kind = stored
                .as_ref()
                .map(|(kind, _)| *kind)
                .unwrap_or_else(|| super::support::classify_tool(&wire_title));
            let input = call
                .get("input")
                .or_else(|| call.get("arguments"))
                .or_else(|| call.get("parameters"))
                .or_else(|| call.pointer("/state/input"));
            let title = activity::input_title(input)
                .or_else(|| stored.map(|(_, title)| title))
                .unwrap_or(wire_title);
            if !complete
                && id.as_ref().is_some_and(|id| {
                    state
                        .tools
                        .get(id)
                        .is_some_and(|(kind, current)| *kind == activity_kind && *current == title)
                })
            {
                return;
            }
            if !complete && let Some(id) = id.as_ref() {
                state
                    .tools
                    .insert(id.clone(), (activity_kind, title.clone()));
            }
            let output = call
                .get("content")
                .or_else(|| call.get("output"))
                .or_else(|| call.get("result"))
                .or_else(|| call.get("error"))
                .or_else(|| call.pointer("/state/content"))
                .or_else(|| call.pointer("/state/output"))
                .or_else(|| call.pointer("/state/result"))
                .or_else(|| call.pointer("/state/error"));
            let item = activity::tool_activity(
                id,
                activity_kind,
                title,
                input,
                output,
                call.get("state").or(Some(data)),
                failed,
                complete,
            );
            let _ = events.send(DriverEvent::RichActivity(item));
        }
        "session.step.started" => {
            if let Some(model) = model_key(data) {
                *usage.last_model.lock() = Some(model);
            }
        }
        "session.step.ended" | "session.usage.updated" => {
            if let Some(tokens) = data.get("tokens").and_then(context_tokens) {
                let window = usage.current_window();
                let _ = events.send(DriverEvent::UsageUpdated {
                    context_tokens: Some(tokens),
                    context_window: window,
                    context_window_source: window.map(|_| ContextWindowSource::Provider),
                });
            }
        }
        "session.execution.started" => {
            state.session_idle = false;
            *turn_active.lock() = true;
        }
        "session.execution.succeeded"
        | "session.execution.failed"
        | "session.execution.interrupted" => {
            state.session_idle = true;
            if matches!(
                kind,
                "session.execution.failed" | "session.execution.interrupted"
            ) {
                let message = data
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("OpenCode v2 execution did not complete");
                let _ = events.send(DriverEvent::Error(message.to_owned()));
            }
            if std::mem::take(&mut *turn_active.lock()) {
                let _ = events.send(DriverEvent::TurnFinished {
                    success: kind == "session.execution.succeeded",
                    summary: None,
                });
            }
        }
        "session.step.failed" => {
            let message = data
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("OpenCode v2 step failed");
            let _ = events.send(DriverEvent::Error(message.to_owned()));
        }
        "session.renamed" => {
            let title = data
                .get("title")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .map(str::to_owned);
            if let Some(title) = title {
                let _ = events.send(DriverEvent::AutoTitleUpdated(Some(title)));
            }
        }
        "permission.asked" => {
            let request = data
                .get("request")
                .or_else(|| data.get("permission"))
                .unwrap_or(data);
            request_permission(request, events, commands, auto_approve, state);
        }
        "permission.replied" => {
            if let Some(id) = data
                .get("requestID")
                .or_else(|| data.get("id"))
                .and_then(Value::as_str)
            {
                state.permissions.remove(id);
            }
        }
        "form.created" => {
            let form = data.get("form").unwrap_or(data);
            request_form(form, events, forms);
        }
        "form.replied" | "form.cancelled" => {
            if let Some(id) = data
                .get("formID")
                .or_else(|| data.get("id"))
                .and_then(Value::as_str)
            {
                forms.lock().remove(id);
            }
        }
        "session.compaction.started" => {
            let _ = events.send(DriverEvent::CompactionStarted);
        }
        "session.compaction.ended" => {
            let _ = events.send(DriverEvent::SessionCompacted {
                tokens_before: None,
                tokens_after: data.get("tokens").and_then(context_tokens),
            });
        }
        "session.compaction.failed" => {
            let reason = data
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("OpenCode v2 compaction failed");
            let _ = events.send(DriverEvent::CompactionFailed {
                reason: reason.to_owned(),
            });
        }
        "session.error" => {
            let message = data
                .pointer("/error/message")
                .or_else(|| data.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("OpenCode v2 reported an error");
            let _ = events.send(DriverEvent::Error(message.to_owned()));
        }
        _ => {}
    }
}

fn request_permission(
    request: &Value,
    events: &impl DriverEventSink,
    commands: &Sender<CommandMessage>,
    auto_approve: bool,
    state: &mut OpenCodeV2StreamState,
) {
    let Some(request_id) = request.get("id").and_then(Value::as_str) else {
        return;
    };
    if !state.permissions.insert(request_id.to_owned()) {
        return;
    }
    if auto_approve {
        let _ = commands.send(CommandMessage::Respond {
            request_id: request_id.to_owned(),
            option_id: "once".into(),
        });
        return;
    }
    let action = request
        .get("action")
        .and_then(Value::as_str)
        .filter(|action| !action.is_empty())
        .unwrap_or("tool");
    let resources = request
        .get("resources")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let _ = events.send(DriverEvent::Permission {
        request_id: request_id.to_owned(),
        title: if resources.is_empty() {
            tr!("permission.allow_named_permission", permission = action)
        } else {
            resources.join(", ")
        },
        detail: tr!(
            "permission.agent_asks_for_named_permission",
            permission = action
        ),
        options: vec![
            PermissionOption {
                id: "once".into(),
                label: tr!("permission.allow_once"),
                allow: true,
            },
            PermissionOption {
                id: "always".into(),
                label: tr!("permission.always_allow"),
                allow: true,
            },
            PermissionOption {
                id: "reject".into(),
                label: tr!("common.deny"),
                allow: false,
            },
        ],
    });
}

fn request_form(
    form: &Value,
    events: &impl DriverEventSink,
    forms: &Mutex<HashMap<String, Vec<Value>>>,
) {
    let Some(request_id) = form.get("id").and_then(Value::as_str) else {
        return;
    };
    let fields = form
        .get("fields")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    forms.lock().insert(request_id.to_owned(), fields.clone());
    let questions = fields
        .iter()
        .filter_map(|field| {
            let id = field.get("key").and_then(Value::as_str)?;
            if field.get("hidden").and_then(Value::as_bool) == Some(true) {
                return None;
            }
            let title = field
                .get("title")
                .and_then(Value::as_str)
                .filter(|title| !title.trim().is_empty())
                .unwrap_or(id);
            let question = field
                .get("description")
                .and_then(Value::as_str)
                .filter(|description| !description.trim().is_empty())
                .unwrap_or(title);
            let options = field
                .get("options")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|option| {
                    let label = option.get("label").and_then(Value::as_str)?.trim();
                    (!label.is_empty()).then(|| UserInputOption {
                        label: label.to_owned(),
                        description: option
                            .get("description")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|description| !description.is_empty())
                            .map(str::to_owned),
                    })
                })
                .collect::<Vec<_>>();
            Some(UserInputQuestion {
                id: id.to_owned(),
                header: title.to_owned(),
                question: question.to_owned(),
                options,
                multi_select: field.get("type").and_then(Value::as_str) == Some("multiselect"),
            })
        })
        .collect::<Vec<_>>();
    if !questions.is_empty() {
        let _ = events.send(DriverEvent::UserInputRequested {
            request_id: request_id.to_owned(),
            questions,
        });
    }
}

fn form_answer(fields: &[Value], answers: Vec<UserInputAnswer>) -> Map<String, Value> {
    let answers = answers
        .into_iter()
        .map(|answer| (answer.question_id, answer.answers))
        .collect::<HashMap<_, _>>();
    let mut result = Map::new();
    for field in fields {
        let Some(key) = field.get("key").and_then(Value::as_str) else {
            continue;
        };
        let Some(values) = answers.get(key) else {
            continue;
        };
        let value = match field.get("type").and_then(Value::as_str) {
            Some("multiselect") => json!(values),
            Some("boolean") => json!(values.first().is_some_and(|value| {
                matches!(value.to_ascii_lowercase().as_str(), "true" | "yes" | "1")
            })),
            Some("number" | "integer") => values
                .first()
                .and_then(|value| value.parse::<f64>().ok())
                .map(|value| json!(value))
                .unwrap_or(Value::Null),
            _ => Value::String(values.first().cloned().unwrap_or_default()),
        };
        result.insert(key.to_owned(), value);
    }
    result
}

fn fork_session_removing_turns(
    server: &OpenCodeServer,
    session_id: &str,
    turns_to_remove: usize,
) -> anyhow::Result<ProviderResumeCursor> {
    let path = format!("/api/session/{}/message", encode_path_segment(session_id));
    let messages = server.request("GET", &path, None)?;
    let user_ids = messages
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|message| message.get("type").and_then(Value::as_str) == Some("user"))
        .filter_map(|message| message.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let retained = user_ids.len().checked_sub(turns_to_remove).ok_or_else(|| {
        anyhow!(
            "OpenCode v2 has only {} native turns, but Padu needs to remove {turns_to_remove}",
            user_ids.len()
        )
    })?;
    let body = user_ids
        .get(retained)
        .map_or_else(|| json!({}), |before| json!({"before": before}));
    let path = format!("/api/session/{}/fork", encode_path_segment(session_id));
    let forked = server.request("POST", &path, Some(&body))?;
    let fork_id = forked
        .pointer("/data/id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| anyhow!("OpenCode v2 returned no forked session ID"))?;
    Ok(ProviderResumeCursor::OpenCodeV2 {
        session_id: fork_id.to_owned(),
    })
}

pub(crate) fn fork_session_at_turn_v2(
    binary: &std::path::Path,
    cwd: &std::path::Path,
    session_id: &str,
    retained_turns: usize,
) -> anyhow::Result<ProviderResumeCursor> {
    let server = OpenCodeServer::start_v2_with_env(binary, cwd, &[])?;
    let path = format!("/api/session/{}/message", encode_path_segment(session_id));
    let messages = server.request("GET", &path, None)?;
    let user_ids = messages
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|message| message.get("type").and_then(Value::as_str) == Some("user"))
        .filter_map(|message| message.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let before = user_ids.get(retained_turns);
    let body = before.map_or_else(|| json!({}), |before| json!({"before": before}));
    let path = format!("/api/session/{}/fork", encode_path_segment(session_id));
    let forked = server.request("POST", &path, Some(&body))?;
    let fork_id = forked
        .pointer("/data/id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| anyhow!("OpenCode v2 returned no forked session ID"))?;
    Ok(ProviderResumeCursor::OpenCodeV2 {
        session_id: fork_id.to_owned(),
    })
}
