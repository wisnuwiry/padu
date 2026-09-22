//! Cloudflare Quick Tunnel transport.
//!
//! Spawns `cloudflared tunnel --no-autoupdate --url http://127.0.0.1:<port>`,
//! parses stdout until the `*.trycloudflare.com` hostname is observed, then
//! surfaces `wss://<hostname>` as the resolved address. The QR payload is
//! emitted on the same handle so the dialog can render it immediately.
//!
//! Once the tunnel is up the child is handed to `watch_child`, so a later
//! crash flips the status to `Failed` rather than leaving the UI claiming a
//! live tunnel. Tearing the process down is the registry's shutdown hook's
//! job. If `cloudflared` reports an account requirement, we surface a
//! structured [`TransportError::AccountRequired`] instead of timing out.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use padu_protocol::persistence::HostKind;
use parking_lot::Mutex;

use crate::transport::{
    QrPayload, Transport, TransportContext, TransportError, TransportHandle, TransportStatus,
    terminate_process, watch_child,
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
    status: Arc<Mutex<TransportStatus>>,
    /// PID of the running `cloudflared`, if any. The `Child` itself is owned
    /// by the watcher thread, so `stop()` kills by pid and the watcher (which
    /// respects `Stopped`) simply observes the exit.
    pid: Arc<Mutex<Option<u32>>>,
}

impl Default for CloudflareTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl CloudflareTransport {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(TransportStatus::Idle)),
            pid: Arc::new(Mutex::new(None)),
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
        if let TransportStatus::Ready { address, .. } = self.status.lock().clone() {
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
        *self.status.lock() = TransportStatus::Starting;
        *self.pid.lock() = None;
        let mut child = Command::new(&binary)
            .args(Self::build_argv(ctx.local_port))
            .stdout(Stdio::piped())
            // `cloudflared` reports quota/auth errors on stderr far more often
            // than stdout — discarding it turned real failures into a 30s
            // timeout. Merge both streams into the startup scan.
            .stderr(Stdio::piped())
            .spawn()
            .map_err(TransportError::from)?;
        let pid = child.id();
        *self.pid.lock() = Some(pid);
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TransportError::Io("cloudflared stdout pipe missing".into()))?;
        let stderr = child.stderr.take();

        // Spawn a blocking reader thread that pushes each stdout line into a
        // bounded channel. This matches the existing pattern in
        // `crates/padu-client/src/process.rs:230` for the daemon supervisor.
        let (line_tx, line_rx) = mpsc::sync_channel::<String>(128);
        let reader_thread = thread::Builder::new()
            .name("cloudflared-stdout".into())
            .spawn({
                let line_tx = line_tx.clone();
                move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines().map_while(Result::ok) {
                        if line_tx.send(line).is_err() {
                            break;
                        }
                    }
                }
            })
            .map_err(|e| TransportError::Io(e.to_string()))?;
        // A second reader merges stderr into the same scan so account/quota
        // errors fail fast instead of timing out.
        let _stderr_thread = thread::Builder::new()
            .name("cloudflared-stderr".into())
            .spawn(move || {
                let Some(stderr) = stderr else { return };
                let reader = BufReader::new(stderr);
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
                *self.status.lock() = TransportStatus::Failed {
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
                        *self.status.lock() = TransportStatus::Failed {
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
                        *self.status.lock() = TransportStatus::Ready {
                            since: Instant::now(),
                            address: address.clone(),
                        };
                        let qr_payload = self.encode_qr_payload_for(ctx, &address).ok();
                        // Hand the child to a watcher so a later crash flips
                        // the status to `Failed` instead of leaving the UI
                        // claiming a live tunnel. A drain thread keeps
                        // consuming the merged output for the life of the
                        // process: dropping the receiver here would let the
                        // reader threads exit and close the pipes, and
                        // `cloudflared` would then die with SIGPIPE on its
                        // next log write. The drain exits on its own once
                        // `cloudflared` goes away (readers hit EOF and drop
                        // their senders), so dropping these handles only
                        // detaches them.
                        drop(reader_thread);
                        thread::Builder::new()
                            .name("cloudflared-drain".into())
                            .spawn(move || while line_rx.recv().is_ok() {})
                            .map_err(|e| TransportError::Io(e.to_string()))?;
                        watch_child(child, self.status.clone(), "cloudflared");
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
                    *self.status.lock() = TransportStatus::Failed {
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
        *self.status.lock() = TransportStatus::Stopped;
        if let Some(pid) = self.pid.lock().take() {
            terminate_process(pid);
        }
        Ok(())
    }

    fn status(&self) -> TransportStatus {
        self.status.lock().clone()
    }

    fn is_starting(&self) -> bool {
        matches!(*self.status.lock(), TransportStatus::Starting)
    }

    fn qr_payload(&self, ctx: &TransportContext) -> Result<QrPayload, TransportError> {
        self.build_qr_payload(ctx)
    }
}

impl CloudflareTransport {
    fn build_qr_payload(&self, ctx: &TransportContext) -> Result<QrPayload, TransportError> {
        let address = match self.status.lock().clone() {
            TransportStatus::Ready { address, .. } => address,
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
