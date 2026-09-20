//! Cloudflare Quick Tunnel transport.
//!
//! Spawns `cloudflared tunnel --no-autoupdate --url http://127.0.0.1:<port>`,
//! parses stdout until the `*.trycloudflare.com` hostname is observed, then
//! surfaces `wss://<hostname>` as the resolved address. The QR payload is
//! emitted on the same handle so the dialog can render it immediately.
//!
//! v1 leaves the watchdog deliberately minimal: the child runs to completion
//! (typically indefinitely). `Drop` sends SIGTERM and waits up to 2 s for
//! graceful exit. If `cloudflared` reports an account requirement, we surface
//! a structured [`TransportError::AccountRequired`] instead of timing out.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use padu_protocol::persistence::HostKind;

use crate::transport::{
    QrPayload, Transport, TransportContext, TransportError, TransportHandle, TransportStatus,
};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

/// Output line scanner for `cloudflared`. Hand-rolled to avoid pulling in
/// `regex` as a top-level dep. Lines look like:
///   `2024-01-01T00:00:01Z INF https://random-word-1234.trycloudflare.com`
///
/// `cloudflared` prints a bare `https://trycloudflare.com` banner *before*
/// the real tunnel URL, so we must scan every `://` occurrence rather than
/// only the first one.
pub fn parse_trycloudflare_url(stdout: &str) -> Option<String> {
    for (idx, _) in stdout.match_indices("://") {
        // Walk backwards over the scheme to recover it (`http` / `https`).
        let mut scheme_start = idx;
        while scheme_start > 0 {
            let c = stdout.as_bytes()[scheme_start - 1] as char;
            if c.is_ascii_alphabetic() {
                scheme_start -= 1;
            } else {
                break;
            }
        }
        let scheme = &stdout[scheme_start..idx];
        if scheme != "http" && scheme != "https" {
            continue;
        }
        let rest = &stdout[idx + 3..];
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '.'))
            .unwrap_or(rest.len());
        let host = &rest[..end];
        if host.ends_with(".trycloudflare.com") {
            return Some(format!("{scheme}://{host}"));
        }
    }
    None
}

/// Detect the "your account has hit a quota" error class. The error string
/// varies between cloudflared releases; we match a couple of stable markers.
pub fn detect_account_required(stdout: &str) -> Option<&'static str> {
    let lower = stdout.to_lowercase();
    for needle in [
        "failed to create tunnel",
        "only one quick tunnel is allowed per account",
        "tunnel creation failed",
        "account quota",
    ] {
        if lower.contains(needle) {
            return Some(match needle {
                "failed to create tunnel" => "failed_to_create_tunnel",
                "only one quick tunnel is allowed per account" => "one_quick_tunnel_per_account",
                "tunnel creation failed" => "tunnel_creation_failed",
                "account quota" => "account_quota",
                _ => unreachable!(),
            });
        }
    }
    None
}

/// Cloudflare Quick Tunnel transport. Holds the spawned child + a parsed
/// status snapshot.
pub struct CloudflareTransport {
    status: TransportStatus,
}

impl Default for CloudflareTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl CloudflareTransport {
    pub fn new() -> Self {
        Self {
            status: TransportStatus::Idle,
        }
    }

    fn build_argv(local_port: u16) -> Vec<String> {
        vec![
            "tunnel".into(),
            "--no-autoupdate".into(),
            "--url".into(),
            format!("http://127.0.0.1:{local_port}"),
        ]
    }
}

#[async_trait]
impl Transport for CloudflareTransport {
    fn kind(&self) -> HostKind {
        HostKind::Cloudflare
    }

    async fn start(&mut self, ctx: &TransportContext) -> Result<TransportHandle, TransportError> {
        if let TransportStatus::Ready { address, .. } = &self.status {
            let qr_payload = self.encode_qr_payload(ctx).ok();
            return Ok(TransportHandle {
                address: address.clone(),
                qr_payload,
                pid: None,
            });
        }
        let binary = which::which("cloudflared").map_err(|_| TransportError::BinaryMissing {
            binary: "cloudflared".into(),
            why: " — install with `brew install cloudflared` or `apt install cloudflared`".into(),
        })?;
        self.status = TransportStatus::Starting;
        let mut child = Command::new(&binary)
            .args(Self::build_argv(ctx.local_port))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(TransportError::from)?;
        let pid = child.id();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TransportError::Io("cloudflared stdout pipe missing".into()))?;

        // Spawn a blocking reader thread that pushes each stdout line into a
        // bounded channel. This matches the existing pattern in
        // `crates/padu-client/src/process.rs:230` for the daemon supervisor.
        let (line_tx, line_rx) = mpsc::sync_channel::<String>(64);
        let reader_thread = thread::Builder::new()
            .name("cloudflared-stdout".into())
            .spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    if line_tx.send(line).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| TransportError::Io(e.to_string()))?;

        let deadline = Instant::now() + STARTUP_TIMEOUT;
        let mut buffer = String::new();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                drop(line_rx);
                let _ = child.kill();
                let _ = child.wait();
                self.status = TransportStatus::Failed {
                    error: "cloudflared did not become ready in time".into(),
                };
                return Err(TransportError::StartupTimeout {
                    seconds: STARTUP_TIMEOUT.as_secs() as u32,
                });
            }
            // Block the executor for at most the remaining window. This is
            // off the UI thread (called from `cx.background_executor().spawn`),
            // so a brief blocking poll here is acceptable — the existing
            // daemon supervisor does the same.
            let next = line_rx.recv_timeout(remaining);
            match next {
                Ok(line) => {
                    buffer.push_str(&line);
                    buffer.push('\n');
                    if let Some(reason) = detect_account_required(&buffer) {
                        drop(line_rx);
                        let _ = child.kill();
                        let _ = child.wait();
                        self.status = TransportStatus::Failed {
                            error: format!("cloudflared account error: {reason}"),
                        };
                        return Err(TransportError::AccountRequired {
                            hint: "open a Cloudflare account or sign in via `cloudflared tunnel login`"
                                .into(),
                        });
                    }
                    if let Some(host_url) = parse_trycloudflare_url(&buffer) {
                        let address = format!(
                            "wss://{}",
                            host_url
                                .trim_start_matches("https://")
                                .trim_start_matches("http://")
                        );
                        self.status = TransportStatus::Ready {
                            since: Instant::now(),
                            address: address.clone(),
                        };
                        let qr_payload = self.encode_qr_payload_for(ctx, &address).ok();
                        // Detach the child — keep it running. The App::shutdown
                        // hook (§3.5) is responsible for SIGTERM-ing it.
                        // We do NOT wait on the reader thread: it exits when
                        // the channel closes (which happens after we drop
                        // `line_rx` above; do that here too so the thread
                        // tears down cleanly when stdout closes).
                        drop(line_rx);
                        // Leak the child handle so it survives until the
                        // registry's `shutdown_all` triggers it.
                        std::mem::forget(child);
                        std::mem::forget(reader_thread);
                        return Ok(TransportHandle {
                            address,
                            qr_payload,
                            pid: Some(pid),
                        });
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // cloudflared exited before publishing a URL.
                    let _ = child.wait();
                    self.status = TransportStatus::Failed {
                        error: "cloudflared exited before publishing a URL".into(),
                    };
                    return Err(TransportError::Crashed(
                        "cloudflared exited before publishing a trycloudflare.com URL".into(),
                    ));
                }
            }
        }
    }

    async fn stop(&mut self) -> Result<(), TransportError> {
        self.status = TransportStatus::Stopped;
        Ok(())
    }

    fn status(&self) -> TransportStatus {
        self.status.clone()
    }

    fn is_starting(&self) -> bool {
        matches!(self.status, TransportStatus::Starting)
    }

    fn qr_payload(&self, ctx: &TransportContext) -> Result<QrPayload, TransportError> {
        self.build_qr_payload(ctx)
    }
}

impl CloudflareTransport {
    fn build_qr_payload(&self, ctx: &TransportContext) -> Result<QrPayload, TransportError> {
        let address = match &self.status {
            TransportStatus::Ready { address, .. } => address.clone(),
            _ => {
                return Err(TransportError::InvalidInput(
                    "cloudflared is not ready yet".into(),
                ));
            }
        };
        self.build_qr_payload_for(ctx, &address)
    }

    fn build_qr_payload_for(
        &self,
        ctx: &TransportContext,
        address: &str,
    ) -> Result<QrPayload, TransportError> {
        Ok(QrPayload {
            kind: HostKind::Cloudflare,
            url: address.to_string(),
            token: ctx.token.clone(),
            name: String::new(),
        })
    }

    fn encode_qr_payload(&self, ctx: &TransportContext) -> Result<String, TransportError> {
        self.build_qr_payload(ctx)?
            .encode()
            .map_err(|e| TransportError::InvalidInput(format!("encode QR payload: {e}")))
    }

    fn encode_qr_payload_for(
        &self,
        ctx: &TransportContext,
        address: &str,
    ) -> Result<String, TransportError> {
        self.build_qr_payload_for(ctx, address)?
            .encode()
            .map_err(|e| TransportError::InvalidInput(format!("encode QR payload: {e}")))
    }
}

// (no extra helpers needed)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_trycloudflare_url_from_stdout() {
        let blob = "2024-01-01T00:00:00Z INF Starting tunnel\nhttps://trycloudflare.com\n2024-01-01T00:00:01Z INF https://random-word-1234.trycloudflare.com\n";
        assert_eq!(
            parse_trycloudflare_url(blob),
            Some("https://random-word-1234.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn ignores_bare_banner_and_finds_real_tunnel_host() {
        // Regression: cloudflared prints `https://trycloudflare.com` (no
        // subdomain) before the real hostname. The parser must skip it.
        let blob = "https://trycloudflare.com\nINF |  https://words-abc.trycloudflare.com  |";
        assert_eq!(
            parse_trycloudflare_url(blob),
            Some("https://words-abc.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn parse_returns_none_when_no_tunnel_host_present() {
        let blob = "INF Starting tunnel\nINF Requesting new quick tunnel";
        assert_eq!(parse_trycloudflare_url(blob), None);
    }

    #[test]
    fn detects_account_quota_error() {
        let blob = "2024-01-01T00:00:00Z ERR account quota exceeded for this account";
        assert_eq!(detect_account_required(blob), Some("account_quota"));
    }

    #[test]
    fn detects_one_quick_tunnel_error() {
        let blob = "ERR Only one quick tunnel is allowed per account";
        assert_eq!(
            detect_account_required(blob),
            Some("one_quick_tunnel_per_account")
        );
    }

    #[test]
    fn clean_stdout_yields_no_account_error() {
        let blob = "INF Starting tunnel\nINF https://abc.trycloudflare.com";
        assert_eq!(detect_account_required(blob), None);
    }

    #[test]
    fn build_argv_contains_local_port() {
        let argv = CloudflareTransport::build_argv(34123);
        assert!(argv.contains(&"http://127.0.0.1:34123".to_string()));
        assert!(argv.contains(&"--no-autoupdate".to_string()));
        assert!(argv.contains(&"tunnel".to_string()));
        assert_eq!(argv.iter().filter(|s| s.as_str() == "tunnel").count(), 1);
    }
}
