//! Reverse SSH relay transport.
//!
//! Spawns `ssh -N -o ExitOnForwardFailure=yes -o ServerAliveInterval=30
//! -o ServerAliveCountMax=3 -o StrictHostKeyChecking=accept-new
//! -R 0.0.0.0:<remote_port>:127.0.0.1:<local_port> [-i <id>]
//! <user>@<host>` and returns the remote-side `wss://<host>:<remote_port>`
//! address.
//!
//! Auth is delegated entirely to the user's existing SSH key/agent — Padu
//! never sees or stores passwords. If `ssh` exits non-zero, the stderr tail
//! is surfaced via [`TransportError::Crashed`].

use std::process::Stdio;
use std::sync::Arc;

use anyhow::Context as _;
use async_trait::async_trait;
use padu_protocol::persistence::HostKind;
use parking_lot::Mutex;
use smol::process::Command;

use crate::transport::{
    QrPayload, Transport, TransportContext, TransportError, TransportHandle, TransportStatus,
};

/// Knobs parsed out of the `HostProfile::ssh` block.
#[derive(Clone, Debug)]
pub struct SshConfig {
    pub user: String,
    pub host: String,
    pub remote_port: u16,
    pub identity_file: Option<String>,
}

impl SshConfig {
    pub fn validate(&self) -> Result<(), TransportError> {
        if self.user.trim().is_empty() {
            return Err(TransportError::InvalidInput(
                "ssh user must not be empty".into(),
            ));
        }
        if self.host.trim().is_empty() {
            return Err(TransportError::InvalidInput(
                "ssh host must not be empty".into(),
            ));
        }
        if self.remote_port == 0 {
            return Err(TransportError::InvalidInput(
                "ssh remote_port must be 1..=65535".into(),
            ));
        }
        Ok(())
    }

    /// Returns the argv passed to `ssh`. Extracted as a pure function so
    /// unit tests can assert on the exact shape.
    pub fn argv(&self, local_port: u16) -> Vec<String> {
        let mut argv: Vec<String> = vec![
            "-N".into(),
            "-o".into(),
            "ExitOnForwardFailure=yes".into(),
            "-o".into(),
            "ServerAliveInterval=30".into(),
            "-o".into(),
            "ServerAliveCountMax=3".into(),
            "-o".into(),
            "StrictHostKeyChecking=accept-new".into(),
            "-R".into(),
            format!("0.0.0.0:{}:127.0.0.1:{}", self.remote_port, local_port),
        ];
        if let Some(identity) = &self.identity_file {
            argv.push("-i".into());
            argv.push(identity.clone());
        }
        argv.push(format!("{}@{}", self.user, self.host));
        argv
    }

    /// The URL clients use to reach the daemon via the relay.
    pub fn address(&self) -> String {
        format!("wss://{}:{}", self.host, self.remote_port)
    }
}

/// State tracked by an active [`SshTransport`].
pub struct SshTransport {
    config: SshConfig,
    status: Arc<Mutex<TransportStatus>>,
}

impl SshTransport {
    pub fn new(config: SshConfig) -> Self {
        Self {
            config,
            status: Arc::new(Mutex::new(TransportStatus::Idle)),
        }
    }
}

#[async_trait]
impl Transport for SshTransport {
    fn kind(&self) -> HostKind {
        HostKind::SshRelay
    }

    async fn start(&mut self, ctx: &TransportContext) -> Result<TransportHandle, TransportError> {
        if let TransportStatus::Ready { address, .. } = self.status.lock().clone() {
            return Ok(TransportHandle {
                address: address.clone(),
                qr_payload: None,
                pid: None,
            });
        }
        self.config.validate()?;
        let binary = which::which("ssh").map_err(|_| TransportError::BinaryMissing {
            binary: "ssh".into(),
            why: "".into(),
        })?;
        *self.status.lock() = TransportStatus::Starting;
        let argv = self.config.argv(ctx.local_port);
        let mut child = Command::new(&binary)
            .args(&argv)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(TransportError::from)?;
        // Give `ssh` a moment to fail-fast on bad auth / unreachable host.
        // If it exits within this window with non-zero, surface stderr.
        smol::Timer::after(std::time::Duration::from_millis(750)).await;
        if let Ok(Some(status)) = child.try_status()
            && !status.success()
        {
            let stderr = read_stderr_tail(&mut child).await.unwrap_or_default();
            *self.status.lock() = TransportStatus::Failed {
                error: format!("ssh exited {}: {}", status, stderr.trim()),
            };
            return Err(TransportError::Crashed(format!(
                "ssh exited {}: {}",
                status,
                stderr.trim()
            )));
        }
        let address = self.config.address();
        *self.status.lock() = TransportStatus::Ready {
            since: std::time::Instant::now(),
            address: address.clone(),
        };
        let pid = child.id();
        // Watch the relay so a dropped connection flips the status to
        // `Failed` rather than leaving the UI claiming a live tunnel. The
        // task owns the child from here; `status()` reaps it.
        let watcher_status = self.status.clone();
        smol::spawn(async move {
            let outcome = child.status().await;
            let mut guard = watcher_status.lock();
            if !matches!(
                *guard,
                TransportStatus::Ready { .. } | TransportStatus::Starting
            ) {
                return;
            }
            *guard = TransportStatus::Failed {
                error: match outcome {
                    Ok(exit) => format!("ssh relay exited ({exit})"),
                    Err(error) => format!("ssh relay could not be waited on: {error}"),
                },
            };
        })
        .detach();
        Ok(TransportHandle {
            address,
            qr_payload: None,
            pid: Some(pid),
        })
    }

    async fn stop(&mut self) -> Result<(), TransportError> {
        *self.status.lock() = TransportStatus::Stopped;
        Ok(())
    }

    fn status(&self) -> TransportStatus {
        self.status.lock().clone()
    }

    fn is_starting(&self) -> bool {
        matches!(*self.status.lock(), TransportStatus::Starting)
    }

    fn qr_payload(&self, ctx: &TransportContext) -> Result<QrPayload, TransportError> {
        if !matches!(*self.status.lock(), TransportStatus::Ready { .. }) {
            return Err(TransportError::InvalidInput(
                "ssh tunnel is not ready yet".into(),
            ));
        }
        Ok(QrPayload {
            kind: HostKind::SshRelay,
            url: self.config.address(),
            token: ctx.token.clone(),
            name: String::new(),
        })
    }
}

/// Read whatever `ssh` wrote to stderr, keeping only the tail so the dialog
/// shows a hint rather than a full stack trace.
async fn read_stderr_tail(child: &mut smol::process::Child) -> anyhow::Result<String> {
    use smol::io::AsyncReadExt;
    let mut stderr = child.stderr.take().context("ssh stderr pipe missing")?;
    let mut buf = Vec::with_capacity(2048);
    stderr
        .read_to_end(&mut buf)
        .await
        .context("read ssh stderr")?;
    if buf.len() > 1024 {
        let cut = buf.len() - 1024;
        buf.drain(..cut);
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(remote_port: u16) -> SshConfig {
        SshConfig {
            user: "alice".into(),
            host: "jump.example.com".into(),
            remote_port,
            identity_file: None,
        }
    }

    #[test]
    fn ssh_argv_includes_reverse_flag_and_local_port() {
        let argv = cfg(19999).argv(34123);
        let reverse_arg = argv
            .iter()
            .find(|s| s.starts_with("-R"))
            .expect("-R missing");
        assert_eq!(reverse_arg, "-R");
        let idx = argv.iter().position(|s| s == "-R").unwrap();
        assert_eq!(argv[idx + 1], "0.0.0.0:19999:127.0.0.1:34123");
        assert!(argv.contains(&"alice@jump.example.com".to_string()));
        assert!(argv.contains(&"-N".to_string()));
        assert!(argv.contains(&"ExitOnForwardFailure=yes".to_string()));
    }

    #[test]
    fn ssh_argv_passes_identity_file_as_separate_arg() {
        let mut c = cfg(19999);
        c.identity_file = Some("/Users/x/.ssh/id_ed25519".into());
        let argv = c.argv(34123);
        let idx = argv.iter().position(|s| s == "-i").unwrap();
        assert_eq!(argv[idx + 1], "/Users/x/.ssh/id_ed25519");
        // Ensure the path is its own argv entry, not joined to another.
        assert!(!argv.iter().any(|a| a.contains(' ') || a.contains('\t')));
    }

    #[test]
    fn ssh_rejects_empty_user_or_host() {
        let mut c = cfg(19999);
        c.user = "".into();
        assert!(c.validate().is_err());
        c.user = "alice".into();
        c.host = "".into();
        assert!(c.validate().is_err());
    }

    #[test]
    fn ssh_rejects_remote_port_zero() {
        let c = SshConfig {
            user: "alice".into(),
            host: "jump.example.com".into(),
            remote_port: 0,
            identity_file: None,
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn ssh_accepts_remote_port_at_max() {
        // u16 max is 65535; the validator should accept it.
        let c = SshConfig {
            user: "alice".into(),
            host: "jump.example.com".into(),
            remote_port: u16::MAX,
            identity_file: None,
        };
        assert!(c.validate().is_ok());
        assert_eq!(c.address(), "wss://jump.example.com:65535");
    }

    #[test]
    fn ssh_address_uses_wss_scheme() {
        let c = cfg(19999);
        assert_eq!(c.address(), "wss://jump.example.com:19999");
    }
}
