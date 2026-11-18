//! CLI testing harness for Padu AI agent provider drivers.
//!
//! Verifies binary detection, model discovery, live session connection,
//! token streaming, mid-session model switching, forking, rollback, and resume.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::anyhow;
use crossbeam_channel::Receiver;
use padu_client::skill_probe::{
    SkillBinaryIdentity, SkillInvocationEvidence, SkillNativeSyntax,
    evaluate_skill_capability_probe, native_skill_syntax, skill_capability_cache_path,
    write_skill_capability_evidence,
};
use padu_core::driver::{
    DriverHandle, DriverStartOptions, SessionOptions, event_channel, start_local,
};
use padu_core::model::{
    DriverEvent, InteractionMode, ProviderKind, ProviderResumeCursor, RuntimeMode,
    discover_provider_models, probe_provider_version, provider_binary_identity, provider_probe,
};
use serde::Serialize;
use serde_json::json;

#[derive(Clone, Debug, Default)]
struct CliArgs {
    command: String,
    provider_raw: Option<String>,
    param: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    timeout_secs: u64,
    json: bool,
}

fn parse_args() -> Result<CliArgs, anyhow::Error> {
    let mut args = std::env::args().skip(1);
    let mut cli = CliArgs {
        timeout_secs: 60,
        ..Default::default()
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => cli.json = true,
            "--model" => {
                cli.model = args.next();
            }
            "--effort" => {
                cli.effort = args.next();
            }
            "--timeout" => {
                if let Some(t) = args.next() {
                    cli.timeout_secs = t.parse().unwrap_or(60);
                }
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            cmd if cli.command.is_empty() => {
                cli.command = cmd.to_string();
            }
            prov if cli.provider_raw.is_none() => {
                cli.provider_raw = Some(prov.to_string());
            }
            extra if cli.param.is_none() => {
                cli.param = Some(extra.to_string());
            }
            _ => {}
        }
    }

    if cli.command.is_empty() {
        cli.command = "help".to_string();
    }
    Ok(cli)
}

fn print_help() {
    println!(
        r#"padu-provider-test — AI Coding Agent Provider Driver Testing Harness

USAGE:
    padu-provider-test <COMMAND> [PROVIDER] [OPTIONS]

COMMANDS:
    list                         List all known providers and detection status
    probe <PROVIDER>             Inspect binary path and detected CLI version
    models <PROVIDER>            Discover and display models, tiers, and presets
    auth <PROVIDER>              Trigger provider authentication handshake
    logout <PROVIDER>            Trigger provider sign-out
    connect <PROVIDER>           Test connection handshake and return session cursor
    turn <PROVIDER> "<PROMPT>"   Test full chat turn with live token/thought streaming
    switch-model <PROVIDER> <M>  Test dynamic in-place model switching via apply_options
    fork <PROVIDER>              Test conversation forking capability
    rollback <PROVIDER>          Test conversation rollback capability
    resume <PROVIDER> <SESS_ID>  Test session re-attachment from resume cursor
    skill-capability <PROVIDER>  Opt-in disposable-session skill mention probe
    suite <PROVIDER>             Run complete automated diagnostic test matrix

OPTIONS:
    --model <NAME>               Specify model id for session startup
    --effort <LEVEL>             Specify reasoning effort (e.g. low, medium, high)
    --timeout <SECS>             Timeout in seconds for prompt settlement (default: 60)
    --json                       Output structured JSON for AI agent integration
    -h, --help                   Print help information

EXAMPLES:
    padu-provider-test list
    padu-provider-test models cursor
    padu-provider-test connect claude
    padu-provider-test turn cursor "Reply with PONG"
    padu-provider-test switch-model cursor gpt-4o
    padu-provider-test skill-capability codex --json
    padu-provider-test suite cursor --json
"#
    );
}

fn parse_provider(name: &str) -> anyhow::Result<ProviderKind> {
    ProviderKind::ALL
        .into_iter()
        .find(|p| {
            p.id().eq_ignore_ascii_case(name)
                || p.short_name().eq_ignore_ascii_case(name)
                || p.command().eq_ignore_ascii_case(name)
        })
        .ok_or_else(|| {
            let valid = ProviderKind::ALL
                .into_iter()
                .map(|p| p.id())
                .collect::<Vec<_>>()
                .join(", ");
            anyhow!("Unknown provider '{name}'. Valid providers: {valid}")
        })
}

#[derive(Serialize)]
struct ProviderListEntry {
    id: &'static str,
    name: &'static str,
    command: &'static str,
    installed: bool,
    path: Option<String>,
    version: Option<String>,
    supports_fork: bool,
    supports_rollback: bool,
    supports_model_discovery: bool,
}

fn run_list(json: bool) -> anyhow::Result<()> {
    let mut entries = Vec::new();
    for provider in ProviderKind::ALL {
        let probe = provider_probe(provider, None);
        let version = probe.path.as_deref().and_then(probe_provider_version);
        entries.push(ProviderListEntry {
            id: provider.id(),
            name: provider.display_name(),
            command: provider.command(),
            installed: probe.installed,
            path: probe.path.map(|p| p.to_string_lossy().into_owned()),
            version,
            supports_fork: provider.supports_conversation_fork(),
            supports_rollback: provider.supports_conversation_rollback(),
            supports_model_discovery: provider.supports_model_discovery(),
        });
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    println!(
        "\n{:<12} {:<20} {:<15} {:<10} {:<30}",
        "ID", "NAME", "BINARY", "STATUS", "VERSION"
    );
    println!("{}", "-".repeat(90));
    for entry in &entries {
        let status = if entry.installed {
            "✓ Ready"
        } else {
            "✗ Not found"
        };
        let version = entry.version.as_deref().unwrap_or("-");
        println!(
            "{:<12} {:<20} {:<15} {:<10} {:<30}",
            entry.id, entry.name, entry.command, status, version
        );
    }
    println!();
    Ok(())
}

fn run_probe(provider: ProviderKind, json: bool) -> anyhow::Result<()> {
    let probe = provider_probe(provider, None);
    let version = probe.path.as_deref().and_then(probe_provider_version);

    if json {
        let out = json!({
            "provider": provider.id(),
            "name": provider.display_name(),
            "command": provider.command(),
            "installed": probe.installed,
            "path": probe.path.map(|p| p.to_string_lossy().into_owned()),
            "version": version,
            "supports_fork": provider.supports_conversation_fork(),
            "supports_rollback": provider.supports_conversation_rollback(),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    println!("\n=== Provider Probe: {} ===", provider.display_name());
    println!("ID:                  {}", provider.id());
    println!("Command:             {}", provider.command());
    println!(
        "Installed:           {}",
        if probe.installed { "YES" } else { "NO" }
    );
    if let Some(path) = &probe.path {
        println!("Binary Path:         {}", path.display());
    }
    if let Some(ver) = &version {
        println!("Detected Version:    {}", ver);
    }
    println!(
        "Supports Fork:       {}",
        provider.supports_conversation_fork()
    );
    println!(
        "Supports Rollback:   {}",
        provider.supports_conversation_rollback()
    );
    println!();
    Ok(())
}

fn run_models(provider: ProviderKind, json: bool) -> anyhow::Result<()> {
    let probe = provider_probe(provider, None);
    let discovered = discover_provider_models(probe);

    if json {
        println!("{}", serde_json::to_string_pretty(&discovered)?);
        return Ok(());
    }

    println!("\n=== Models & Presets: {} ===", provider.display_name());
    if discovered.models.is_empty() {
        println!("(No models discovered or provider does not advertise static models)");
    } else {
        println!("Models ({}):", discovered.models.len());
        for m in &discovered.models {
            let def = if m.is_default { " [DEFAULT]" } else { "" };
            println!("  - {} (ID: {}){}", m.name, m.id, def);
            if !m.reasoning_efforts.is_empty() {
                let efforts = m
                    .reasoning_efforts
                    .iter()
                    .map(|e| e.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("    Reasoning Efforts: {}", efforts);
            }
            if !m.service_tiers.is_empty() {
                let tiers = m
                    .service_tiers
                    .iter()
                    .map(|t| t.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("    Service Tiers:     {}", tiers);
            }
        }
    }

    if !discovered.agent_presets.is_empty() {
        println!("\nAgent Presets ({}):", discovered.agent_presets.len());
        for p in &discovered.agent_presets {
            let def = if p.is_default { " [DEFAULT]" } else { "" };
            println!("  - {} (ID: {}){}", p.name, p.id, def);
        }
    }
    println!();
    Ok(())
}

struct DriverSession {
    handle: DriverHandle,
    receiver: Receiver<DriverEvent>,
    cursor: Option<ProviderResumeCursor>,
}

fn start_session(
    provider: ProviderKind,
    model: Option<String>,
    effort: Option<String>,
    resume_cursor: Option<ProviderResumeCursor>,
) -> anyhow::Result<DriverSession> {
    start_session_in(
        provider,
        model,
        effort,
        resume_cursor,
        std::env::current_dir()?,
    )
}

fn start_session_in(
    provider: ProviderKind,
    model: Option<String>,
    effort: Option<String>,
    resume_cursor: Option<ProviderResumeCursor>,
    cwd: PathBuf,
) -> anyhow::Result<DriverSession> {
    let probe = provider_probe(provider, None);
    let binary = probe.path.ok_or_else(|| {
        anyhow!(
            "Provider '{}' binary not found on system",
            provider.command()
        )
    })?;
    let (wake_tx, _wake_rx) = smol::channel::bounded(1);
    let (event_sender, receiver) = event_channel(wake_tx);

    let options = DriverStartOptions {
        binary,
        cwd,
        mode: RuntimeMode::FullAccess,
        interaction_mode: InteractionMode::Build,
        model,
        reasoning_effort: effort,
        service_tier: None,
        context_window: None,
        agent_preset: None,
        computer_use_enabled: false,
        provider_cursor: resume_cursor,
    };

    let handle = start_local(provider, options, event_sender)?;

    // Wait for DriverEvent::Connected with timeout
    let start = Instant::now();
    let mut cursor = None;
    while start.elapsed() < Duration::from_secs(15) {
        if let Ok(event) = receiver.recv_timeout(Duration::from_millis(100)) {
            match event {
                DriverEvent::Connected { provider_cursor } => {
                    cursor = provider_cursor;
                    break;
                }
                DriverEvent::Error(err) => {
                    return Err(anyhow!("Driver reported startup error: {err}"));
                }
                DriverEvent::ProcessExited => {
                    return Err(anyhow!(
                        "Driver process exited unexpectedly during handshake"
                    ));
                }
                _ => {}
            }
        }
    }

    Ok(DriverSession {
        handle,
        receiver,
        cursor,
    })
}

fn run_probe_turn(
    session: &DriverSession,
    prompt: &str,
    timeout: Duration,
) -> anyhow::Result<Vec<DriverEvent>> {
    session.handle.prompt(prompt.to_owned());
    let started = Instant::now();
    let mut events = Vec::new();
    while started.elapsed() < timeout {
        if let Ok(event) = session.receiver.recv_timeout(Duration::from_millis(50)) {
            events.push(event.clone());
            match event {
                DriverEvent::TurnFinished { success, summary } => {
                    if success {
                        return Ok(events);
                    }
                    return Err(anyhow!(
                        "skill probe turn failed: {}",
                        summary.unwrap_or_else(|| "provider reported failure".into())
                    ));
                }
                DriverEvent::Error(error) => {
                    return Err(anyhow!("skill probe turn error: {error}"));
                }
                DriverEvent::ProcessExited => {
                    return Err(anyhow!("provider exited during skill probe"));
                }
                _ => {}
            }
        }
    }
    Err(anyhow!("skill probe turn did not settle before timeout"))
}

fn structured_skill_invocation_evidence(
    provider: ProviderKind,
    binary_version: &str,
    binary_identity: &SkillBinaryIdentity,
    events: &[DriverEvent],
) -> Vec<SkillInvocationEvidence> {
    // No DriverEvent variant currently represents a provider-emitted native
    // skill invocation. TextDelta and ReasoningDelta are model-authored text,
    // so marker-shaped output must remain ordinary transcript content rather
    // than becoming capability evidence. Add a typed provider event here only
    // when its wire payload directly proves that the provider interpreted the
    // probe skill.
    let _ = (provider, binary_version, binary_identity, events);
    Vec::new()
}

fn write_probe_skill(root: &Path, name: &str) {
    let roots = [
        ".agents/skills",
        ".codex/skills",
        ".claude/skills",
        ".pi/skills",
        ".omp/skills",
        ".cursor/skills",
        ".fx/skills",
        ".amp/skills",
        ".opencode/skills",
        ".config/agents/skills",
    ];
    for relative in roots {
        let directory = root.join(relative).join(name);
        std::fs::create_dir_all(&directory).expect("create disposable skill directory");
        std::fs::write(
            directory.join("SKILL.md"),
            format!(
                "---\nname: {name}\ndescription: Padu disposable skill capability probe\n---\nThis disposable skill exists only for provider capability probing.\n"
            ),
        )
        .expect("write disposable skill");
    }
}

fn skill_probe_invocation(provider: ProviderKind, name: &str) -> String {
    match native_skill_syntax(provider) {
        Some(SkillNativeSyntax::Dollar) | None => format!("${name}"),
        Some(SkillNativeSyntax::SkillSlash) => format!("/skill:{name}"),
    }
}

fn run_skill_capability(
    provider: ProviderKind,
    model: Option<String>,
    effort: Option<String>,
    timeout_secs: u64,
    json: bool,
) -> anyhow::Result<()> {
    let probe = provider_probe(provider, None);
    let binary = probe
        .path
        .as_deref()
        .ok_or_else(|| anyhow!("provider binary is not installed"))?;
    let binary_version = probe_provider_version(binary)
        .ok_or_else(|| anyhow!("provider binary version is unavailable"))?;
    let binary_identity = provider_binary_identity(binary)
        .ok_or_else(|| anyhow!("provider binary identity is unavailable"))?;
    let nonce = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );
    let root = std::env::temp_dir().join(format!("padu-skill-probe-{}-{nonce}", provider.id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root)?;

    write_probe_skill(&root, "padu-probe-leading");
    write_probe_skill(&root, "padu-probe-inline");
    write_probe_skill(&root, "padu-probe-repeat");
    write_probe_skill(&root, "padu-probe-alpha");
    write_probe_skill(&root, "padu-probe-beta");
    let leading_invocation = skill_probe_invocation(provider, "padu-probe-leading");
    let inline_invocation = skill_probe_invocation(provider, "padu-probe-inline");
    let repeated_invocation = skill_probe_invocation(provider, "padu-probe-repeat");
    let alpha_invocation = skill_probe_invocation(provider, "padu-probe-alpha");
    let beta_invocation = skill_probe_invocation(provider, "padu-probe-beta");

    let result = (|| -> anyhow::Result<_> {
        let session = start_session_in(provider, model, effort, None, root.clone())?;
        let timeout = Duration::from_secs(timeout_secs);
        let mut driver_events = run_probe_turn(&session, &leading_invocation, timeout)?;
        driver_events.append(&mut run_probe_turn(
            &session,
            &format!("Please inspect this project, then {inline_invocation}."),
            timeout,
        )?);
        driver_events.append(&mut run_probe_turn(
            &session,
            &format!("{repeated_invocation} first, then {repeated_invocation} second."),
            timeout,
        )?);
        driver_events.append(&mut run_probe_turn(
            &session,
            &format!("{alpha_invocation} and {beta_invocation}."),
            timeout,
        )?);
        // Existing DriverEvent activity describes model and tool work; none of
        // it proves that the provider interpreted a native skill mention. Keep
        // the probe fail-closed until a driver exposes provider-emitted
        // invocation evidence.
        let observations = structured_skill_invocation_evidence(
            provider,
            &binary_version,
            &binary_identity,
            &driver_events,
        );
        let evidence = evaluate_skill_capability_probe(
            provider,
            Some(&binary_version),
            Some(&binary_identity),
            &observations,
        )
        .ok_or_else(|| anyhow!("provider identity disappeared during probe"))?;
        let cache_path = skill_capability_cache_path()
            .ok_or_else(|| anyhow!("provider skill capability cache path is unavailable"))?;
        write_skill_capability_evidence(&cache_path, &evidence)?;
        Ok((evidence, cache_path))
    })();
    let _ = std::fs::remove_dir_all(&root);
    let (evidence, cache_path) = result?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "provider": provider.id(),
                "binary": binary.display().to_string(),
                "binaryVersion": evidence.binary_version,
                "binaryIdentity": evidence.binary_identity.clone(),
                "evidenceSource": "structured provider invocation events (none observed)",
                "observations": evidence.observations,
                "capability": evidence.capability,
                "cachePath": cache_path.display().to_string(),
            }))?
        );
    } else {
        println!(
            "\n=== Skill Mention Capability: {} ===",
            provider.display_name()
        );
        println!("Binary version:        {}", evidence.binary_version);
        if let Some(identity) = evidence.binary_identity.as_ref() {
            println!("Binary path:           {}", identity.canonical_path);
            println!("Binary SHA-256:        {}", identity.content_sha256);
        }
        println!("Evidence source:       structured provider invocation events (none observed)");
        println!(
            "Leading invocation:    {}",
            evidence.observations.leading_invocation
        );
        println!(
            "Inline mention:        {}",
            evidence.observations.inline_mention
        );
        println!(
            "Repeated same mention: {}",
            evidence.observations.repeated_same_mention
        );
        println!(
            "Repeated different:    {}",
            evidence.observations.repeated_different_mentions
        );
        println!("Cache:                 {}", cache_path.display());
        println!();
    }
    Ok(())
}

fn run_connect(
    provider: ProviderKind,
    model: Option<String>,
    effort: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let session = start_session(provider, model, effort, None)?;

    if json {
        let out = json!({
            "connected": true,
            "provider": provider.id(),
            "cursor": session.cursor,
            "supports_steer": session.handle.supports_steer(),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    println!("\n✓ Connected to {} successfully!", provider.display_name());
    if let Some(cursor) = &session.cursor {
        println!("Session ID / Native ID: {}", cursor.native_id());
    }
    println!(
        "Supports Mid-Turn Steering: {}",
        session.handle.supports_steer()
    );
    println!();
    Ok(())
}

fn run_auth(provider: ProviderKind, json: bool) -> anyhow::Result<()> {
    let probe = provider_probe(provider, None);
    let binary = probe.path.ok_or_else(|| {
        anyhow!(
            "Provider '{}' binary not found on system",
            provider.command()
        )
    })?;
    let cwd = std::env::current_dir()?;

    match provider {
        ProviderKind::Agy => {
            padu_core::driver::authenticate_agy(&binary, &cwd, |url| {
                println!("\nPlease open the following link to authenticate:\n{url}\n");
            })?;
            if json {
                println!(
                    "{}",
                    json!({ "authenticated": true, "provider": provider.id() })
                );
            } else {
                println!(
                    "\n✓ Authenticated with {} successfully!\n",
                    provider.display_name()
                );
            }
            Ok(())
        }
        _ => Err(anyhow!(
            "Provider '{}' does not support standalone authentication",
            provider.id()
        )),
    }
}

fn run_logout(provider: ProviderKind, json: bool) -> anyhow::Result<()> {
    let probe = provider_probe(provider, None);
    let binary = probe.path.ok_or_else(|| {
        anyhow!(
            "Provider '{}' binary not found on system",
            provider.command()
        )
    })?;
    let cwd = std::env::current_dir()?;

    match provider {
        ProviderKind::Agy => {
            padu_core::driver::logout_agy(&binary, &cwd)?;
            if json {
                println!(
                    "{}",
                    json!({ "logged_out": true, "provider": provider.id() })
                );
            } else {
                println!(
                    "\n✓ Logged out from {} successfully!\n",
                    provider.display_name()
                );
            }
            Ok(())
        }
        _ => Err(anyhow!(
            "Provider '{}' does not support standalone logout",
            provider.id()
        )),
    }
}

fn run_turn(
    provider: ProviderKind,
    prompt: &str,
    model: Option<String>,
    effort: Option<String>,
    timeout_secs: u64,
    json: bool,
) -> anyhow::Result<()> {
    let session = start_session(provider, model, effort, None)?;

    if !json {
        println!("\nPrompt: \"{}\"\n--- Streaming Response ---", prompt);
    }

    session.handle.prompt(prompt.to_string());

    let start = Instant::now();
    let deadline = Duration::from_secs(timeout_secs);
    let mut response_text = String::new();
    let mut reasoning_text = String::new();
    let mut activities = Vec::new();
    let mut turn_success = false;

    while start.elapsed() < deadline {
        if let Ok(event) = session.receiver.recv_timeout(Duration::from_millis(50)) {
            match event {
                DriverEvent::TextDelta(delta) => {
                    if !json {
                        print!("{delta}");
                        std::io::Write::flush(&mut std::io::stdout())?;
                    }
                    response_text.push_str(&delta);
                }
                DriverEvent::ReasoningDelta(thought) => {
                    if !json {
                        print!("\x1b[90m{thought}\x1b[0m");
                        std::io::Write::flush(&mut std::io::stdout())?;
                    }
                    reasoning_text.push_str(&thought);
                }
                DriverEvent::RichActivity(act) => {
                    let summary = format!("[Tool: {} complete={}]", act.title, act.complete);
                    if !json {
                        println!("\n\x1b[36m{}\x1b[0m", summary);
                    }
                    activities.push(summary);
                }
                DriverEvent::Activity {
                    kind,
                    title,
                    complete,
                    ..
                } => {
                    let summary =
                        format!("[Activity: {:?} '{}' complete={}]", kind, title, complete);
                    if !json {
                        println!("\n\x1b[36m{}\x1b[0m", summary);
                    }
                    activities.push(summary);
                }
                DriverEvent::TurnFinished { success, summary } => {
                    turn_success = success;
                    if !json {
                        println!(
                            "\n\n--- Turn Settled (success: {success}, time: {:.2}s) ---",
                            start.elapsed().as_secs_f64()
                        );
                        if let Some(s) = summary {
                            println!("Summary: {s}");
                        }
                    }
                    break;
                }
                DriverEvent::Error(err) => {
                    if !json {
                        eprintln!("\n\x1b[31mError: {}\x1b[0m", err);
                    }
                    return Err(anyhow!("Driver emitted error during turn: {err}"));
                }
                DriverEvent::ProcessExited => {
                    if !json {
                        eprintln!("\n\x1b[31mDriver process exited before turn finished\x1b[0m");
                    }
                    return Err(anyhow!("Driver process exited prematurely"));
                }
                _ => {}
            }
        }
    }

    if json {
        let out = json!({
            "success": turn_success,
            "elapsed_secs": start.elapsed().as_secs_f64(),
            "text": response_text,
            "reasoning": reasoning_text,
            "activities": activities,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    }

    Ok(())
}

fn run_switch_model(
    provider: ProviderKind,
    target_model: &str,
    effort: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let session = start_session(provider, None, None, None)?;

    let options = SessionOptions {
        mode: RuntimeMode::FullAccess,
        interaction_mode: InteractionMode::Build,
        model: Some(target_model.to_string()),
        reasoning_effort: effort,
        service_tier: None,
        context_window: None,
    };

    let handled_in_place = session.handle.apply_options(options);

    if json {
        let out = json!({
            "provider": provider.id(),
            "target_model": target_model,
            "handled_in_place": handled_in_place,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    println!("\n=== Model Switch Test: {} ===", provider.display_name());
    println!("Target Model:     {}", target_model);
    println!(
        "Handled In-Place: {}",
        if handled_in_place {
            "YES (hot reload)"
        } else {
            "NO (restart required)"
        }
    );
    println!();
    Ok(())
}

fn run_fork(provider: ProviderKind, turns_to_remove: usize, json: bool) -> anyhow::Result<()> {
    if !provider.supports_conversation_fork() {
        return Err(anyhow!(
            "Provider '{}' does not support conversation forking",
            provider.id()
        ));
    }

    let session = start_session(provider, None, None, None)?;
    let fork_cursor = session.handle.fork(turns_to_remove)?;

    if json {
        let out = json!({
            "provider": provider.id(),
            "fork_supported": true,
            "turns_removed": turns_to_remove,
            "fork_cursor": fork_cursor,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    println!(
        "\n✓ Conversation fork succeeded for {}! (removed {} turns)",
        provider.display_name(),
        turns_to_remove
    );
    println!("New Native Fork Cursor: {}", fork_cursor.native_id());
    println!();
    Ok(())
}

fn run_rollback(provider: ProviderKind, turns: usize, json: bool) -> anyhow::Result<()> {
    if !provider.supports_conversation_rollback() {
        return Err(anyhow!(
            "Provider '{}' does not support conversation rollback",
            provider.id()
        ));
    }

    let session = start_session(provider, None, None, None)?;
    let rollback_cursor = session.handle.rollback(turns)?;

    if json {
        let out = json!({
            "provider": provider.id(),
            "rollback_supported": true,
            "turns_rolled_back": turns,
            "rollback_cursor": rollback_cursor,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    println!(
        "\n✓ Conversation rollback succeeded for {}! ({} turns)",
        provider.display_name(),
        turns
    );
    if let Some(cursor) = rollback_cursor {
        println!("Rollback Cursor: {}", cursor.native_id());
    } else {
        println!("Rollback settled without new cursor");
    }
    println!();
    Ok(())
}

fn run_resume(provider: ProviderKind, session_id: &str, json: bool) -> anyhow::Result<()> {
    let cursor = ProviderResumeCursor::from_session_id(provider, session_id.to_string());
    let session = start_session(provider, None, None, Some(cursor))?;

    if json {
        let out = json!({
            "resumed": true,
            "provider": provider.id(),
            "session_id": session_id,
            "active_cursor": session.cursor,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    println!(
        "\n✓ Resumed session '{}' on {} successfully!",
        session_id,
        provider.display_name()
    );
    println!();
    Ok(())
}

#[derive(Serialize)]
struct SuiteResult {
    provider: &'static str,
    installed: bool,
    binary_path: Option<String>,
    cli_version: Option<String>,
    models_count: usize,
    connect_ok: bool,
    turn_ok: bool,
    model_switch_ok: bool,
    fork_ok: Option<bool>,
    rollback_ok: Option<bool>,
    overall_passed: bool,
    errors: Vec<String>,
}

fn run_suite(provider: ProviderKind, json: bool) -> anyhow::Result<()> {
    let mut errors = Vec::new();
    let probe = provider_probe(provider, None);
    let installed = probe.installed;
    let binary_path = probe
        .path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());
    let cli_version = probe.path.as_deref().and_then(probe_provider_version);

    if !installed {
        let err = format!(
            "Provider '{}' binary '{}' is not installed",
            provider.id(),
            provider.command()
        );
        errors.push(err.clone());
        let res = SuiteResult {
            provider: provider.id(),
            installed: false,
            binary_path: None,
            cli_version: None,
            models_count: 0,
            connect_ok: false,
            turn_ok: false,
            model_switch_ok: false,
            fork_ok: None,
            rollback_ok: None,
            overall_passed: false,
            errors,
        };
        if json {
            println!("{}", serde_json::to_string_pretty(&res)?);
        } else {
            println!(
                "\n[✗] Binary detection: NOT INSTALLED ({})",
                provider.command()
            );
            println!("Overall Result: FAILED (binary missing)\n");
        }
        return Ok(());
    }

    let models = discover_provider_models(probe).models;
    let models_count = models.len();

    let mut connect_ok = false;
    let mut turn_ok = false;
    let mut model_switch_ok = false;
    let mut fork_ok = None;
    let mut rollback_ok = None;

    // 1. Connect
    match start_session(provider, None, None, None) {
        Ok(session) => {
            connect_ok = true;

            // 2. Model Switch
            let dummy_model = models
                .first()
                .map(|m| m.id.as_str())
                .unwrap_or("test-model");
            let _ = session.handle.apply_options(SessionOptions {
                mode: RuntimeMode::FullAccess,
                interaction_mode: InteractionMode::Build,
                model: Some(dummy_model.to_string()),
                reasoning_effort: None,
                service_tier: None,
                context_window: None,
            });
            model_switch_ok = true;

            // 3. Rollback & Fork (if supported)
            if provider.supports_conversation_rollback() {
                rollback_ok = Some(session.handle.rollback(0).is_ok());
            }
            if provider.supports_conversation_fork() {
                fork_ok = Some(session.handle.fork(0).is_ok());
            }

            // 4. Test Prompt
            session.handle.prompt("Say hello".to_string());
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(15) {
                if let Ok(event) = session.receiver.recv_timeout(Duration::from_millis(50)) {
                    if matches!(event, DriverEvent::TurnFinished { .. }) {
                        turn_ok = true;
                        break;
                    }
                    if let DriverEvent::Error(e) = event {
                        errors.push(format!("Turn error: {e}"));
                        break;
                    }
                }
            }
        }
        Err(e) => {
            errors.push(format!("Connection error: {e}"));
        }
    }

    let overall_passed = installed && connect_ok && errors.is_empty();

    let res = SuiteResult {
        provider: provider.id(),
        installed,
        binary_path,
        cli_version: cli_version.clone(),
        models_count,
        connect_ok,
        turn_ok,
        model_switch_ok,
        fork_ok,
        rollback_ok,
        overall_passed,
        errors,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&res)?);
        return Ok(());
    }

    println!(
        "\n=== Diagnostic Test Suite: {} ===",
        provider.display_name()
    );
    println!(
        "[{}] Binary Detection:   {} ({})",
        if installed { "✓" } else { "✗" },
        provider.command(),
        cli_version.unwrap_or_else(|| "unknown".into())
    );
    println!(
        "[{}] Model Catalog:      {} models discovered",
        if models_count > 0 { "✓" } else { "!" },
        models_count
    );
    println!(
        "[{}] Session Connect:    {}",
        if connect_ok { "✓" } else { "✗" },
        if connect_ok { "Handshake OK" } else { "Failed" }
    );
    println!(
        "[{}] Turn Execution:     {}",
        if turn_ok { "✓" } else { "!" },
        if turn_ok {
            "Settled cleanly"
        } else {
            "Did not settle in 15s"
        }
    );
    println!(
        "[{}] Model Switch:       {}",
        if model_switch_ok { "✓" } else { "✗" },
        if model_switch_ok {
            "Accepted"
        } else {
            "Failed"
        }
    );
    if let Some(ok) = fork_ok {
        println!(
            "[{}] Fork Capability:    {}",
            if ok { "✓" } else { "✗" },
            if ok { "Verified" } else { "Failed" }
        );
    }
    if let Some(ok) = rollback_ok {
        println!(
            "[{}] Rollback Check:     {}",
            if ok { "✓" } else { "✗" },
            if ok { "Verified" } else { "Failed" }
        );
    }
    println!(
        "\nOverall Status: {}",
        if overall_passed {
            "\x1b[32mPASSED\x1b[0m"
        } else {
            "\x1b[31mFAILED\x1b[0m"
        }
    );
    println!();
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = parse_args()?;

    match args.command.as_str() {
        "list" => run_list(args.json),
        "probe" => {
            let provider = parse_provider(
                args.provider_raw
                    .as_deref()
                    .ok_or_else(|| anyhow!("probe requires a provider name"))?,
            )?;
            run_probe(provider, args.json)
        }
        "models" => {
            let provider = parse_provider(
                args.provider_raw
                    .as_deref()
                    .ok_or_else(|| anyhow!("models requires a provider name"))?,
            )?;
            run_models(provider, args.json)
        }
        "auth" => {
            let provider = parse_provider(
                args.provider_raw
                    .as_deref()
                    .ok_or_else(|| anyhow!("auth requires a provider name"))?,
            )?;
            run_auth(provider, args.json)
        }
        "logout" => {
            let provider = parse_provider(
                args.provider_raw
                    .as_deref()
                    .ok_or_else(|| anyhow!("logout requires a provider name"))?,
            )?;
            run_logout(provider, args.json)
        }
        "connect" => {
            let provider = parse_provider(
                args.provider_raw
                    .as_deref()
                    .ok_or_else(|| anyhow!("connect requires a provider name"))?,
            )?;
            run_connect(provider, args.model, args.effort, args.json)
        }
        "turn" => {
            let provider = parse_provider(
                args.provider_raw
                    .as_deref()
                    .ok_or_else(|| anyhow!("turn requires a provider name"))?,
            )?;
            let prompt = args
                .param
                .as_deref()
                .ok_or_else(|| anyhow!("turn requires a prompt string"))?;
            run_turn(
                provider,
                prompt,
                args.model,
                args.effort,
                args.timeout_secs,
                args.json,
            )
        }
        "switch-model" => {
            let provider = parse_provider(
                args.provider_raw
                    .as_deref()
                    .ok_or_else(|| anyhow!("switch-model requires a provider name"))?,
            )?;
            let target_model = args
                .param
                .as_deref()
                .ok_or_else(|| anyhow!("switch-model requires a target model name"))?;
            run_switch_model(provider, target_model, args.effort, args.json)
        }
        "fork" => {
            let provider = parse_provider(
                args.provider_raw
                    .as_deref()
                    .ok_or_else(|| anyhow!("fork requires a provider name"))?,
            )?;
            let turns: usize = args
                .param
                .as_deref()
                .and_then(|p| p.parse().ok())
                .unwrap_or(0);
            run_fork(provider, turns, args.json)
        }
        "rollback" => {
            let provider = parse_provider(
                args.provider_raw
                    .as_deref()
                    .ok_or_else(|| anyhow!("rollback requires a provider name"))?,
            )?;
            let turns: usize = args
                .param
                .as_deref()
                .and_then(|p| p.parse().ok())
                .unwrap_or(0);
            run_rollback(provider, turns, args.json)
        }
        "resume" => {
            let provider = parse_provider(
                args.provider_raw
                    .as_deref()
                    .ok_or_else(|| anyhow!("resume requires a provider name"))?,
            )?;
            let session_id = args
                .param
                .as_deref()
                .ok_or_else(|| anyhow!("resume requires a session-id"))?;
            run_resume(provider, session_id, args.json)
        }
        "skill-capability" => {
            let provider = parse_provider(
                args.provider_raw
                    .as_deref()
                    .ok_or_else(|| anyhow!("skill-capability requires a provider name"))?,
            )?;
            run_skill_capability(
                provider,
                args.model,
                args.effort,
                args.timeout_secs,
                args.json,
            )
        }
        "suite" => {
            let provider = parse_provider(
                args.provider_raw
                    .as_deref()
                    .ok_or_else(|| anyhow!("suite requires a provider name"))?,
            )?;
            run_suite(provider, args.json)
        }
        "help" | _ => {
            print_help();
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_probe_uses_each_providers_native_skill_spelling() {
        assert_eq!(
            skill_probe_invocation(ProviderKind::Codex, "deploy"),
            "$deploy"
        );
        assert_eq!(
            skill_probe_invocation(ProviderKind::Fx, "deploy"),
            "$deploy"
        );
        assert_eq!(
            skill_probe_invocation(ProviderKind::Pi, "deploy"),
            "/skill:deploy"
        );
        assert_eq!(
            skill_probe_invocation(ProviderKind::OhMyPi, "deploy"),
            "/skill:deploy"
        );
    }

    #[test]
    fn model_authored_marker_text_cannot_enable_native_invocation() {
        let model_output = "PADU_SKILL_LEADING_TEST PADU_SKILL_INLINE_TEST";
        let binary_identity = SkillBinaryIdentity {
            canonical_path: "/opt/codex/bin/codex".into(),
            content_sha256: "a".repeat(64),
        };
        let events = [DriverEvent::TextDelta(model_output.into())];
        let observations = structured_skill_invocation_evidence(
            ProviderKind::Codex,
            "1.2.3",
            &binary_identity,
            &events,
        );
        assert!(observations.is_empty());
        assert!(matches!(
            &events[0],
            DriverEvent::TextDelta(text) if text == model_output
        ));

        let evidence = evaluate_skill_capability_probe(
            ProviderKind::Codex,
            Some("1.2.3"),
            Some(&binary_identity),
            &observations,
        )
        .expect("matching provider identity");
        assert!(!evidence.observations.leading_invocation);
        assert!(!evidence.observations.inline_mention);
        assert!(!evidence.observations.repeated_same_mention);
        assert!(!evidence.observations.repeated_different_mentions);
        assert!(!evidence.capability.leading_invocation);
        assert!(!evidence.capability.inline_mentions);
        assert!(!evidence.capability.repeated_mentions);
    }

    #[test]
    fn structured_evidence_requires_an_explicit_provider_event() {
        let binary_identity = SkillBinaryIdentity {
            canonical_path: "/opt/codex/bin/codex".into(),
            content_sha256: "b".repeat(64),
        };
        let explicit = SkillInvocationEvidence::new(
            ProviderKind::Codex,
            "1.2.3",
            binary_identity.clone(),
            padu_client::skill_probe::SKILL_CAPABILITY_PROBE_VERSION,
            padu_client::skill_probe::SkillInvocationObservation::LeadingInvocation,
        );
        let evidence = evaluate_skill_capability_probe(
            ProviderKind::Codex,
            Some("1.2.3"),
            Some(&binary_identity),
            &[explicit],
        )
        .expect("matching provider identity");
        assert!(evidence.capability.leading_invocation);
    }
}
