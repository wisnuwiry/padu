//! Desktop ownership of the Padu daemon process.

use std::path::PathBuf;

use anyhow::{Context as _, anyhow, bail};

pub fn start_process() -> anyhow::Result<padu_client::DaemonSupervisor> {
    let address = std::env::var(padu_client::DAEMON_ADDRESS_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty());
    let token = std::env::var(padu_client::DAEMON_TOKEN_ENV)
        .ok()
        .filter(|value| !value.is_empty());
    match (address, token) {
        (Some(address), Some(token)) => {
            return padu_client::DaemonSupervisor::connect(address.trim(), token);
        }
        (Some(_), None) => bail!(
            "{} is set but {} is missing",
            padu_client::DAEMON_ADDRESS_ENV,
            padu_client::DAEMON_TOKEN_ENV
        ),
        (None, Some(_)) => bail!(
            "{} is set but {} is missing",
            padu_client::DAEMON_TOKEN_ENV,
            padu_client::DAEMON_ADDRESS_ENV
        ),
        (None, None) => {}
    }
    let app_settings = padu_client::persistence::load_or_create_app_settings()
        .context("could not load desktop daemon settings")?;
    let exposure = app_settings.daemon_exposure;
    match padu_client::DaemonSupervisor::spawn_configured(
        &daemon_executable_path()?,
        cfg!(debug_assertions),
        exposure.clone(),
    ) {
        Ok(supervisor) => Ok(supervisor),
        Err(error) if exposure.enabled => {
            // A previous desktop process can leave its exposed daemon alive
            // briefly while the app is being relaunched. Reuse it only when
            // the persisted token authenticates successfully; an unrelated
            // listener still produces the original startup error below.
            let address = format!("ws://127.0.0.1:{}", exposure.port);
            match padu_client::DaemonSupervisor::connect(&address, exposure.token.clone()) {
                Ok(supervisor) => Ok(supervisor),
                Err(_) if cfg!(debug_assertions) => {
                    // Development must remain launchable after a stale daemon
                    // with an incompatible token has claimed the configured
                    // port. Keep the preference intact, but use a private
                    // ephemeral listener for this process instead of exposing
                    // the app on an address it does not own.
                    let mut fallback = exposure;
                    fallback.enabled = false;
                    padu_client::DaemonSupervisor::spawn_configured(
                        &daemon_executable_path()?,
                        true,
                        fallback,
                    )
                    .map_err(|fallback_error| {
                        error.context(format!(
                            "could not start the development daemon on its fallback listener: {fallback_error:#}"
                        ))
                    })
                }
                Err(_) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

/// Resolve the local host name once during app construction. Settings can
/// then show a useful LAN URL without touching the OS from a render frame.
pub fn local_hostname() -> Option<String> {
    #[cfg(unix)]
    {
        let mut buffer = [0_u8; 256];
        let result = unsafe { libc::gethostname(buffer.as_mut_ptr().cast(), buffer.len()) };
        if result == 0 {
            let length = buffer
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(buffer.len());
            let hostname = String::from_utf8_lossy(&buffer[..length]).trim().to_owned();
            if !hostname.is_empty() {
                return Some(hostname);
            }
        }
    }
    // `COMPUTERNAME` is the Windows equivalent and is always set; `HOSTNAME`
    // covers the shells that export it.
    ["COMPUTERNAME", "HOSTNAME"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .map(|hostname| hostname.trim().to_owned())
        .find(|hostname| !hostname.is_empty())
}

/// Detect this machine's LAN IPv4 address once during app construction.
///
/// Uses a UDP socket bound to `0.0.0.0:0` and "connected" to a public IP.
/// No packet is sent — `connect` on a datagram socket only selects the
/// default route — so this is a cheap, synchronous, cross-platform way to
/// learn which local address another LAN device should use. Returns `None`
/// when offline or when only loopback is available. Callers must cache the
/// result; render frames must never do network I/O.
pub fn local_lan_address() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    // 8.8.8.8:80 is never contacted; it only forces route selection.
    socket.connect("8.8.8.8:80").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    match ip {
        std::net::IpAddr::V4(v4) => {
            if v4.is_loopback() || v4.is_unspecified() {
                return None;
            }
            Some(v4.to_string())
        }
        // Prefer IPv4 for Direct LAN codes; an IPv6-only route would need
        // bracket formatting and is still unreachable for most phones.
        std::net::IpAddr::V6(_) => None,
    }
}

fn daemon_executable_path() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os("PADU_DAEMON_PATH").filter(|path| !path.is_empty()) {
        return Ok(path.into());
    }
    let executable = format!("padu-daemon{}", std::env::consts::EXE_SUFFIX);
    let current = std::env::current_exe().context("could not locate the Padu executable")?;

    // Development keeps the daemon beside Cargo's debug artifacts rather than
    // inside Padu Debug.app. The supervisor watches this file and swaps only
    // the daemon when the development watcher relinks it.
    #[cfg(debug_assertions)]
    if let Some(debug_directory) = current
        .ancestors()
        .find(|candidate| candidate.file_name().is_some_and(|name| name == "debug"))
    {
        let external = debug_directory.join(&executable);
        if external.is_file() {
            return Ok(external);
        }
    }

    let sibling = current
        .parent()
        .map(|directory| directory.join(&executable))
        .ok_or_else(|| anyhow!("Padu executable has no parent directory"))?;
    if sibling.is_file() {
        return Ok(sibling);
    }
    #[cfg(debug_assertions)]
    bail!(
        "Padu daemon was not found in Cargo's debug directory or next to the app executable: {}",
        sibling.display(),
    );
    #[cfg(not(debug_assertions))]
    bail!(
        "Padu daemon is missing next to the app executable: {}",
        sibling.display(),
    )
}
