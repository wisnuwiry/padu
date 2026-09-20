//! Tailscale transport.
//!
//! No subprocess is spawned — Tailscale is assumed to be already installed
//! and running on the user's machine. We resolve a MagicDNS name (or accept
//! a raw 100.x IP) and surface `wss://<host>:<port>` as the resolved
//! address.
//!
//! v1 deliberately does NOT call `tailscale serve` / `tailscale funnel` —
//! the daemon's own listener (already configured via DaemonExposureSettings)
//! is the source of truth. Tailscale is just the network path.

use async_trait::async_trait;
use padu_protocol::persistence::HostKind;

use crate::transport::{
    QrPayload, Transport, TransportContext, TransportError, TransportHandle, TransportStatus,
};

/// Knobs parsed out of the `HostProfile::tailscale` block.
#[derive(Clone, Debug)]
pub struct TailscaleConfig {
    pub magic_dns: String,
    pub port: u16,
}

impl TailscaleConfig {
    pub fn validate(&self) -> Result<(), TransportError> {
        if self.magic_dns.trim().is_empty() {
            return Err(TransportError::InvalidInput(
                "tailscale magic_dns must not be empty".into(),
            ));
        }
        if self.port == 0 {
            return Err(TransportError::InvalidInput(
                "tailscale port must be 1..=65535".into(),
            ));
        }
        Ok(())
    }

    /// Pure function: turn the user-supplied `magic_dns` into a URL the
    /// client can connect to. We accept either a hostname (MagicDNS) or a
    /// raw 100.x IP without doing any DNS lookup — Tailscale's CGNAT range
    /// is well-known to be `100.64.0.0/10`.
    pub fn address(&self) -> String {
        format!("wss://{}:{}", self.magic_dns, self.port)
    }

    /// Returns `true` if `magic_dns` looks like a raw Tailnet IP. Useful for
    /// deciding whether to attempt `tailscale status` lookup at all.
    pub fn looks_like_tailnet_ip(magic_dns: &str) -> bool {
        let parts: Vec<&str> = magic_dns.split('.').collect();
        if parts.len() != 4 {
            return false;
        }
        let first: Option<u16> = match parts.first() {
            Some(s) => s.parse().ok(),
            None => return false,
        };
        let second: Option<u16> = match parts.get(1) {
            Some(s) => s.parse().ok(),
            None => return false,
        };
        match (first, second) {
            (Some(100), Some(s)) => (64..=127).contains(&s),
            _ => false,
        }
    }
}

pub struct TailscaleTransport {
    config: TailscaleConfig,
    status: TransportStatus,
}

impl TailscaleTransport {
    pub fn new(config: TailscaleConfig) -> Self {
        Self {
            config,
            status: TransportStatus::Idle,
        }
    }
}

#[async_trait]
impl Transport for TailscaleTransport {
    fn kind(&self) -> HostKind {
        HostKind::Tailscale
    }

    async fn start(&mut self, _ctx: &TransportContext) -> Result<TransportHandle, TransportError> {
        if let TransportStatus::Ready { address, .. } = &self.status {
            return Ok(TransportHandle {
                address: address.clone(),
                qr_payload: None,
                pid: None,
            });
        }
        self.config.validate()?;
        // v1 does not spawn anything. We still warn the user if the daemon
        // is not exposed (caller checks `daemon_exposure.enabled` separately
        // and surfaces the warning in the dialog).
        let address = self.config.address();
        self.status = TransportStatus::Ready {
            since: std::time::Instant::now(),
            address: address.clone(),
        };
        Ok(TransportHandle {
            address,
            qr_payload: None,
            pid: None,
        })
    }

    async fn stop(&mut self) -> Result<(), TransportError> {
        self.status = TransportStatus::Stopped;
        Ok(())
    }

    fn status(&self) -> TransportStatus {
        self.status.clone()
    }

    fn is_starting(&self) -> bool {
        false
    }

    fn qr_payload(&self, ctx: &TransportContext) -> Result<QrPayload, TransportError> {
        if !matches!(self.status, TransportStatus::Ready { .. }) {
            return Err(TransportError::InvalidInput(
                "tailscale host is not resolved yet".into(),
            ));
        }
        Ok(QrPayload {
            kind: HostKind::Tailscale,
            url: self.config.address(),
            token: ctx.token.clone(),
            name: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tailscale_address_uses_wss_scheme() {
        let c = TailscaleConfig {
            magic_dns: "mac.tail-abc.ts.net".into(),
            port: 34123,
        };
        assert_eq!(c.address(), "wss://mac.tail-abc.ts.net:34123");
    }

    #[test]
    fn tailscale_rejects_empty_magic_dns() {
        let c = TailscaleConfig {
            magic_dns: "".into(),
            port: 34123,
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn tailscale_rejects_zero_port() {
        let c = TailscaleConfig {
            magic_dns: "mac.tail-abc.ts.net".into(),
            port: 0,
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn detects_tailnet_ip_cgnat_range() {
        assert!(TailscaleConfig::looks_like_tailnet_ip("100.64.0.1"));
        assert!(TailscaleConfig::looks_like_tailnet_ip("100.100.50.5"));
        assert!(TailscaleConfig::looks_like_tailnet_ip("100.127.255.254"));
        assert!(!TailscaleConfig::looks_like_tailnet_ip("100.63.0.1"));
        assert!(!TailscaleConfig::looks_like_tailnet_ip("100.128.0.1"));
        assert!(!TailscaleConfig::looks_like_tailnet_ip("192.168.1.5"));
        assert!(!TailscaleConfig::looks_like_tailnet_ip(
            "mac.tail-abc.ts.net"
        ));
    }
}
