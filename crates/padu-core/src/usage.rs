//! Account plan-usage limits per provider, read the way CodexBar reads them:
//! Claude's OAuth credential (macOS keychain first, then
//! `~/.claude/.credentials.json`) calls `api.anthropic.com/api/oauth/usage`; Codex's
//! `~/.codex/auth.json` token calls the ChatGPT backend's usage endpoint;
//! OpenCode Go's API key calls `opencode.ai/zen/go/v1/usage`; Grok answers
//! the `x.ai/billing` extension request on a short-lived `grok agent stdio`
//! probe. Payload shapes were verified against live responses or the
//! providers' own schemas, not guessed.
//!
//! Everything in this module blocks on subprocesses and the network and must
//! run on the background executor. Render reads only the parsed snapshot the
//! app entity stores.

use std::io::{BufRead as _, BufReader, Write as _};
use std::process::Stdio;

#[cfg(target_os = "macos")]
use std::process::Command;
use std::time::Duration;

use anyhow::{Context as _, anyhow};
use serde_json::{Value, json};

const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const CLAUDE_PROFILE_URL: &str = "https://api.anthropic.com/api/oauth/profile";
#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";
/// The usage endpoint rejects requests without this beta header.
const OAUTH_BETA_HEADER: &str = "oauth-2025-04-20";
/// User-Agent when the CLI's probed version is not known yet.
const FALLBACK_CLI_VERSION: &str = "2.1.0";

/// The absolute path keeps a shadowed `curl` on `PATH` out of the credential
/// exchange. Windows 10 build 17063 and later ship the same tool in System32.
#[cfg(not(windows))]
const CURL_PATH: &str = "/usr/bin/curl";
#[cfg(windows)]
const CURL_PATH: &str = r"C:\Windows\System32\curl.exe";

const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const OPENCODE_GO_USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";

pub use padu_protocol::usage::{PlanUsage, PlanWindow, format_tokens, reset_label};

struct OauthCredentials {
    access_token: String,
    subscription_type: Option<String>,
    rate_limit_tier: Option<String>,
}

/// Fetch the Claude account's plan usage. Blocking: keychain read, then one
/// HTTPS round trip. Never call from the UI thread.
pub fn fetch_claude_plan_usage(cli_version: Option<&str>) -> anyhow::Result<PlanUsage> {
    let credentials = read_credentials()?;
    let user_agent = format!(
        "claude-code/{}",
        cli_version.unwrap_or(FALLBACK_CLI_VERSION)
    );
    let (status, body) = http_get(
        CLAUDE_USAGE_URL,
        &[
            format!("Authorization: Bearer {}", credentials.access_token),
            format!("anthropic-beta: {OAUTH_BETA_HEADER}"),
            "Accept: application/json".to_owned(),
            format!("User-Agent: {user_agent}"),
        ],
    )?;
    match status {
        200 => {}
        401 | 403 => {
            return Err(anyhow!(tr!(
                "usage_error.signin_cannot_read",
                provider = "Claude Code",
                status = status
            )));
        }
        429 => return Err(anyhow!(tr!("usage_error.rate_limited"))),
        other => return Err(anyhow!(tr!("usage_error.http_status", status = other))),
    }
    let body: Value = serde_json::from_str(&body).context(tr!("usage_error.invalid_json"))?;
    let mut usage = parse_plan_usage(&body, &credentials);
    // The stored credential's tier is login-time metadata and survives plan
    // changes unchanged — verified live: a keychain saying `max_5x` against a
    // profile reporting `max_20x`. The profile's organization tier is the
    // account's current plan, so it wins; the credential label stays as the
    // fallback when the profile is unreachable. The same profile response
    // also carries the logged-in account identity when present.
    if let Some(profile) = fetch_claude_profile(&credentials.access_token, &user_agent) {
        if let Some(label) = profile_plan_label(&profile) {
            usage.plan_label = Some(label);
        }
        if let Some(account) = profile_account_label(&profile) {
            usage.account_label = Some(account);
        }
    }
    Ok(usage)
}

fn fetch_claude_profile(access_token: &str, user_agent: &str) -> Option<Value> {
    let (status, body) = http_get(
        CLAUDE_PROFILE_URL,
        &[
            format!("Authorization: Bearer {access_token}"),
            "Accept: application/json".to_owned(),
            format!("User-Agent: {user_agent}"),
        ],
    )
    .ok()?;
    if status != 200 {
        return None;
    }
    serde_json::from_str(&body).ok()
}

/// Logged-in Claude identity from the profile response, preferring the
/// account email and degrading through login/name fields. Anything blank or
/// missing yields `None` so callers hide the account row.
fn profile_account_label(body: &Value) -> Option<String> {
    let account = body.get("account")?;
    ["email_address", "email", "login", "name"]
        .into_iter()
        .filter_map(|key| account.get(key).and_then(Value::as_str))
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_owned)
}

/// "Max (20x)" from the profile's organization: `organization_type`
/// ("claude_max") names the plan, `rate_limit_tier`
/// ("default_claude_max_20x") carries the usage multiple.
fn profile_plan_label(body: &Value) -> Option<String> {
    let organization = body.get("organization")?;
    let tier = organization.get("rate_limit_tier").and_then(Value::as_str);
    let subscription = organization
        .get("organization_type")
        .and_then(Value::as_str)
        .and_then(|organization_type| organization_type.strip_prefix("claude_"));
    plan_label(subscription, tier)
}

/// Fetch the ChatGPT account's Codex rate limits: `~/.codex/auth.json` holds
/// the OAuth token and account id, and the ChatGPT backend answers with the
/// same primary/secondary windows the CLI's own status view shows. Blocking.
pub fn fetch_codex_plan_usage() -> anyhow::Result<PlanUsage> {
    let path = dirs::home_dir()
        .ok_or_else(|| anyhow!(tr!("usage_error.no_home_directory")))?
        .join(".codex/auth.json");
    let auth: Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .with_context(|| tr!("usage_error.read_file", path = path.display()))?,
    )
    .context(tr!("usage_error.codex_auth_invalid"))?;
    let access_token = auth
        .pointer("/tokens/access_token")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| anyhow!(tr!("usage_error.codex_access_token_missing")))?;
    let mut headers = vec![
        format!("Authorization: Bearer {access_token}"),
        "Accept: application/json".to_owned(),
        "User-Agent: padu".to_owned(),
    ];
    if let Some(account_id) = auth.pointer("/tokens/account_id").and_then(Value::as_str) {
        headers.push(format!("ChatGPT-Account-Id: {account_id}"));
    }
    let (status, body) = http_get(CODEX_USAGE_URL, &headers)?;
    match status {
        200 => {}
        401 | 403 => {
            return Err(anyhow!(tr!(
                "usage_error.signin_cannot_read",
                provider = "Codex",
                status = status
            )));
        }
        429 => return Err(anyhow!(tr!("usage_error.rate_limited"))),
        other => return Err(anyhow!(tr!("usage_error.http_status", status = other))),
    }
    let body: Value = serde_json::from_str(&body).context(tr!("usage_error.invalid_json"))?;
    let mut usage = parse_codex_plan_usage(&body)
        .ok_or_else(|| anyhow!(tr!("usage_error.no_rate_limit_windows")))?;
    // `~/.codex/auth.json` is the only local identity: prefer an explicit
    // email, then the JWT `id_token` claim, then the raw account id. The
    // usage endpoint itself carries quota, not identity.
    usage.account_label = codex_account_label(&auth).or_else(|| usage_body_account_label(&body));
    Ok(usage)
}

/// Identity from the Codex auth blob. Direct email fields win; ChatGPT also
/// stores an `id_token` JWT whose payload commonly carries the login email;
/// the opaque account id is the last resort. Never errors — unknown means
/// the settings row simply hides.
fn codex_account_label(auth: &Value) -> Option<String> {
    let direct = [
        "/tokens/email",
        "/tokens/user_email",
        "/email",
        "/user/email",
        "/user/email_address",
    ]
    .into_iter()
    .filter_map(|pointer| auth.pointer(pointer).and_then(Value::as_str))
    .map(str::trim)
    .find(|value| !value.is_empty())
    .map(str::to_owned);
    if direct.is_some() {
        return direct;
    }
    if let Some(email) = auth
        .pointer("/tokens/id_token")
        .and_then(Value::as_str)
        .and_then(jwt_payload_email)
    {
        return Some(email);
    }
    auth.pointer("/tokens/account_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Best-effort `email` claim from an unsigned JWT payload. Signature is
/// irrelevant here — this only labels local settings UI, never auth.
pub(crate) fn jwt_payload_email(token: &str) -> Option<String> {
    use base64::Engine as _;
    let payload = token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()
        .or_else(|| {
            base64::engine::general_purpose::STANDARD
                .decode(payload)
                .ok()
        })?;
    let claims: Value = serde_json::from_slice(&decoded).ok()?;
    ["email", "email_address", "upn", "preferred_username"]
        .into_iter()
        .filter_map(|key| claims.get(key).and_then(Value::as_str))
        .map(str::trim)
        .find(|value| !value.is_empty() && value.contains('@'))
        .map(str::to_owned)
}

/// Defensive identity from a usage response envelope, for backends that
/// inline the account next to quota. Returns `None` for today's Codex shape.
fn usage_body_account_label(body: &Value) -> Option<String> {
    ["/email", "/account/email", "/user/email", "/account_id"]
        .into_iter()
        .filter_map(|pointer| body.pointer(pointer).and_then(Value::as_str))
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Fetch OpenCode Go's rolling, weekly, and monthly subscription limits.
/// `None` means OpenCode has no Go credential, which is normal for people
/// using Zen or any of OpenCode's many other providers. Blocking.
pub fn fetch_opencode_go_plan_usage() -> anyhow::Result<Option<PlanUsage>> {
    let Some(api_key) = opencode_go_api_key() else {
        return Ok(None);
    };
    let (status, body) = http_get(
        OPENCODE_GO_USAGE_URL,
        &[
            format!("Authorization: Bearer {api_key}"),
            "Accept: application/json".to_owned(),
            "User-Agent: padu".to_owned(),
        ],
    )?;
    match status {
        200 => {}
        401 | 403 => {
            return Err(anyhow!(tr!(
                "usage_error.opencode_go_key_rejected",
                status = status
            )));
        }
        429 => return Err(anyhow!(tr!("usage_error.rate_limited"))),
        other => return Err(anyhow!(tr!("usage_error.http_status", status = other))),
    }
    let body: Value = serde_json::from_str(&body).context(tr!("usage_error.invalid_json"))?;
    parse_opencode_go_plan_usage(&body)
        .map(Some)
        .ok_or_else(|| anyhow!(tr!("usage_error.no_rate_limit_windows")))
}

/// Match OpenCode's credential precedence closely enough for its Go provider:
/// `OPENCODE_AUTH_CONTENT` replaces auth.json, a provider entry overrides the
/// catalog environment key, and `OPENCODE_API_KEY` remains the fallback.
fn opencode_go_api_key() -> Option<String> {
    let auth = std::env::var("OPENCODE_AUTH_CONTENT")
        .ok()
        .and_then(|payload| serde_json::from_str::<Value>(&payload).ok())
        .or_else(|| {
            let data_home = std::env::var_os("XDG_DATA_HOME")
                .filter(|path| !path.is_empty())
                .map(std::path::PathBuf::from)
                .or_else(|| dirs::home_dir().map(|home| home.join(".local/share")))?;
            let payload = std::fs::read_to_string(data_home.join("opencode/auth.json")).ok()?;
            serde_json::from_str(&payload).ok()
        });
    opencode_go_api_key_from_auth(auth.as_ref()).or_else(|| {
        std::env::var("OPENCODE_API_KEY")
            .ok()
            .map(|key| key.trim().to_owned())
            .filter(|key| !key.is_empty())
    })
}

fn opencode_go_api_key_from_auth(auth: Option<&Value>) -> Option<String> {
    let entry = auth?.get("opencode-go")?;
    if entry.get("type").and_then(Value::as_str) != Some("api") {
        return None;
    }
    entry
        .get("key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
}

/// Live shape verified on 2026-08-12. Standard Zen has no corresponding
/// `/zen/v1/usage` route, so these rows deliberately represent Go only.
fn parse_opencode_go_plan_usage(body: &Value) -> Option<PlanUsage> {
    let usage = body.get("usage")?;
    let windows = [
        ("rolling", tr!("usage.hour_limit", count = 5)),
        ("weekly", tr!("usage.weekly_limit")),
        ("monthly", tr!("usage.monthly_limit")),
    ]
    .into_iter()
    .filter_map(|(key, label)| {
        let window = usage.get(key)?;
        Some(PlanWindow {
            label,
            percent: window
                .get("percent")
                .and_then(Value::as_f64)?
                .clamp(0.0, 100.0),
            resets_at: window
                .get("resetsAt")
                .and_then(Value::as_str)
                .and_then(|reset| chrono::DateTime::parse_from_rfc3339(reset).ok())
                .map(|date| date.timestamp()),
        })
    })
    .collect::<Vec<_>>();
    if windows.is_empty() {
        return None;
    }
    Some(PlanUsage {
        plan_label: Some("Go".to_owned()),
        // Today's Go `/usage` envelope carries quota only; keep the
        // defensive lookup so a future identity field surfaces without a
        // second code change, and the row hides until then.
        account_label: usage_body_account_label(body),
        windows,
    })
}

/// Fetch Grok's plan usage by asking the agent itself: a short-lived
/// `grok agent stdio` process answers the `x.ai/billing` extension request
/// with the account's monthly quota. Blocking, bounded by timeouts, and the
/// probe process is always torn down.
pub fn fetch_grok_plan_usage(binary: &std::path::Path) -> anyhow::Result<PlanUsage> {
    let mut command = crate::command_env::command(binary);
    let command = command
        .args(["agent", "stdio"])
        .env("GROK_OAUTH2_REFERRER", "padu")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child =
        crate::command_env::spawn(command).context(tr!("usage_error.start_grok_probe"))?;
    let result = grok_billing_over_stdio(&mut child);
    // The probe has no shutdown request; ending it is the protocol.
    let _ = child.kill();
    let _ = child.wait();
    result.and_then(|billing| parse_grok_billing(&billing))
}

fn grok_billing_over_stdio(child: &mut std::process::Child) -> anyhow::Result<Value> {
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!(tr!("usage_error.grok_stdin_unavailable")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!(tr!("usage_error.grok_stdout_unavailable")))?;
    let (lines_tx, lines) = crossbeam_channel::unbounded::<Value>();
    std::thread::Builder::new()
        .name("padu-grok-usage-probe".into())
        .spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(value) = serde_json::from_str::<Value>(&line)
                    && lines_tx.send(value).is_err()
                {
                    break;
                }
            }
        })
        .context(tr!("usage_error.start_grok_reader"))?;

    let mut send = |id: u64, method: &str, params: Value| -> anyhow::Result<()> {
        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        serde_json::to_writer(&mut stdin, &message)?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        Ok(())
    };
    let wait_for = |id: u64| -> anyhow::Result<Value> {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let remaining = deadline
                .checked_duration_since(std::time::Instant::now())
                .ok_or_else(|| anyhow!(tr!("usage_error.grok_billing_timeout")))?;
            let message = lines
                .recv_timeout(remaining)
                .map_err(|_| anyhow!(tr!("usage_error.grok_billing_timeout")))?;
            if message.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = message.pointer("/error/message").and_then(Value::as_str) {
                return Err(anyhow!(tr!("usage_error.grok_answered", error = error)));
            }
            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    };

    // The same initialize the ACP session sends; billing is only served on an
    // initialized connection.
    send(
        1,
        "initialize",
        json!({
            "protocolVersion": "1",
            "clientCapabilities": {
                "fs": {"readTextFile": false, "writeTextFile": false},
                "terminal": false
            }
        }),
    )?;
    wait_for(1)?;
    send(2, "_x.ai/billing", json!({}))?;
    wait_for(2)
}

/// Map `_x.ai/billing` into the panel's rows. Verified live: current builds
/// wrap the billing config in a `config` object beside `subscription_tier`,
/// and unified-billing accounts may report a period without any percent
/// meter — the tier still names the plan then. Older flat shapes carry
/// `monthlyLimit`/`usage` totals instead of `creditUsagePercent`.
fn parse_grok_billing(billing: &Value) -> anyhow::Result<PlanUsage> {
    let config = billing
        .get("config")
        .filter(|config| config.is_object())
        .unwrap_or(billing);
    let plan_label = billing
        .get("subscription_tier")
        .or_else(|| config.get("subscription_tier"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|tier| !tier.is_empty())
        .map(str::to_owned);
    let percent = config
        .get("creditUsagePercent")
        .and_then(Value::as_f64)
        .or_else(|| {
            let limit = config
                .pointer("/monthlyLimit/val")
                .and_then(Value::as_f64)
                .filter(|limit| *limit > 0.0)?;
            let used = config
                .pointer("/usage/totalUsed/val")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            Some(used * 100.0 / limit)
        });

    let mut windows = Vec::new();
    if let Some(percent) = percent {
        let label = match config
            .pointer("/currentPeriod/type")
            .and_then(Value::as_str)
        {
            Some(period) if period.contains("WEEKLY") => tr!("usage.weekly_limit"),
            Some(period) if period.contains("DAILY") => tr!("usage.daily_limit"),
            _ => tr!("usage.monthly_limit"),
        };
        let resets_at = config
            .get("billingPeriodEnd")
            .or_else(|| config.pointer("/currentPeriod/end"))
            .or_else(|| config.pointer("/billingCycle/billingPeriodEnd"))
            .and_then(Value::as_str)
            .and_then(|end| chrono::DateTime::parse_from_rfc3339(end).ok())
            .map(|date| date.timestamp());
        windows.push(PlanWindow {
            label,
            percent: percent.clamp(0.0, 100.0),
            resets_at,
        });
    }
    if plan_label.is_none() && windows.is_empty() {
        return Err(anyhow!(tr!("usage_error.grok_no_billing_data")));
    }
    Ok(PlanUsage {
        plan_label,
        // Verified billing shapes carry the tier but no email; degrade
        // through likely identity keys and hide when absent.
        account_label: ["/email", "/account/email", "/user/email", "/username"]
            .into_iter()
            .filter_map(|pointer| {
                billing
                    .pointer(pointer)
                    .and_then(Value::as_str)
                    .or_else(|| {
                        billing
                            .get("config")
                            .and_then(|config| config.pointer(pointer))
                            .and_then(Value::as_str)
                    })
            })
            .map(str::trim)
            .find(|value| !value.is_empty())
            .map(str::to_owned),
        windows,
    })
}

/// Map the ChatGPT backend's usage response (primary/secondary windows in
/// seconds, plus model-scoped `additional_rate_limits`) into the panel's
/// rows.
fn parse_codex_plan_usage(body: &Value) -> Option<PlanUsage> {
    let mut windows = Vec::new();
    if let Some(rate_limit) = body.get("rate_limit") {
        push_codex_windows(&mut windows, rate_limit, None);
    }
    for entry in body
        .get("additional_rate_limits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(rate_limit) = entry.get("rate_limit") else {
            continue;
        };
        let name = entry
            .get("limit_name")
            .and_then(Value::as_str)
            .unwrap_or("Model");
        // The Spark bonus lane is promotional quota, not a limit the account
        // plans around; its row is noise next to the real lanes.
        if name.to_ascii_lowercase().contains("spark") {
            continue;
        }
        push_codex_windows(&mut windows, rate_limit, Some(name));
    }
    if windows.is_empty() {
        return None;
    }
    Some(PlanUsage {
        plan_label: openai_plan_label(body.get("plan_type").and_then(Value::as_str)),
        // Quota endpoint only; the caller overlays identity from auth.json.
        account_label: None,
        windows,
    })
}

/// The account-wide lanes read "5-hour limit"; a model-scoped lane reads
/// "Weekly · GPT-5.3-Codex-Spark", mirroring the Claude panel's scoped rows.
fn push_codex_windows(windows: &mut Vec<PlanWindow>, rate_limit: &Value, scope: Option<&str>) {
    for key in ["primary_window", "secondary_window"] {
        let Some(window) = rate_limit.get(key).filter(|window| !window.is_null()) else {
            continue;
        };
        let Some(percent) = window.get("used_percent").and_then(Value::as_f64) else {
            continue;
        };
        let minutes = window
            .get("limit_window_seconds")
            .and_then(Value::as_i64)
            .map(|seconds| seconds / 60);
        let base = window_label_from_minutes(minutes);
        let label = match scope {
            Some(name) => {
                let suffix = tr!("usage.limit_suffix");
                let period = base.strip_suffix(&suffix).unwrap_or(&base);
                tr!("usage.scoped_limit", period = period, name = name)
            }
            None => base,
        };
        windows.push(PlanWindow {
            label,
            percent: percent.clamp(0.0, 100.0),
            resets_at: window.get("reset_at").and_then(Value::as_i64),
        });
    }
}

/// "5-hour limit" / "Weekly limit" from a window duration, shared by the
/// Codex stream notification (minutes) and the ChatGPT usage endpoint
/// (seconds, converted by the caller).
pub fn window_label_from_minutes(minutes: Option<i64>) -> String {
    let Some(minutes) = minutes.filter(|minutes| *minutes > 0) else {
        return tr!("usage.usage_limit");
    };
    if minutes < 24 * 60 {
        tr!("usage.hour_limit", count = (minutes + 59) / 60)
    } else if minutes == 7 * 24 * 60 {
        tr!("usage.weekly_limit")
    } else {
        tr!(
            "usage.day_limit",
            count = (minutes + 24 * 60 - 1) / (24 * 60)
        )
    }
}

/// ChatGPT plan names, shared by the Codex stream notification and the usage
/// endpoint. The tier strings themselves encode the usage multiple — plain
/// `pro` is the 20x plan and `prolite` the 5x one (CodexBar ships the same
/// mapping). Unknown tiers stay unlabeled rather than guessing.
pub fn openai_plan_label(plan: Option<&str>) -> Option<String> {
    Some(
        match plan? {
            "free" | "free_workspace" | "guest" => "Free",
            "go" => "Go",
            "plus" => "Plus",
            "pro" => "Pro (20x)",
            "prolite" | "pro_lite" => "Pro (5x)",
            "team" => "Team",
            "business" | "self_serve_business_usage_based" => "Business",
            "enterprise" | "ent26" | "enterprise_cbp_usage_based" => "Enterprise",
            "edu" | "education" | "k12" => "Edu",
            _ => return None,
        }
        .to_owned(),
    )
}

/// The Claude Code OAuth blob: keychain on macOS, with the credentials file as
/// the cross-platform fallback. Claude Code stores the macOS item via
/// `security`, so `security` is on its ACL and this read does not prompt.
fn read_credentials() -> anyhow::Result<OauthCredentials> {
    #[cfg(target_os = "macos")]
    let payload = keychain_payload().or_else(|keychain_error| {
        credentials_file_payload()
            .map_err(|_| keychain_error.context(tr!("usage_error.claude_credentials_missing")))
    })?;
    #[cfg(not(target_os = "macos"))]
    let payload =
        credentials_file_payload().context(tr!("usage_error.claude_credentials_missing"))?;
    parse_credentials(&payload)
}

#[cfg(target_os = "macos")]
fn keychain_payload() -> anyhow::Result<String> {
    let output = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"])
        .stdin(Stdio::null())
        .output()
        .context(tr!("usage_error.run_security"))?;
    if !output.status.success() {
        return Err(anyhow!(tr!(
            "usage_error.keychain_item_missing",
            service = KEYCHAIN_SERVICE
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn credentials_file_payload() -> anyhow::Result<String> {
    let path = dirs::home_dir()
        .ok_or_else(|| anyhow!(tr!("usage_error.no_home_directory")))?
        .join(".claude/.credentials.json");
    std::fs::read_to_string(&path)
        .with_context(|| tr!("usage_error.read_file", path = path.display()))
}

fn parse_credentials(payload: &str) -> anyhow::Result<OauthCredentials> {
    let value: Value = serde_json::from_str(payload.trim())
        .context(tr!("usage_error.claude_credentials_invalid"))?;
    let oauth = value
        .get("claudeAiOauth")
        .ok_or_else(|| anyhow!(tr!("usage_error.claude_oauth_missing")))?;
    let access_token = oauth
        .get("accessToken")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| anyhow!(tr!("usage_error.claude_access_token_missing")))?
        .to_owned();
    let field = |name: &str| {
        oauth
            .get(name)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .filter(|value| !value.is_empty())
    };
    Ok(OauthCredentials {
        access_token,
        subscription_type: field("subscriptionType"),
        rate_limit_tier: field("rateLimitTier"),
    })
}

/// GET `url` with the given header lines. Headers travel to curl as a config
/// on stdin, never on argv, so bearer tokens cannot show up in the process
/// table. Shared with the usage-history rate-table fetch.
pub fn http_get(url: &str, headers: &[String]) -> anyhow::Result<(u16, String)> {
    let mut child = crate::command_env::plain_command(CURL_PATH)
        .args(["-sS", "--max-time", "15", "-D", "-", "-K", "-", url])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context(tr!("usage_error.run_curl"))?;
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!(tr!("usage_error.curl_stdin_unavailable")))?;
        for header in headers {
            writeln!(stdin, "header = \"{header}\"").context(tr!("usage_error.configure_curl"))?;
        }
    }
    let output = child
        .wait_with_output()
        .context(tr!("usage_error.curl_did_not_finish"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let error = stderr
            .lines()
            .last()
            .map(str::trim)
            .filter(|error| !error.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| tr!("usage_error.unknown_error"));
        return Err(anyhow!(tr!("usage_error.curl_failed", error = error)));
    }
    split_status_and_body(&String::from_utf8_lossy(&output.stdout))
}

/// `-D -` prefixes the body with the response headers; the status code is on
/// the first line and the body follows the blank separator line.
fn split_status_and_body(raw: &str) -> anyhow::Result<(u16, String)> {
    let status = raw
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| anyhow!(tr!("usage_error.curl_no_status")))?;
    let body = raw
        .split_once("\r\n\r\n")
        .or_else(|| raw.split_once("\n\n"))
        .map(|(_, body)| body)
        .unwrap_or_default();
    Ok((status, body.to_owned()))
}

fn parse_plan_usage(body: &Value, credentials: &OauthCredentials) -> PlanUsage {
    let mut windows = limit_entry_windows(body);
    if windows.is_empty() {
        windows = flat_field_windows(body);
    }
    PlanUsage {
        plan_label: plan_label(
            credentials.subscription_type.as_deref(),
            credentials.rate_limit_tier.as_deref(),
        ),
        // The OAuth blob holds tokens, not identity; the profile fetch in
        // `fetch_claude_plan_usage` overlays the account email when reachable.
        account_label: None,
        windows,
    }
}

/// The modern shape: a `limits` array whose entries carry a `kind` and, for
/// model-scoped lanes, the model's display name.
fn limit_entry_windows(body: &Value) -> Vec<PlanWindow> {
    let Some(limits) = body.get("limits").and_then(Value::as_array) else {
        return Vec::new();
    };
    limits
        .iter()
        .filter_map(|entry| {
            let label = match entry.get("kind").and_then(Value::as_str)? {
                "session" => tr!("usage.hour_limit", count = 5),
                "weekly_all" => tr!("usage.weekly_all_models"),
                "weekly_scoped" => {
                    let model = entry
                        .pointer("/scope/model/display_name")
                        .and_then(Value::as_str)
                        .unwrap_or("model");
                    tr!("usage.weekly_model", model = model)
                }
                // Overage/credit lanes render elsewhere if ever wanted; the
                // meter mirrors the CLI's three quota rows.
                _ => return None,
            };
            Some(PlanWindow {
                label,
                percent: entry
                    .get("percent")
                    .and_then(Value::as_f64)?
                    .clamp(0.0, 100.0),
                resets_at: parse_reset(entry.get("resets_at")),
            })
        })
        .collect()
}

/// The older flat shape, kept as a fallback for accounts the `limits` array
/// has not reached.
fn flat_field_windows(body: &Value) -> Vec<PlanWindow> {
    [
        "five_hour",
        "seven_day",
        "seven_day_opus",
        "seven_day_sonnet",
    ]
    .into_iter()
    .filter_map(|key| {
        let window = body.get(key)?;
        let label = match key {
            "five_hour" => tr!("usage.hour_limit", count = 5),
            "seven_day" => tr!("usage.weekly_all_models"),
            "seven_day_opus" => tr!("usage.weekly_model", model = "Opus"),
            "seven_day_sonnet" => tr!("usage.weekly_model", model = "Sonnet"),
            _ => return None,
        };
        Some(PlanWindow {
            label,
            percent: window
                .get("utilization")
                .and_then(Value::as_f64)?
                .clamp(0.0, 100.0),
            resets_at: parse_reset(window.get("resets_at")),
        })
    })
    .collect()
}

fn parse_reset(value: Option<&Value>) -> Option<i64> {
    let text = value?.as_str()?;
    chrono::DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|date| date.timestamp())
}

/// "Max (5x)" from `subscriptionType: "max"` + `rateLimitTier:
/// "default_claude_max_5x"`, matching how the CLI titles its usage panel.
fn plan_label(subscription_type: Option<&str>, rate_limit_tier: Option<&str>) -> Option<String> {
    let tier_words = rate_limit_tier
        .map(|tier| {
            tier.to_ascii_lowercase()
                .split(['_', '-', ' '])
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let base = subscription_type.map(str::to_ascii_lowercase).or_else(|| {
        ["max", "pro", "team", "enterprise"]
            .into_iter()
            .find(|plan| tier_words.iter().any(|word| word == plan))
            .map(str::to_owned)
    })?;
    let mut label = match base.as_str() {
        "max" => "Max".to_owned(),
        "pro" => "Pro".to_owned(),
        "team" => "Team".to_owned(),
        "enterprise" => "Enterprise".to_owned(),
        other => {
            let mut chars = other.chars();
            let first = chars.next()?;
            first.to_uppercase().collect::<String>() + chars.as_str()
        }
    };
    if base == "max"
        && let Some(position) = tier_words.iter().position(|word| word == "max")
        && let Some(multiplier) = tier_words.get(position + 1)
        && multiplier.ends_with('x')
        && multiplier[..multiplier.len() - 1].parse::<u32>().is_ok()
    {
        label = format!("{label} ({multiplier})");
    }
    Some(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trimmed from a live response captured on 2026-08-07.
    const LIVE_BODY: &str = r#"{
        "five_hour": {"utilization": 41.0, "resets_at": "2026-08-07T14:59:59.729061+00:00"},
        "seven_day": {"utilization": 20.0, "resets_at": "2026-08-13T11:59:59.729091+00:00"},
        "seven_day_opus": null,
        "limits": [
            {"kind": "session", "group": "session", "percent": 41, "severity": "normal",
             "resets_at": "2026-08-07T14:59:59.729061+00:00", "scope": null, "is_active": true},
            {"kind": "weekly_all", "group": "weekly", "percent": 20, "severity": "normal",
             "resets_at": "2026-08-13T11:59:59.729091+00:00", "scope": null, "is_active": false},
            {"kind": "weekly_scoped", "group": "weekly", "percent": 38, "severity": "normal",
             "resets_at": "2026-08-13T11:59:59.729307+00:00",
             "scope": {"model": {"id": null, "display_name": "Fable"}, "surface": null},
             "is_active": false}
        ]
    }"#;

    fn credentials(tier: Option<&str>, subscription: Option<&str>) -> OauthCredentials {
        OauthCredentials {
            access_token: "token".into(),
            subscription_type: subscription.map(str::to_owned),
            rate_limit_tier: tier.map(str::to_owned),
        }
    }

    #[test]
    fn parses_the_limits_array_into_the_three_quota_rows() {
        let body: Value = serde_json::from_str(LIVE_BODY).unwrap();
        let usage = parse_plan_usage(
            &body,
            &credentials(Some("default_claude_max_5x"), Some("max")),
        );
        assert_eq!(usage.plan_label.as_deref(), Some("Max (5x)"));
        let rows = usage
            .windows
            .iter()
            .map(|window| (window.label.as_str(), window.percent))
            .collect::<Vec<_>>();
        assert_eq!(
            rows,
            [
                ("5-hour limit", 41.0),
                ("Weekly · all models", 20.0),
                ("Weekly · Fable", 38.0),
            ]
        );
        assert!(
            usage
                .windows
                .iter()
                .all(|window| window.resets_at.is_some())
        );
    }

    #[test]
    fn falls_back_to_flat_fields_when_the_limits_array_is_missing() {
        let body: Value = serde_json::from_str(
            r#"{
                "five_hour": {"utilization": 12.5, "resets_at": "2026-08-07T14:59:59+00:00"},
                "seven_day": {"utilization": 3.0, "resets_at": "2026-08-13T11:59:59+00:00"},
                "seven_day_opus": {"utilization": 7.0, "resets_at": "2026-08-13T11:59:59+00:00"}
            }"#,
        )
        .unwrap();
        let usage = parse_plan_usage(&body, &credentials(None, Some("pro")));
        assert_eq!(usage.plan_label.as_deref(), Some("Pro"));
        assert_eq!(
            usage
                .windows
                .iter()
                .map(|window| window.label.as_str())
                .collect::<Vec<_>>(),
            ["5-hour limit", "Weekly · all models", "Weekly · Opus"]
        );
    }

    #[test]
    fn the_profile_organization_names_the_live_plan() {
        // Shape captured live on 2026-08-07: the keychain still said 5x while
        // the profile reported the account's actual 20x tier.
        let body: Value = serde_json::from_str(
            r#"{
                "account": {"has_claude_max": true},
                "organization": {
                    "organization_type": "claude_max",
                    "billing_type": "stripe_subscription",
                    "rate_limit_tier": "default_claude_max_20x"
                }
            }"#,
        )
        .unwrap();
        assert_eq!(profile_plan_label(&body).as_deref(), Some("Max (20x)"));
        assert_eq!(profile_plan_label(&serde_json::json!({})), None);
    }

    #[test]
    fn the_profile_account_names_the_logged_in_user() {
        let body: Value = serde_json::from_str(
            r#"{
                "account": {"email_address": "dev@example.com", "has_claude_max": true},
                "organization": {
                    "organization_type": "claude_max",
                    "rate_limit_tier": "default_claude_max_20x"
                }
            }"#,
        )
        .unwrap();
        assert_eq!(
            profile_account_label(&body).as_deref(),
            Some("dev@example.com")
        );
        let login_only: Value =
            serde_json::from_str(r#"{"account": {"login": "dev-handle"}}"#).unwrap();
        assert_eq!(
            profile_account_label(&login_only).as_deref(),
            Some("dev-handle")
        );
        assert_eq!(
            profile_account_label(&serde_json::json!({"account": {}})),
            None
        );
        assert_eq!(profile_account_label(&serde_json::json!({})), None);
    }

    #[test]
    fn codex_identity_prefers_email_then_jwt_then_account_id() {
        use base64::Engine as _;
        // Direct email field wins.
        let direct: Value = serde_json::json!({
            "tokens": {"access_token": "x", "account_id": "acc-1", "email": "dev@example.com"}
        });
        assert_eq!(
            codex_account_label(&direct).as_deref(),
            Some("dev@example.com")
        );
        // Otherwise the id_token JWT payload's email claim is decoded.
        let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::json!({"email": "jwt@example.com"}).to_string());
        let jwt = format!("header.{claims}.signature");
        let via_jwt: Value = serde_json::json!({"tokens": {"access_token": "x", "id_token": jwt}});
        assert_eq!(
            codex_account_label(&via_jwt).as_deref(),
            Some("jwt@example.com")
        );
        // Opaque account id is the last resort, not an error.
        let id_only: Value =
            serde_json::json!({"tokens": {"access_token": "x", "account_id": "acc-9"}});
        assert_eq!(codex_account_label(&id_only).as_deref(), Some("acc-9"));
        assert_eq!(
            codex_account_label(&serde_json::json!({"tokens": {}})),
            None
        );
    }

    #[test]
    fn quota_only_bodies_report_no_account() {
        assert_eq!(
            usage_body_account_label(&serde_json::json!({
                "plan_type": "plus",
                "rate_limit": {"primary_window": {"used_percent": 1}}
            })),
            None
        );
    }

    #[test]
    fn plan_labels_cover_tier_multipliers_and_missing_metadata() {
        assert_eq!(
            plan_label(Some("max"), Some("default_claude_max_20x")).as_deref(),
            Some("Max (20x)")
        );
        // The tier alone still names the plan.
        assert_eq!(
            plan_label(None, Some("default_claude_max_5x")).as_deref(),
            Some("Max (5x)")
        );
        assert_eq!(plan_label(Some("pro"), None).as_deref(), Some("Pro"));
        assert_eq!(plan_label(None, None), None);
    }

    #[test]
    fn credentials_parse_reads_the_keychain_blob_shape() {
        let parsed = parse_credentials(
            r#"{"claudeAiOauth": {"accessToken": "sk-ant-oat01-abc",
                "subscriptionType": "max", "rateLimitTier": "default_claude_max_5x"}}"#,
        )
        .unwrap();
        assert_eq!(parsed.access_token, "sk-ant-oat01-abc");
        assert_eq!(parsed.subscription_type.as_deref(), Some("max"));
        assert!(parse_credentials(r#"{"mcpOAuth": {}}"#).is_err());
    }

    #[test]
    fn status_line_and_body_split_from_curl_header_dump() {
        let (status, body) =
            split_status_and_body("HTTP/2 200 \r\ncontent-type: application/json\r\n\r\n{\"a\":1}")
                .unwrap();
        assert_eq!(status, 200);
        assert_eq!(body, "{\"a\":1}");
    }

    #[test]
    fn codex_usage_endpoint_maps_windows_and_plan() {
        // Shape mirrors the ChatGPT backend response CodexBar decodes.
        let body: Value = serde_json::from_str(
            r#"{
                "plan_type": "plus",
                "rate_limit": {
                    "primary_window":
                        {"used_percent": 37, "reset_at": 1800000000, "limit_window_seconds": 18000},
                    "secondary_window":
                        {"used_percent": 8, "reset_at": 1800500000, "limit_window_seconds": 604800}
                }
            }"#,
        )
        .unwrap();
        let usage = parse_codex_plan_usage(&body).expect("both windows should map");
        assert_eq!(usage.plan_label.as_deref(), Some("Plus"));
        assert_eq!(
            usage
                .windows
                .iter()
                .map(|window| (window.label.as_str(), window.percent, window.resets_at))
                .collect::<Vec<_>>(),
            [
                ("5-hour limit", 37.0, Some(1_800_000_000)),
                ("Weekly limit", 8.0, Some(1_800_500_000)),
            ]
        );
        assert!(parse_codex_plan_usage(&serde_json::json!({"plan_type": "plus"})).is_none());
    }

    #[test]
    fn codex_model_scoped_limits_become_named_rows_except_spark() {
        // Shape captured live on 2026-08-07: a Pro account with a weekly
        // account lane plus model-scoped weekly lanes. The Spark bonus lane
        // is dropped; other scoped lanes keep their named rows.
        let body: Value = serde_json::from_str(
            r#"{
                "plan_type": "pro",
                "rate_limit": {
                    "primary_window":
                        {"used_percent": 99, "limit_window_seconds": 604800, "reset_at": 1786160310},
                    "secondary_window": null
                },
                "additional_rate_limits": [{
                    "limit_name": "GPT-5.3-Codex-Spark",
                    "rate_limit": {
                        "primary_window":
                            {"used_percent": 0, "limit_window_seconds": 604800, "reset_at": 1786720969}
                    }
                }, {
                    "limit_name": "GPT-5.3-Codex",
                    "rate_limit": {
                        "primary_window":
                            {"used_percent": 12, "limit_window_seconds": 604800, "reset_at": 1786720969}
                    }
                }]
            }"#,
        )
        .unwrap();
        let usage = parse_codex_plan_usage(&body).unwrap();
        assert_eq!(usage.plan_label.as_deref(), Some("Pro (20x)"));
        assert_eq!(
            usage
                .windows
                .iter()
                .map(|window| (window.label.as_str(), window.percent))
                .collect::<Vec<_>>(),
            [("Weekly limit", 99.0), ("Weekly · GPT-5.3-Codex", 12.0),]
        );
    }

    #[test]
    fn opencode_go_usage_maps_live_subscription_windows() {
        // Exact response shape captured from the live Go endpoint on
        // 2026-08-12. Percentages vary by account; the envelope and reset
        // timestamps are the contract this parser relies on.
        let body: Value = serde_json::from_str(
            r#"{
                "usage": {
                    "rolling": {
                        "status": "ok",
                        "percent": 0,
                        "resetsAt": "2026-08-12T05:52:15.153Z"
                    },
                    "weekly": {
                        "status": "ok",
                        "percent": 8,
                        "resetsAt": "2026-08-17T00:00:00.153Z"
                    },
                    "monthly": {
                        "status": "ok",
                        "percent": 36,
                        "resetsAt": "2026-09-01T13:40:30.153Z"
                    }
                }
            }"#,
        )
        .unwrap();
        let usage = parse_opencode_go_plan_usage(&body).expect("all Go windows should map");
        assert_eq!(usage.plan_label.as_deref(), Some("Go"));
        assert_eq!(
            usage
                .windows
                .iter()
                .map(|window| (window.label.as_str(), window.percent))
                .collect::<Vec<_>>(),
            [
                ("5-hour limit", 0.0),
                ("Weekly limit", 8.0),
                ("Monthly limit", 36.0),
            ]
        );
        assert!(
            usage
                .windows
                .iter()
                .all(|window| window.resets_at.is_some())
        );
        assert!(parse_opencode_go_plan_usage(&serde_json::json!({"usage": {}})).is_none());
    }

    #[test]
    fn opencode_auth_uses_only_the_go_provider_entry() {
        let auth = serde_json::json!({
            "opencode": {"type": "api", "key": "zen-key"},
            "opencode-go": {"type": "api", "key": " go-key "}
        });
        assert_eq!(
            opencode_go_api_key_from_auth(Some(&auth)).as_deref(),
            Some("go-key")
        );
        assert_eq!(
            opencode_go_api_key_from_auth(Some(&serde_json::json!({
                "opencode": {"type": "api", "key": "zen-key"}
            }))),
            None
        );
        assert_eq!(
            opencode_go_api_key_from_auth(Some(&serde_json::json!({
                "opencode-go": {"type": "oauth", "key": "wrong-kind"}
            }))),
            None
        );
    }

    #[test]
    fn grok_billing_maps_to_a_monthly_window() {
        // Older flat shape (CodexBar's GrokBillingResponse fixture).
        let billing: Value = serde_json::from_str(
            r#"{
                "billingCycle": {
                    "billingPeriodStart": "2026-05-01T00:00:00Z",
                    "billingPeriodEnd": "2026-06-01T00:00:00Z"
                },
                "monthlyLimit": {"val": 99900},
                "usage": {"includedUsed": {"val": 49950}, "totalUsed": {"val": 49950}}
            }"#,
        )
        .unwrap();
        let usage = parse_grok_billing(&billing).expect("the monthly lane should map");
        assert_eq!(usage.windows.len(), 1);
        assert_eq!(usage.windows[0].label, "Monthly limit");
        assert_eq!(usage.windows[0].percent, 50.0);
        assert_eq!(usage.windows[0].resets_at, Some(1_780_272_000));
        // Nothing reported means signed out, not a zero-width meter.
        assert!(parse_grok_billing(&serde_json::json!({"usage": {}})).is_err());
    }

    #[test]
    fn grok_unified_billing_keeps_the_tier_without_meters() {
        // Captured live from `_x.ai/billing` on 2026-08-07: the config is
        // enveloped, the period is weekly, and no percent meter is exposed.
        let billing: Value = serde_json::from_str(
            r#"{
                "config": {
                    "currentPeriod": {
                        "type": "USAGE_PERIOD_TYPE_WEEKLY",
                        "start": "2026-08-06T15:32:05.102798+00:00",
                        "end": "2026-08-13T15:32:05.102798+00:00"
                    },
                    "onDemandCap": {"val": 0},
                    "isUnifiedBillingUser": true,
                    "billingPeriodStart": "2026-08-06T15:32:05.102798+00:00",
                    "billingPeriodEnd": "2026-08-13T15:32:05.102798+00:00"
                },
                "subscription_tier": "X Premium"
            }"#,
        )
        .unwrap();
        let usage = parse_grok_billing(&billing).expect("the tier alone still labels the plan");
        assert_eq!(usage.plan_label.as_deref(), Some("X Premium"));
        assert!(usage.windows.is_empty());

        // The same envelope with a percent produces a weekly lane.
        let mut with_percent = billing.clone();
        with_percent["config"]["creditUsagePercent"] = serde_json::json!(37.5);
        let usage = parse_grok_billing(&with_percent).unwrap();
        assert_eq!(usage.windows.len(), 1);
        assert_eq!(usage.windows[0].label, "Weekly limit");
        assert_eq!(usage.windows[0].percent, 37.5);
        assert!(usage.windows[0].resets_at.is_some());
    }

    #[test]
    fn token_counts_format_like_the_cli_meter() {
        assert_eq!(format_tokens(950), "950");
        assert_eq!(format_tokens(87_650), "87.7k");
        assert_eq!(format_tokens(999_600), "1.0M");
        assert_eq!(format_tokens(1_000_000), "1.0M");
    }

    #[test]
    fn reset_labels_stay_relative_until_a_day_out() {
        let now = 1_700_000_000;
        assert_eq!(reset_label(now + 49 * 60, now), "Resets in 49 min");
        assert_eq!(
            reset_label(now + 3 * 3600 + 120, now),
            "Resets in 3 hr 2 min"
        );
        assert_eq!(reset_label(now - 5, now), "Resets soon");
        // Beyond a day the label goes absolute in local time; the exact text
        // depends on the machine's zone, so assert only the shape.
        let far = reset_label(now + 3 * 24 * 3600, now);
        assert!(far.starts_with("Resets ") && !far.contains(" in "), "{far}");
    }
}
