//! Antigravity (Agy) specifics for the ACP transport.
//!
//! The ACP wire protocol itself stays provider-neutral in [`super::acp`].
//! Everything here is Agy-only: process launch, OAuth identity, model
//! discovery, interactive questions, and plan-artifact extraction.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    AuthenticateRequest, ClientCapabilities, Implementation, InitializeRequest, InitializeResponse,
    LogoutRequest, NewSessionRequest, RequestId, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SelectedPermissionOutcome,
    SessionConfigOption, SessionConfigOptionCategory,
};
use agent_client_protocol::{Agent, Client, ConnectionTo};
use anyhow::anyhow;
use parking_lot::Mutex;
use serde_json::{Map, Value};

use super::acp::{
    AcpLaunch, AcpStreamState, find_config_option, launch_for, sdk_agent,
    sdk_agent_with_stderr_callback, session_config_select_values,
};
use crate::driver::{DriverEventSender, DriverEventSink};
use crate::model::{
    DriverEvent, ProviderKind, ProviderModel, ProviderModelOption, UserInputAnswer,
    UserInputOption, UserInputQuestion,
};

/// Launch details for the Antigravity ACP server.
pub(crate) fn agy_launch(agy_session_temp: Option<&Path>) -> anyhow::Result<AcpLaunch> {
    // Only the Windows branch consumes the session temp dir; keep the
    // parameter warning-free on other platforms.
    #[cfg(not(windows))]
    let _ = agy_session_temp;
    let mut env = Vec::new();
    if std::env::var_os("ANTIGRAVITY_HARNESS_PATH").is_none() {
        let harness_name = if cfg!(target_os = "windows") {
            "localharness_external.exe"
        } else {
            "localharness_external"
        };
        if let Some(harness) = crate::command_env::find_executable(harness_name)
            .or_else(|| crate::command_env::find_executable("localharness"))
        {
            env.push((
                "ANTIGRAVITY_HARNESS_PATH".into(),
                harness.to_string_lossy().into_owned(),
            ));
        }
    }
    #[cfg(windows)]
    if let Ok((runfiles_cache, isolated_temp)) = crate::agy_install::ensure_runtime_dirs() {
        env.push((
            "RULES_PYTHON_EXTRACT_ROOT".into(),
            runfiles_cache.to_string_lossy().into_owned(),
        ));
        // Per-session scratch when the driver holds a guard; the
        // shared root otherwise (short-lived helpers). The runfiles
        // cache stays shared either way — no ~900MB duplication.
        let temp_dir = agy_session_temp.unwrap_or(&isolated_temp);
        let temp_str = temp_dir.to_string_lossy().into_owned();
        env.push(("TEMP".into(), temp_str.clone()));
        env.push(("TMP".into(), temp_str));
    }
    Ok(AcpLaunch {
        // The official registry distribution requires an empty UID flag on
        // Linux; macOS and Windows accept the server with no arguments.
        args: if cfg!(target_os = "linux") {
            vec!["--uid=".into()]
        } else {
            Vec::new()
        },
        env,
    })
}

pub(crate) fn discover_agy_models(binary: &Path) -> Vec<ProviderModel> {
    let Ok(cwd) = std::env::current_dir() else {
        return Vec::new();
    };
    let Ok(agent) = sdk_agent(
        binary,
        &cwd,
        launch_for(ProviderKind::Agy, None, None).unwrap_or(AcpLaunch {
            args: Vec::new(),
            env: Vec::new(),
        }),
        None,
        Arc::new(Mutex::new(Vec::new())),
    ) else {
        return Vec::new();
    };
    let request = Client.builder().name("padu").connect_with(
        agent,
        async move |connection: ConnectionTo<Agent>| {
            let initialize = connection
                .send_request(
                    InitializeRequest::new(ProtocolVersion::V1)
                        .client_capabilities(ClientCapabilities::new().terminal(false))
                        .client_info(Implementation::new("padu", env!("CARGO_PKG_VERSION"))),
                )
                .block_task()
                .await?;
            let _ = authenticate_agy_connection(&connection, &initialize).await;
            let response = connection
                .send_request(NewSessionRequest::new(&cwd))
                .block_task()
                .await?;
            if response.config_options.is_none() {
                return Ok(Vec::new());
            }
            let raw_ids = response
                .config_options
                .unwrap_or_default()
                .iter()
                .find(|option| option.category == Some(SessionConfigOptionCategory::Model))
                .map(session_config_select_values)
                .unwrap_or_default()
                .into_iter()
                .map(|id| id.to_owned())
                .collect::<Vec<_>>();
            Ok(group_agy_models(&raw_ids))
        },
    );
    let result = smol::block_on(smol::future::race(
        async move { request.await.map_err(anyhow::Error::new) },
        async move {
            smol::Timer::after(Duration::from_secs(30)).await;
            Err(anyhow!("Antigravity model discovery timed out"))
        },
    ));
    result.unwrap_or_default()
}

pub(crate) fn split_agy_model_effort(raw_id: &str) -> (&str, Option<&str>) {
    if let Some((base, suffix)) = raw_id.rsplit_once('-') {
        if matches!(
            suffix,
            "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra"
        ) {
            return (base, Some(suffix));
        }
    }
    (raw_id, None)
}

pub(crate) fn group_agy_models(raw_ids: &[String]) -> Vec<ProviderModel> {
    let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
    for raw in raw_ids {
        let (base, effort) = split_agy_model_effort(raw);
        if let Some((_, efforts)) = groups.iter_mut().find(|(b, _)| *b == base) {
            if let Some(effort) = effort {
                if !efforts.contains(&effort) {
                    efforts.push(effort);
                }
            }
        } else {
            groups.push((base, effort.map_or_else(Vec::new, |e| vec![e])));
        }
    }

    const EFFORT_ORDER: [&str; 7] = ["minimal", "low", "medium", "high", "xhigh", "max", "ultra"];

    groups
        .into_iter()
        .enumerate()
        .map(|(index, (base, mut efforts))| {
            efforts.sort_by_key(|e| {
                EFFORT_ORDER
                    .iter()
                    .position(|known| known == e)
                    .unwrap_or(usize::MAX)
            });

            let name = crate::model_catalog::display_name_from_slug(base);
            let mut model = ProviderModel::new(base, name);
            if index == 0 {
                model = model.default();
            }

            if !efforts.is_empty() {
                let default_effort = if efforts.contains(&"medium") {
                    "medium"
                } else if efforts.contains(&"low") {
                    "low"
                } else {
                    efforts[0]
                };

                let options = efforts
                    .into_iter()
                    .map(|effort| {
                        ProviderModelOption::new(
                            effort,
                            crate::model_catalog::reasoning_effort_label(effort),
                        )
                    })
                    .collect::<Vec<_>>();

                model = model.reasoning(options, default_effort);
            }

            model
        })
        .collect()
}

pub(crate) fn resolve_agy_model_id(
    model: &str,
    reasoning_effort: Option<&str>,
    config_options: Option<&[SessionConfigOption]>,
) -> String {
    let available_values = config_options
        .and_then(|options| find_config_option(options, SessionConfigOptionCategory::Model))
        .map(session_config_select_values)
        .unwrap_or_default();

    // 1. If model is already an exact available value (e.g. gemini-3.8-flash-high or gemini-pro-agent):
    if available_values.contains(&model) {
        return model.to_owned();
    }

    // 2. If an effort is explicitly specified:
    if let Some(effort) = reasoning_effort.filter(|e| !e.is_empty()) {
        let candidate = format!("{model}-{effort}");
        if available_values.is_empty() || available_values.contains(&candidate.as_str()) {
            return candidate;
        }
    }

    // 3. If no effort or effort didn't match, check available effort suffixes in preferred order:
    if !available_values.is_empty() {
        for preferred in ["medium", "low", "high", "xhigh", "minimal", "max", "ultra"] {
            let candidate = format!("{model}-{preferred}");
            if available_values.contains(&candidate.as_str()) {
                return candidate;
            }
        }
        let prefix = format!("{model}-");
        if let Some(matched) = available_values.iter().find(|val| val.starts_with(&prefix)) {
            return (*matched).to_owned();
        }
    }

    // 4. Default fallback:
    if let Some(effort) = reasoning_effort.filter(|e| !e.is_empty()) {
        format!("{model}-{effort}")
    } else {
        model.to_owned()
    }
}

pub fn agy_auth_status(binary: &Path) -> anyhow::Result<bool> {
    // 1. Check Antigravity's own token files in GEMINI_HOME or ~/.gemini.
    // Deliberately scoped to the `antigravity*` stores: the root
    // `oauth_creds.json` is Gemini CLI's global credential and must not
    // mark Antigravity as signed in on its own.
    let gemini_home = std::env::var_os("GEMINI_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".gemini")));
    if let Some(home) = gemini_home {
        if agy_token_files()
            .iter()
            .any(|relative| home.join(relative).exists())
        {
            return Ok(true);
        }
    }

    // 2. On macOS, check macOS Keychain for the antigravity-acp account
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                "gemini",
                "-a",
                "antigravity-acp",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
        {
            if output.status.success() {
                return Ok(true);
            }
        }
    }

    // 3. Fallback to CLI command if a custom wrapper or binary supports it
    if let Ok(output) = crate::command_env::command(binary)
        .args(["auth", "status"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    {
        if output.status.success() {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Antigravity's own credential files, relative to `GEMINI_HOME` or
/// `~/.gemini`, in lookup order. `antigravity-cli/antigravity-oauth-token`
/// (`{token, auth_method, id_token}`) is the live store verified against a
/// signed-in install; the `jetski` and `antigravity-acp` variants cover
/// sibling distributions. The Gemini-global root `oauth_creds.json` is
/// deliberately absent: it belongs to Gemini CLI, not Antigravity.
fn agy_token_files() -> [&'static str; 5] {
    [
        "antigravity-cli/antigravity-oauth-token",
        "jetski-standalone-oauth-token",
        "antigravity-acp/acp_token.json",
        "antigravity-acp/acp_business_token.json",
        "antigravity/acp/acp_token.json",
    ]
}

/// Logged-in Google identity for Antigravity, scoped strictly to the ACP
/// server credentials (`antigravity-acp` in Keychain or `acp_token.json` files).
/// Never reads Gemini CLI's global credentials (`oauth_creds.json`).
///
/// Looks in order:
/// 1. Cached identity file `account_identity.json` in Padu's Antigravity directory
/// 2. Local Antigravity token files (`antigravity-cli`, `jetski`, `antigravity-acp`)
/// 3. If only `refresh_token` is present in ACP credentials (e.g. macOS Keychain or
///    acp_token.json), resolves the user's email via a token refresh request,
///    caches it to `account_identity.json`, and returns it.
pub fn agy_account_label() -> Option<String> {
    // 1. Cached identity file in ~/.padu/providers/antigravity/account_identity.json
    let cache_path = crate::agy_install::account_identity_file();
    if let Ok(content) = std::fs::read_to_string(&cache_path) {
        if let Some(account) = agy_account_from_token_blob(&content) {
            return Some(account);
        }
    }

    // 2. Check Antigravity-specific token files
    let home = std::env::var_os("GEMINI_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".gemini")));
    if let Some(ref home) = home {
        for relative in agy_token_files() {
            if let Ok(payload) = std::fs::read_to_string(home.join(relative)) {
                if let Some(account) = agy_account_from_token_blob(&payload) {
                    return Some(account);
                }
            }
        }
    }

    // 3. Extract ACP credentials (keychain on macOS or acp_token.json) and resolve email
    let acp_blob = {
        #[cfg(target_os = "macos")]
        {
            crate::command_env::plain_command("security")
                .args([
                    "find-generic-password",
                    "-s",
                    "gemini",
                    "-a",
                    "antigravity-acp",
                    "-w",
                ])
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }
    .or_else(|| {
        let home = home.as_ref()?;
        for relative in [
            "antigravity-acp/acp_token.json",
            "antigravity-acp/acp_business_token.json",
            "antigravity/acp/acp_token.json",
        ] {
            if let Ok(payload) = std::fs::read_to_string(home.join(relative)) {
                return Some(payload);
            }
        }
        None
    });

    if let Some(blob) = acp_blob {
        if let Some(account) = agy_account_from_token_blob(&blob) {
            return Some(account);
        }
        if let Some(email) = resolve_email_from_acp_token_blob(&blob) {
            if let Some(parent) = cache_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(
                &cache_path,
                serde_json::json!({ "email": &email }).to_string(),
            );
            return Some(email);
        }
    }

    None
}

/// Given an Antigravity ACP credential blob with OAuth client credentials and a
/// `refresh_token`, exchanges the refresh token with Google's token endpoint to
/// extract the `email` claim from the returned `id_token` (or userinfo endpoint).
fn resolve_email_from_acp_token_blob(payload: &str) -> Option<String> {
    let value: Value = serde_json::from_str(payload.trim()).ok()?;
    let client_id = value.get("client_id")?.as_str()?;
    let client_secret = value.get("client_secret")?.as_str()?;
    let refresh_token = value.get("refresh_token")?.as_str()?;
    let token_uri = value
        .get("token_uri")
        .and_then(Value::as_str)
        .unwrap_or("https://oauth2.googleapis.com/token");

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    let form_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", client_id)
        .append_pair("client_secret", client_secret)
        .append_pair("refresh_token", refresh_token)
        .append_pair("grant_type", "refresh_token")
        .finish();

    let response = client
        .post(token_uri)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_body)
        .send()
        .ok()?;

    let response_value: Value = serde_json::from_reader(response).ok()?;

    if let Some(id_token) = response_value.get("id_token").and_then(Value::as_str) {
        if let Some(email) = crate::usage::jwt_payload_email(id_token) {
            return Some(email);
        }
    }

    if let Some(access_token) = response_value.get("access_token").and_then(Value::as_str) {
        let userinfo_resp = client
            .get("https://www.googleapis.com/oauth2/v2/userinfo")
            .header("Authorization", format!("Bearer {access_token}"))
            .send()
            .ok()?;
        let userinfo: Value = serde_json::from_reader(userinfo_resp).ok()?;
        if let Some(email) = userinfo.get("email").and_then(Value::as_str) {
            let trimmed = email.trim();
            if !trimmed.is_empty() && trimmed.contains('@') {
                return Some(trimmed.to_owned());
            }
        }
    }

    None
}

/// Identity from one Antigravity credential blob: explicit email fields win,
/// then the OAuth `id_token` JWT's email claim. Pure for testing; callers
/// feed it token-file payloads verbatim.
pub(crate) fn agy_account_from_token_blob(payload: &str) -> Option<String> {
    let value: Value = serde_json::from_str(payload.trim()).ok()?;
    let candidates = [
        "/email",
        "/email_address",
        "/account/email",
        "/account/email_address",
        "/user/email",
        "/user/email_address",
        "/id_token",
        "/idToken",
        "/tokens/id_token",
    ];
    for pointer in candidates {
        let Some(text) = value.pointer(pointer).and_then(Value::as_str) else {
            continue;
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        // Raw JWTs carry no `@`; only the decoded email claim counts.
        if text.contains('@') {
            return Some(text.to_owned());
        }
        if pointer.contains("token")
            && let Some(email) = crate::usage::jwt_payload_email(text)
        {
            return Some(email);
        }
    }
    // Nested credential envelopes (e.g. keychain blobs wrapping the OAuth
    // response one level deeper) get one recursive look.
    for key in ["credentials", "oauth", "tokens", "account", "user"] {
        if let Some(nested) = value.get(key)
            && let Ok(rewritten) = serde_json::to_string(nested)
            && let Some(account) = agy_account_from_token_blob(&rewritten)
        {
            return Some(account);
        }
    }
    None
}

pub fn logout_agy(binary: &Path, cwd: &Path) -> anyhow::Result<()> {
    // 1. Best-effort ACP RPC logout with 45-second timeout (giving Windows cold starts ample time)
    if let Ok(launch) = launch_for(ProviderKind::Agy, None, None) {
        if let Ok(agent) = sdk_agent(binary, cwd, launch, None, Arc::new(Mutex::new(Vec::new()))) {
            let request = Client.builder().name("padu").connect_with(
                agent,
                async move |connection: ConnectionTo<Agent>| {
                    let _initialize = connection
                        .send_request(
                            InitializeRequest::new(ProtocolVersion::V1)
                                .client_capabilities(ClientCapabilities::new().terminal(false))
                                .client_info(Implementation::new(
                                    "padu",
                                    env!("CARGO_PKG_VERSION"),
                                )),
                        )
                        .block_task()
                        .await?;
                    connection
                        .send_request(LogoutRequest::new())
                        .block_task()
                        .await?;
                    Ok(())
                },
            );
            let _ = smol::block_on(smol::future::race(
                async move { request.await.map_err(anyhow::Error::new) },
                async move {
                    smol::Timer::after(Duration::from_secs(45)).await;
                    Err(anyhow!("Antigravity sign-out timed out"))
                },
            ));
        }
    }

    // 2. Clear cached ACP identity file
    let cache_file = crate::agy_install::account_identity_file();
    if cache_file.exists() {
        let _ = std::fs::remove_file(&cache_file);
    }

    // 3. Deterministically remove all local token files on disk
    let gemini_home = std::env::var_os("GEMINI_HOME").map(PathBuf::from);
    let home = dirs::home_dir();
    let mut candidate_roots = Vec::new();
    if let Some(gemini_home) = gemini_home {
        candidate_roots.push(gemini_home);
    }
    if let Some(home) = home {
        candidate_roots.push(home.join(".gemini"));
    }

    for root in candidate_roots {
        for relative in agy_token_files() {
            let path = root.join(relative);
            if path.exists() {
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    // 4. Purge macOS Keychain entry if on macOS
    #[cfg(target_os = "macos")]
    {
        let _ = crate::command_env::plain_command("security")
            .args([
                "delete-generic-password",
                "-s",
                "gemini",
                "-a",
                "antigravity-acp",
            ])
            .output();
    }

    // 4. Trigger stale temp directories cleanup
    crate::agy_install::cleanup_stale_antigravity_temp_dirs();

    // 5. Verify auth status
    if agy_auth_status(binary)? {
        anyhow::bail!("Antigravity sign-out did not clear credentials");
    }

    Ok(())
}

pub fn authenticate_agy(
    binary: &Path,
    cwd: &Path,
    on_auth_url: impl Fn(String) + Send + Sync + 'static,
) -> anyhow::Result<()> {
    let on_auth_url = Arc::new(on_auth_url);
    let auth_callback = {
        let on_auth_url = on_auth_url.clone();
        Arc::new(move |line: &str| {
            if let Some(start) = line
                .find("https://accounts.google.com/o/oauth2")
                .or_else(|| line.find("https://"))
            {
                let rest = &line[start..];
                let url = rest.split_whitespace().next().unwrap_or(rest);
                on_auth_url(url.to_string());
            }
        })
    };
    let agent = sdk_agent_with_stderr_callback(
        binary,
        cwd,
        launch_for(ProviderKind::Agy, None, None)?,
        None,
        Arc::new(Mutex::new(Vec::new())),
        Some(auth_callback),
    )?;
    let request = Client.builder().name("padu").connect_with(
        agent,
        async move |connection: ConnectionTo<Agent>| {
            let initialize = connection
                .send_request(
                    InitializeRequest::new(ProtocolVersion::V1)
                        .client_capabilities(ClientCapabilities::new().terminal(false))
                        .client_info(Implementation::new("padu", env!("CARGO_PKG_VERSION"))),
                )
                .block_task()
                .await?;
            authenticate_agy_connection(&connection, &initialize).await
        },
    );
    smol::block_on(smol::future::race(
        async move { request.await.map_err(anyhow::Error::new) },
        async move {
            smol::Timer::after(Duration::from_secs(300)).await;
            Err(anyhow!("Antigravity sign-in timed out"))
        },
    ))
    .map_err(|error| anyhow::anyhow!("Antigravity sign-in failed: {error}"))
}

pub(crate) async fn authenticate_agy_connection(
    connection: &ConnectionTo<Agent>,
    initialize: &InitializeResponse,
) -> agent_client_protocol::Result<()> {
    let Some(method) = initialize
        .auth_methods
        .iter()
        .find(|method| method.id().0.as_ref() == "oauth-personal")
        .or_else(|| initialize.auth_methods.first())
    else {
        return Ok(());
    };
    connection
        .send_request(AuthenticateRequest::new(method.id().clone()))
        .block_task()
        .await?;
    Ok(())
}

pub(crate) fn is_agy_question_request(params: &Value, request: &RequestPermissionRequest) -> bool {
    let is_interaction = params
        .pointer("/toolCall/toolCallId")
        .and_then(Value::as_str)
        .is_some_and(|id| id.starts_with("interaction_"));
    is_interaction && !request.options.is_empty()
}

pub(crate) fn agy_user_input_questions(
    params: &Value,
    request: &RequestPermissionRequest,
) -> Vec<UserInputQuestion> {
    if let Some(raw_questions) = params
        .pointer("/toolCall/rawInput/questions")
        .and_then(Value::as_array)
        .filter(|q| !q.is_empty())
    {
        let mut questions = Vec::new();
        for (idx, q) in raw_questions.iter().enumerate() {
            let question_text = q
                .get("question")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .unwrap_or("Please select an option:")
                .to_owned();
            let multi_select = q
                .get("is_multi_select")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let mut options = Vec::new();
            if let Some(raw_opts) = q.get("options").and_then(Value::as_array) {
                for opt in raw_opts {
                    if let Some(opt_str) = opt.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                        options.push(UserInputOption {
                            label: opt_str.to_owned(),
                            description: None,
                        });
                    }
                }
            }
            if options.is_empty() {
                options = request
                    .options
                    .iter()
                    .map(|o| UserInputOption {
                        label: o.name.clone(),
                        description: None,
                    })
                    .collect();
            }
            questions.push(UserInputQuestion {
                id: format!("question_{idx}"),
                header: "Question".to_owned(),
                question: question_text,
                options,
                multi_select,
            });
        }
        if !questions.is_empty() {
            return questions;
        }
    }

    let question_text = params
        .pointer("/toolCall/title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or("Please select an option:")
        .to_owned();

    let options = request
        .options
        .iter()
        .map(|option| UserInputOption {
            label: option.name.clone(),
            description: None,
        })
        .collect::<Vec<_>>();

    if options.is_empty() {
        return Vec::new();
    }

    vec![UserInputQuestion {
        id: "question_0".to_owned(),
        header: "Question".to_owned(),
        question: question_text,
        options,
        multi_select: false,
    }]
}

pub(crate) fn agy_user_input_response(
    request: &RequestPermissionRequest,
    submitted: &[UserInputAnswer],
) -> RequestPermissionResponse {
    let selected_label = submitted
        .iter()
        .find_map(|answer| answer.answers.first())
        .map(String::as_str);

    if let Some(selected) = selected_label {
        let matched = request
            .options
            .iter()
            .find(|opt| opt.name.trim().eq_ignore_ascii_case(selected.trim()))
            .or_else(|| {
                request
                    .options
                    .iter()
                    .find(|opt| opt.option_id.to_string() == selected.trim())
            })
            .or_else(|| request.options.first());

        if let Some(choice) = matched {
            return RequestPermissionResponse::new(RequestPermissionOutcome::Selected(
                SelectedPermissionOutcome::new(choice.option_id.clone()),
            ));
        }
    }

    RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled)
}

/// Mutable plan-artifact tracking for one Agy stream. Lives inside the
/// transport's stream state; all plan parsing lives here with it.
#[derive(Default)]
pub(crate) struct AgyPlanState {
    content: Option<String>,
    path: Option<PathBuf>,
    text_buffer: String,
    inlined: bool,
    /// When this turn last received a stream update. The turn watchdog reads it
    /// to tell a provider that went silent from one that is still working.
    last_update: Option<Instant>,
}

impl AgyPlanState {
    pub(crate) fn reset(&mut self) {
        self.content = None;
        self.path = None;
        self.text_buffer.clear();
        self.inlined = false;
        // Sending the prompt starts this turn's clock even if the provider
        // never answers, so the watchdog measures silence from the request.
        self.last_update = Some(Instant::now());
    }

    /// Records a stream update so a stalled turn can be told from a live one.
    pub(crate) fn note_activity(&mut self) {
        self.last_update = Some(Instant::now());
    }

    /// Buffer a streamed text chunk, harvesting a plan-file link on first sight.
    pub(crate) fn observe_text_chunk(&mut self, text: &str) {
        self.text_buffer.push_str(text);
        if self.path.is_none() {
            if let Some(path) = extract_plan_link_path(&self.text_buffer) {
                self.path = Some(path);
            }
        }
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn percent_decode_file_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(index + 1).copied().and_then(hex_value),
                bytes.get(index + 2).copied().and_then(hex_value),
            )
        {
            decoded.push(high << 4 | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).unwrap_or_else(|_| path.to_owned())
}

fn is_plan_file(path: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let file_name = normalized
        .rsplit('/')
        .next()
        .unwrap_or(&normalized)
        .to_ascii_lowercase();
    let ext_matches = file_name.ends_with(".md") || file_name.ends_with(".markdown");
    if !ext_matches {
        return false;
    }

    if file_name.contains("plan") {
        return true;
    }

    let normalized_lower = normalized.to_ascii_lowercase();
    let segments: Vec<&str> = normalized_lower.split('/').collect();
    let dir_segments = if segments.len() > 1 {
        &segments[..segments.len() - 1]
    } else {
        &[]
    };

    dir_segments.iter().any(|&seg| {
        seg == "brain" || seg == "antigravity-acp" || seg == "antigravity" || seg == ".gemini"
    })
}

fn extract_plan_link_path(text: &str) -> Option<PathBuf> {
    // 1. Scan for file:// URIs (e.g. file:///path/to/plan.md or [plan.md](file://...))
    let mut search_idx = 0;
    while let Some(start) = text[search_idx..].find("file://") {
        let actual_start = search_idx + start;
        let url_part = &text[actual_start..];
        let end = url_part
            .find(|c: char| c.is_whitespace() || matches!(c, ')' | ']' | '"' | '\'' | '>'))
            .unwrap_or(url_part.len());
        let raw_url = &url_part[..end];
        let clean_url = raw_url.trim_end_matches(|c: char| c == '.' || c == ',');
        if let Ok(url) = url::Url::parse(clean_url) {
            let path = url.to_file_path().ok().or_else(|| {
                // Fallback for Windows file URLs parsed across platforms
                let decoded = percent_decode_file_path(url.path());
                let trimmed = decoded.trim_start_matches('/');
                if trimmed.len() >= 2 && trimmed.chars().nth(1) == Some(':') {
                    Some(PathBuf::from(trimmed))
                } else {
                    Some(PathBuf::from(decoded))
                }
            });
            if let Some(path) = path {
                if is_plan_file(&path) {
                    return Some(path);
                }
            }
        }
        search_idx = actual_start + "file://".len();
    }

    // 2. Scan for markdown links with direct filesystem paths: [...](path)
    let mut search_idx = 0;
    while let Some(start) = text[search_idx..].find("](") {
        let actual_start = search_idx + start + 2;
        let rest = &text[actual_start..];
        if let Some(end) = rest.find(')') {
            let target = rest[..end].trim();
            if !target.starts_with("http://") && !target.starts_with("https://") {
                let decoded = percent_decode_file_path(target);
                let path = PathBuf::from(decoded);
                if is_plan_file(&path) {
                    return Some(path);
                }
            }
            search_idx = actual_start + end + 1;
        } else {
            break;
        }
    }

    None
}

pub(crate) fn extract_agy_tool_plan(update: &Value, state: &mut AgyPlanState) {
    let arguments = update.get("rawInput").filter(|value| !value.is_null());
    let get_prop = |map: &Map<String, Value>, keys: &[&str]| -> Option<String> {
        for key in keys {
            if let Some(val) = map.get(*key).and_then(Value::as_str) {
                return Some(val.to_owned());
            }
        }
        None
    };

    let file_keys = &["TargetFile", "targetFile", "target_file", "path", "file"];
    let content_keys = &[
        "CodeContent",
        "codeContent",
        "code_content",
        "content",
        "text",
    ];

    let (target_file, code_content) = match arguments {
        Some(Value::Object(map)) => (get_prop(map, file_keys), get_prop(map, content_keys)),
        Some(Value::String(s)) => {
            if let Ok(Value::Object(map)) = serde_json::from_str(s) {
                (get_prop(&map, file_keys), get_prop(&map, content_keys))
            } else {
                (None, None)
            }
        }
        _ => (None, None),
    };

    if let Some(target_file) = target_file {
        let path = PathBuf::from(target_file);
        if is_plan_file(&path) {
            state.path = Some(path);
            if let Some(content) = code_content {
                state.content = Some(content);
            }
        }
    }
}

/// The first `max` bytes of `value`, backed off to a UTF-8 boundary so a
/// multi-byte character straddling the cut cannot panic.
fn byte_prefix(value: &str, max: usize) -> &str {
    if value.len() <= max {
        return value;
    }
    let mut end = max;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

pub(crate) fn maybe_inline_agy_plan(
    provider: ProviderKind,
    state: &mut AgyPlanState,
    events: &impl DriverEventSink,
) {
    if provider != ProviderKind::Agy || state.inlined {
        return;
    }
    if state.path.is_none() {
        if let Some(path) = extract_plan_link_path(&state.text_buffer) {
            state.path = Some(path);
        }
    }
    let content = state.content.clone().or_else(|| {
        let path = state.path.as_ref()?;
        std::fs::read_to_string(path).ok()
    });
    if let Some(content) = content {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            if !state.text_buffer.contains(trimmed)
                && !(trimmed.len() > 60 && state.text_buffer.contains(byte_prefix(trimmed, 60)))
            {
                state.inlined = true;
                let _ = events.send(DriverEvent::TextDelta(format!("\n\n---\n\n{trimmed}\n")));
            }
        }
    }
}

// __AGY_APPEND__

/// How often the turn watchdog samples the stream for silence.
const AGY_WATCHDOG_TICK: Duration = Duration::from_secs(1);

/// How long an Agy turn may stay quiet, with its visible work already finished,
/// before Padu stops waiting for the `session/prompt` response that ends it.
///
/// The response is the turn's only terminator: Antigravity sends it once its
/// harness reports the conversation fully idle, and on Windows that report can
/// go missing. The answer then streams to completion while the turn hangs — no
/// protocol error, no process exit, no further traffic — leaving the session
/// spinning until the user cancels.
///
/// Silence alone cannot decide this, so the watchdog also requires the shape of
/// a finished turn: output already streamed, and no announced tool call left
/// open. A turn that is still generating keeps emitting message and thought
/// deltas, and one running a tool keeps that call open, so neither reads as
/// quiet. The window is generous on purpose — it also covers a long silent pause
/// such as context compaction — because settling a turn that is still running
/// would drop the rest of its output.
pub(crate) const AGY_TURN_QUIET: Duration = Duration::from_secs(120);

/// Whether `state` has gone quiet long enough to settle its turn.
fn turn_went_quiet(
    state: &AcpStreamState,
    armed_at: Instant,
    now: Instant,
    quiet: Duration,
) -> bool {
    state.agy_turn_looks_settled()
        && now.saturating_duration_since(state.agy.last_update.unwrap_or(armed_at)) >= quiet
}

/// Settles one outstanding Agy prompt whose turn the provider never finished.
///
/// Returns immediately; the sampling runs on its own thread. `is_pending`
/// reports whether the ACP request is still outstanding and `settle` claims it,
/// returning false when someone else already did — so a real response arriving
/// at any point wins the race and no turn is ever finished twice. `live` is the
/// connection's own flag, which lets a watchdog outlive its driver by at most
/// one tick, and `cancel_turn` tells the provider to abandon the turn it still
/// believes is running.
#[allow(clippy::too_many_arguments)]
pub(crate) fn watch_agy_turn(
    quiet: Duration,
    state: Arc<Mutex<AcpStreamState>>,
    request_id: RequestId,
    live: Arc<AtomicBool>,
    is_pending: impl Fn(&RequestId) -> bool + Send + 'static,
    settle: impl Fn(&RequestId) -> bool + Send + 'static,
    cancel_turn: impl Fn() + Send + 'static,
    events: DriverEventSender,
) {
    let _ = thread::Builder::new()
        .name("padu-agy-turn".into())
        .spawn(move || {
            let armed_at = Instant::now();
            loop {
                thread::sleep(AGY_WATCHDOG_TICK);
                if !live.load(Ordering::Acquire) || !is_pending(&request_id) {
                    return;
                }
                if !turn_went_quiet(&state.lock(), armed_at, Instant::now(), quiet) {
                    continue;
                }
                if !settle(&request_id) {
                    return;
                }
                // Antigravity still holds this turn open, and its step stream
                // stays latched until that turn ends — which is what would make
                // every later prompt on this session fail. Tell it to stop; the
                // response it then sends is absorbed by the claim above.
                cancel_turn();
                // The turn is over by every measure Padu can take, so it ends
                // the way a completed turn does: the plan artifact lands in the
                // transcript first, then the turn settles with no summary.
                maybe_inline_agy_plan(ProviderKind::Agy, &mut state.lock().agy, &events);
                let _ = events.send(DriverEvent::TurnFinished {
                    success: true,
                    summary: None,
                });
                return;
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::acp::{AcpStreamState, handle_session_update};
    use agent_client_protocol::schema::v1::{SessionConfigSelectOption, SessionNotification};
    use serde_json::json;

    fn select_model_option(current: &str, values: &[&str]) -> SessionConfigOption {
        SessionConfigOption::select(
            "model".to_owned(),
            "model".to_owned(),
            current.to_owned(),
            values
                .iter()
                .map(|value| SessionConfigSelectOption::new((*value).to_owned(), *value))
                .collect::<Vec<_>>(),
        )
        .category(SessionConfigOptionCategory::Model)
    }

    #[test]
    fn agy_identity_prefers_email_then_jwt_then_nothing() {
        assert_eq!(
            agy_account_from_token_blob(r#"{"email": "dev@example.com"}"#).as_deref(),
            Some("dev@example.com")
        );
        // Nested envelopes get one recursive look.
        assert_eq!(
            agy_account_from_token_blob(
                r#"{"credentials": {"account": {"email_address": "nested@example.com"}}}"#
            )
            .as_deref(),
            Some("nested@example.com")
        );
        // OAuth id_token JWTs decode to their email claim.
        use base64::Engine as _;
        let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::json!({"email": "jwt@example.com"}).to_string());
        let blob = format!(r#"{{"id_token": "header.{claims}.signature"}}"#);
        assert_eq!(
            agy_account_from_token_blob(&blob).as_deref(),
            Some("jwt@example.com")
        );
        // The live antigravity-cli envelope carries the JWT beside the
        // opaque access token, which must never surface as the identity.
        let live = format!(
            r#"{{"token": "ya29.opaque", "auth_method": "oauth", "id_token": "header.{claims}.signature"}}"#
        );
        assert_eq!(
            agy_account_from_token_blob(&live).as_deref(),
            Some("jwt@example.com")
        );
        // Token-only blobs without identity stay hidden, not errors.
        assert_eq!(
            agy_account_from_token_blob(r#"{"access_token": "ya29.abc"}"#),
            None
        );
        assert_eq!(agy_account_from_token_blob("not json"), None);
        assert_eq!(agy_account_from_token_blob("{}"), None);
    }

    #[test]
    fn resolve_email_from_acp_token_blob_rejects_empty_or_invalid_blob() {
        assert_eq!(resolve_email_from_acp_token_blob("not json"), None);
        assert_eq!(resolve_email_from_acp_token_blob("{}"), None);
        assert_eq!(
            resolve_email_from_acp_token_blob(
                r#"{"client_id": "cid", "client_secret": "sec", "refresh_token": "rt"}"#
            ),
            None
        );
    }

    #[test]
    fn split_and_group_agy_models() {
        let raw = vec![
            "gemini-3.8-flash-high".to_string(),
            "gemini-3.8-flash-medium".to_string(),
            "gemini-3.8-flash-low".to_string(),
            "gemini-3.7-flash-high".to_string(),
            "gemini-3.7-flash-medium".to_string(),
            "gemini-3.7-flash-low".to_string(),
            "gemini-pro-agent".to_string(),
            "gemini-3.1-pro-low".to_string(),
        ];
        let models = group_agy_models(&raw);
        assert_eq!(models.len(), 4);

        assert_eq!(models[0].id, "gemini-3.8-flash");
        assert_eq!(models[0].name, "Gemini 3.8 Flash");
        assert!(models[0].is_default);
        assert_eq!(
            models[0].default_reasoning_effort.as_deref(),
            Some("medium")
        );
        assert_eq!(
            models[0]
                .reasoning_efforts
                .iter()
                .map(|o| o.id.as_str())
                .collect::<Vec<_>>(),
            vec!["low", "medium", "high"]
        );

        assert_eq!(models[1].id, "gemini-3.7-flash");
        assert_eq!(models[1].name, "Gemini 3.7 Flash");
        assert!(!models[1].is_default);
        assert_eq!(
            models[1].default_reasoning_effort.as_deref(),
            Some("medium")
        );
        assert_eq!(
            models[1]
                .reasoning_efforts
                .iter()
                .map(|o| o.id.as_str())
                .collect::<Vec<_>>(),
            vec!["low", "medium", "high"]
        );

        assert_eq!(models[2].id, "gemini-pro-agent");
        assert_eq!(models[2].name, "Gemini Pro Agent");
        assert!(models[2].reasoning_efforts.is_empty());
        assert_eq!(models[2].default_reasoning_effort, None);

        assert_eq!(models[3].id, "gemini-3.1-pro");
        assert_eq!(models[3].name, "Gemini 3.1 Pro");
        assert_eq!(models[3].default_reasoning_effort.as_deref(), Some("low"));
        assert_eq!(
            models[3]
                .reasoning_efforts
                .iter()
                .map(|o| o.id.as_str())
                .collect::<Vec<_>>(),
            vec!["low"]
        );
    }

    #[test]
    fn resolve_agy_model_id_resolves_effort_and_fallbacks() {
        let option = select_model_option(
            "gemini-3.8-flash-medium",
            &[
                "gemini-3.8-flash-high",
                "gemini-3.8-flash-medium",
                "gemini-3.8-flash-low",
                "gemini-pro-agent",
                "gemini-3.1-pro-low",
            ],
        );
        let options = vec![option];

        assert_eq!(
            resolve_agy_model_id("gemini-3.8-flash", Some("high"), Some(&options)),
            "gemini-3.8-flash-high"
        );
        assert_eq!(
            resolve_agy_model_id("gemini-3.8-flash", Some("low"), Some(&options)),
            "gemini-3.8-flash-low"
        );
        assert_eq!(
            resolve_agy_model_id("gemini-3.8-flash", None, Some(&options)),
            "gemini-3.8-flash-medium"
        );
        assert_eq!(
            resolve_agy_model_id("gemini-3.1-pro", None, Some(&options)),
            "gemini-3.1-pro-low"
        );
        assert_eq!(
            resolve_agy_model_id("gemini-pro-agent", None, Some(&options)),
            "gemini-pro-agent"
        );
        assert_eq!(
            resolve_agy_model_id("gemini-3.8-flash-high", None, Some(&options)),
            "gemini-3.8-flash-high"
        );
    }

    #[test]
    fn agy_question_request_detection_and_response() {
        let request: RequestPermissionRequest = serde_json::from_value(json!({
            "sessionId": "session-1",
            "toolCall": {
                "toolCallId": "interaction_abc123",
                "title": "Which database would you prefer?",
                "rawInput": {
                    "questions": [
                        {
                            "question": "Which database would you prefer?",
                            "options": ["PostgreSQL", "SQLite", "MySQL"],
                            "is_multi_select": false
                        }
                    ]
                }
            },
            "options": [
                { "optionId": "1", "name": "PostgreSQL", "kind": "allow_once" },
                { "optionId": "2", "name": "SQLite", "kind": "allow_once" },
                { "optionId": "3", "name": "MySQL", "kind": "allow_once" }
            ]
        }))
        .unwrap();
        let params = serde_json::to_value(&request).unwrap();

        assert!(is_agy_question_request(&params, &request));

        let questions = agy_user_input_questions(&params, &request);
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].question, "Which database would you prefer?");
        assert_eq!(questions[0].options.len(), 3);
        assert_eq!(questions[0].options[0].label, "PostgreSQL");
        assert_eq!(questions[0].options[1].label, "SQLite");
        assert_eq!(questions[0].options[2].label, "MySQL");
        assert!(!questions[0].multi_select);

        let response = agy_user_input_response(
            &request,
            &[UserInputAnswer {
                question_id: "question_0".into(),
                answers: vec!["SQLite".into()],
            }],
        );

        match response.outcome {
            RequestPermissionOutcome::Selected(selected) => {
                assert_eq!(selected.option_id.to_string(), "2");
            }
            _ => panic!("Expected Selected outcome"),
        }
    }

    #[test]
    fn agy_extract_plan_link_path_extracts_file_links() {
        let text = "I have created the plan artifact. Review the plan in [plan.md](file:///Users/alice/.gemini/antigravity-acp/brain/123/plan.md). Please review.";
        let extracted = extract_plan_link_path(text);
        assert_eq!(
            extracted,
            Some(PathBuf::from(
                "/Users/alice/.gemini/antigravity-acp/brain/123/plan.md"
            ))
        );

        let text_with_encoded_spaces =
            "Review [plan.md](file:///Users/alice/My%20Projects/brain/plan.md).";
        let extracted = extract_plan_link_path(text_with_encoded_spaces);
        assert_eq!(
            extracted,
            Some(PathBuf::from("/Users/alice/My Projects/brain/plan.md"))
        );

        let non_plan_text = "Created [hello.rs](file:///Users/alice/project/hello.rs).";
        assert_eq!(extract_plan_link_path(non_plan_text), None);

        let windows_file_url = "Review in [plan.md](file:///C:/Users/alice/.gemini/antigravity-acp/brain/123/plan.md).";
        let extracted_win = extract_plan_link_path(windows_file_url);
        assert!(extracted_win.is_some());
        let win_path_str = extracted_win.unwrap().to_string_lossy().replace('\\', "/");
        assert!(win_path_str.ends_with("/plan.md"));
        assert!(win_path_str.contains("Users/alice"));

        let windows_raw_md_link =
            r"Review in [plan.md](C:\Users\alice\.gemini\antigravity-acp\brain\123\plan.md).";
        let extracted_raw_win = extract_plan_link_path(windows_raw_md_link);
        assert!(extracted_raw_win.is_some());
    }

    #[test]
    fn agy_is_plan_file_multiplatform_matrix() {
        // macOS / Linux unix paths
        assert!(is_plan_file(Path::new(
            "/Users/alice/.gemini/antigravity-acp/brain/123/plan.md"
        )));
        assert!(is_plan_file(Path::new(
            "/home/bob/.gemini/antigravity/brain/session/implementation_plan.markdown"
        )));
        assert!(is_plan_file(Path::new("/workspace/project/plan.md")));
        assert!(is_plan_file(Path::new("/workspace/project/PLAN.MD")));
        assert!(is_plan_file(Path::new(
            "/workspace/project/step1_plan.markdown"
        )));

        // Windows backslash paths
        assert!(is_plan_file(Path::new(
            r"C:\Users\alice\.gemini\antigravity-acp\brain\123\plan.md"
        )));
        assert!(is_plan_file(Path::new(
            r"C:\Users\alice\.gemini\antigravity-acp\brain\123\architecture.markdown"
        )));
        assert!(is_plan_file(Path::new(r"D:\Projects\app\PLAN.MD")));
        assert!(is_plan_file(Path::new(
            r"\\?\C:\Users\alice\.gemini\brain\123\plan.md"
        )));

        // Windows forward-slash paths
        assert!(is_plan_file(Path::new(
            "C:/Users/alice/.gemini/antigravity-acp/brain/123/plan.md"
        )));
        assert!(is_plan_file(Path::new("D:/Projects/app/custom_plan.md")));

        // Non-markdown files (must be false even in brain)
        assert!(!is_plan_file(Path::new(
            r"C:\Users\alice\.gemini\brain\output.json"
        )));
        assert!(!is_plan_file(Path::new(
            "/home/bob/.gemini/brain/script.py"
        )));
        assert!(!is_plan_file(Path::new("/Users/alice/project/plan.rs")));

        // Ordinary markdown files outside brain directories without "plan" in name (must be false)
        assert!(!is_plan_file(Path::new("/Users/alice/project/README.md")));
        assert!(!is_plan_file(Path::new(
            "/Users/alice/project/docs/brain.md"
        )));
        assert!(!is_plan_file(Path::new(
            r"C:\Users\alice\project\docs\brain.md"
        )));
    }

    #[test]
    fn agy_inlines_plan_artifact_when_created_via_tool_call() {
        let (events, event_rx) = crossbeam_channel::unbounded();
        let mut state = AcpStreamState::default();

        let tool_update = serde_json::from_value(json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "write_1",
            "title": "write_to_file",
            "kind": "edit",
            "status": "completed",
            "rawInput": {
                "TargetFile": "/Users/alice/.gemini/antigravity-acp/brain/123/plan.md",
                "CodeContent": "# Test Plan\n1. Do something"
            }
        }))
        .unwrap();

        handle_session_update(
            ProviderKind::Agy,
            SessionNotification::new("s", tool_update),
            &events,
            &mut state,
        )
        .unwrap();

        assert_eq!(
            state.agy.path.as_deref(),
            Some(Path::new(
                "/Users/alice/.gemini/antigravity-acp/brain/123/plan.md"
            ))
        );
        assert_eq!(
            state.agy.content.as_deref(),
            Some("# Test Plan\n1. Do something")
        );

        // Turn completes:
        maybe_inline_agy_plan(ProviderKind::Agy, &mut state.agy, &events);

        let seen = event_rx.try_iter().collect::<Vec<_>>();
        let inlined = seen.iter().find_map(|e| match e {
            DriverEvent::TextDelta(t) if t.contains("# Test Plan") => Some(t),
            _ => None,
        });
        assert!(inlined.is_some());
        assert!(state.agy.inlined);

        // Second call does not duplicate:
        maybe_inline_agy_plan(ProviderKind::Agy, &mut state.agy, &events);
        let seen2 = event_rx.try_iter().collect::<Vec<_>>();
        assert!(seen2.is_empty());
    }

    #[test]
    fn agy_inlines_plan_artifact_from_text_link() {
        let (events, event_rx) = crossbeam_channel::unbounded();
        let mut state = AcpStreamState::default();

        let temp_dir = std::env::temp_dir().join(format!("padu-test-{}", uuid::Uuid::new_v4()));
        let brain_dir = temp_dir.join("brain").join("test-session");
        std::fs::create_dir_all(&brain_dir).unwrap();
        let plan_path = brain_dir.join("plan.md");
        std::fs::write(&plan_path, "# File Plan Content\n- Step A\n- Step B").unwrap();

        let plan_url = url::Url::from_file_path(&plan_path).unwrap();
        let message_text = format!("I have created the plan. Review at [plan.md]({plan_url}).");

        let update = serde_json::from_value(json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": message_text}
        }))
        .unwrap();

        handle_session_update(
            ProviderKind::Agy,
            SessionNotification::new("s", update),
            &events,
            &mut state,
        )
        .unwrap();

        assert_eq!(state.agy.path.as_deref(), Some(plan_path.as_path()));

        // When turn ends, it reads the plan from disk and inlines it
        maybe_inline_agy_plan(ProviderKind::Agy, &mut state.agy, &events);

        let seen = event_rx.try_iter().collect::<Vec<_>>();
        let inlined = seen.iter().find_map(|e| match e {
            DriverEvent::TextDelta(t) if t.contains("# File Plan Content") => Some(t),
            _ => None,
        });
        assert!(inlined.is_some());
        assert!(inlined.unwrap().contains("- Step A"));
        assert!(state.agy.inlined);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn byte_prefix_backs_off_to_a_char_boundary() {
        // "é" is two bytes, so a 3-byte cut lands mid-character.
        let value = "éééé";
        assert_eq!(byte_prefix(value, 3), "é");
        assert_eq!(byte_prefix(value, 4), "éé");
        // Below the max the whole value comes back.
        assert_eq!(byte_prefix("abc", 60), "abc");
        // An ASCII cut is exact.
        assert_eq!(byte_prefix("abcdefgh", 4), "abcd");
    }

    #[test]
    fn inlining_a_plan_with_multibyte_plan_artifacts_does_not_panic() {
        let (events, event_rx) = crossbeam_channel::unbounded();
        let mut state = AgyPlanState::default();
        // A plan whose 60-byte mark falls inside a multi-byte character used
        // to panic the slice; it must inline cleanly instead.
        state.content = Some(format!("{}é{}", "a".repeat(59), "b".repeat(40)));

        maybe_inline_agy_plan(ProviderKind::Agy, &mut state, &events);

        assert!(state.inlined);
        let seen = event_rx.try_iter().collect::<Vec<_>>();
        assert!(seen.iter().any(|event| matches!(
            event,
            DriverEvent::TextDelta(text) if text.contains("bbbb")
        )));
    }
}
