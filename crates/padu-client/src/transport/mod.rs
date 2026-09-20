//! Client-side transport runtime.
//!
//! A `Transport` is the sidecar that fronts the local daemon with a reachable
//! network address for clients that aren't on the same machine. Four kinds are
//! supported today:
//!
//! - [`Direct`]: no sidecar; the user-supplied `address` is used verbatim.
//! - [`Tailscale`]: resolves a MagicDNS / 100.x address to `wss://...`.
//! - [`Cloudflare`]: spawns `cloudflared tunnel --url` and captures the
//!   `*.trycloudflare.com` hostname it prints.
//! - [`Ssh`]: opens a reverse SSH tunnel via the user's local `ssh` client.
//!
//! Every transport is responsible for its own watchdog: it must surface
//! unexpected subprocess death via [`TransportStatus::Failed`] so the host
//! dialog can stop showing "Connecting" forever (§12.3 of the remote-host
//! transports plan).
//!
//! Subprocess death outside the transport itself (e.g. `ssh` flap) is
//! handled inside each transport's `start()` implementation; the
//! [`crate::daemon::DaemonSupervisor`] re-uses the resolved `address` for
//! its 500 ms re-poll loop.

pub mod cloudflare;
pub mod qr;
pub mod ssh;
pub mod tailscale;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use anyhow::Result;
use async_trait::async_trait;
use padu_protocol::persistence::HostKind;
use serde::{Deserialize, Serialize};

pub use cloudflare::CloudflareTransport;
pub use qr::{QrPayload, render_svg};
pub use ssh::SshTransport;
pub use tailscale::TailscaleTransport;

/// Inputs supplied to `Transport::start`.
#[derive(Clone, Debug)]
pub struct TransportContext {
    /// Local daemon port the transport must forward.
    pub local_port: u16,
    /// Daemon bearer token. Only used for QR payloads; never sent over the
    /// wire by the transport itself.
    pub token: String,
}

/// Output of a successful `start()`.
#[derive(Clone, Debug)]
pub struct TransportHandle {
    /// `wss://...` (or `ws://`) address a client uses to reach the daemon.
    pub address: String,
    /// Pre-encoded `padu://connect?...` payload for QR rendering.
    /// `Some` for Cloudflare (auto), `None` for others (caller can request).
    pub qr_payload: Option<String>,
    /// PID of the underlying subprocess (None for transports that don't spawn one).
    pub pid: Option<u32>,
}

/// Lifecycle status snapshot for the settings UI. Cheap to read.
#[derive(Clone, Debug, Default)]
pub enum TransportStatus {
    #[default]
    Idle,
    Starting,
    Ready {
        since: Instant,
        address: String,
    },
    Failed {
        error: String,
    },
    Stopped,
}

/// Structured error types a transport can return. The host dialog maps each
/// to a user-visible message + an optional help link.
#[derive(Clone, Debug, thiserror::Error, Serialize, Deserialize)]
pub enum TransportError {
    #[error("the required binary `{binary}` is not installed{why}")]
    BinaryMissing { binary: String, why: String },
    #[error("this feature requires a Tailscale account")]
    TailscaleNotRunning,
    #[error("cloudflared reported an account requirement: {hint}")]
    AccountRequired { hint: String },
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("transport did not become ready within {seconds}s")]
    StartupTimeout { seconds: u32 },
    #[error("transport exited unexpectedly: {0}")]
    Crashed(String),
    #[error("io error: {0}")]
    Io(String),
}

impl From<std::io::Error> for TransportError {
    fn from(value: std::io::Error) -> Self {
        TransportError::Io(value.to_string())
    }
}

/// Boxed async transport used by the host dialog. Each implementation manages
/// its own background watchdog task so callers don't need to poll.
#[async_trait]
pub trait Transport: Send {
    fn kind(&self) -> HostKind;

    /// Spawn / acquire the underlying resource. Must be idempotent: calling
    /// `start()` again after a successful start returns the existing handle
    /// without spawning a duplicate subprocess.
    async fn start(&mut self, ctx: &TransportContext) -> Result<TransportHandle, TransportError>;

    /// Best-effort tear-down. `Drop` also sends SIGTERM so this is mostly
    /// used to flush logs / reset state.
    async fn stop(&mut self) -> Result<(), TransportError>;

    /// Cheap snapshot for the settings status row. No I/O.
    fn status(&self) -> TransportStatus;

    /// True while `start()` is in flight. The dialog gates "Save and connect"
    /// on this.
    fn is_starting(&self) -> bool;

    /// Build a QR payload from the given context. Used to power the "Show QR"
    /// button for non-Cloudflare transports (§12.8 of the plan).
    fn qr_payload(&self, ctx: &TransportContext) -> Result<QrPayload, TransportError> {
        let _ = ctx;
        Err(TransportError::InvalidInput(
            "this transport does not support QR payloads".into(),
        ))
    }
}

/// Trivial "transport" for the `Direct` kind — no sidecar, no subprocess.
/// The address is whatever the user typed.
pub struct DirectTransport {
    address: String,
}

impl DirectTransport {
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
        }
    }
}

#[async_trait]
impl Transport for DirectTransport {
    fn kind(&self) -> HostKind {
        HostKind::Direct
    }

    async fn start(&mut self, _ctx: &TransportContext) -> Result<TransportHandle, TransportError> {
        Ok(TransportHandle {
            address: self.address.clone(),
            qr_payload: None,
            pid: None,
        })
    }

    async fn stop(&mut self) -> Result<(), TransportError> {
        Ok(())
    }

    fn status(&self) -> TransportStatus {
        TransportStatus::Ready {
            since: Instant::now(),
            address: self.address.clone(),
        }
    }

    fn is_starting(&self) -> bool {
        false
    }

    fn qr_payload(&self, ctx: &TransportContext) -> Result<QrPayload, TransportError> {
        Ok(QrPayload {
            kind: HostKind::Direct,
            url: self.address.clone(),
            token: ctx.token.clone(),
            name: String::new(),
        })
    }
}

/// Shared slot holding one live transport. The async mutex lets callers
/// `await` a transport operation without holding a `std` lock across an
/// await point.
pub type TransportSlot = Arc<async_lock::Mutex<Box<dyn Transport>>>;

/// Registry of live transports, keyed by host id. The desktop `Padu` app
/// keeps one of these on its state and looks up / creates / shuts-down
/// transports as the user switches hosts.
#[derive(Clone, Default)]
pub struct TransportRegistry {
    inner: Arc<Mutex<HashMap<String, TransportSlot>>>,
}

impl TransportRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a transport for the given host id. If a transport already
    /// exists for that id the new one replaces it (after stopping the old).
    pub fn register(&self, host_id: impl Into<String>, transport: Box<dyn Transport>) {
        let id = host_id.into();
        let previous = {
            let mut guard = self.inner.lock().expect("transport registry poisoned");
            guard.remove(&id)
        };
        if let Some(prev) = previous {
            let mut prev = futures_lite::future::block_on(prev.lock());
            let _ = futures_lite::future::block_on(prev.stop());
        }
        self.inner
            .lock()
            .expect("transport registry poisoned")
            .insert(id, Arc::new(async_lock::Mutex::new(transport)));
    }

    pub fn get(&self, host_id: &str) -> Option<TransportSlot> {
        self.inner
            .lock()
            .expect("transport registry poisoned")
            .get(host_id)
            .cloned()
    }

    /// Drop a transport slot without stopping it. Callers that need a graceful
    /// shutdown should `stop()` the transport first.
    pub fn remove(&self, host_id: &str) -> Option<TransportSlot> {
        self.inner
            .lock()
            .expect("transport registry poisoned")
            .remove(host_id)
    }

    /// Number of registered hosts.
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .expect("transport registry poisoned")
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Tear everything down — called from the `App` shutdown hook (§3.5).
    /// The registry's own lock is released before any transport is awaited.
    pub async fn shutdown_all(&self) {
        let entries: Vec<TransportSlot> = {
            let mut guard = self.inner.lock().expect("transport registry poisoned");
            guard.drain().map(|(_, t)| t).collect()
        };
        for transport in entries {
            let mut transport = transport.lock().await;
            let _ = transport.stop().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A transport that records start/stop calls — used to verify registry
    /// lifecycle without spawning subprocesses.
    struct RecorderTransport {
        starts: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        stops: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        status: std::sync::Arc<std::sync::Mutex<TransportStatus>>,
    }

    #[async_trait]
    impl Transport for RecorderTransport {
        fn kind(&self) -> HostKind {
            HostKind::Direct
        }
        async fn start(
            &mut self,
            _ctx: &TransportContext,
        ) -> Result<TransportHandle, TransportError> {
            self.starts
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            *self.status.lock().unwrap() = TransportStatus::Ready {
                since: Instant::now(),
                address: "wss://recorder.test".into(),
            };
            Ok(TransportHandle {
                address: "wss://recorder.test".into(),
                qr_payload: None,
                pid: None,
            })
        }
        async fn stop(&mut self) -> Result<(), TransportError> {
            self.stops.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            *self.status.lock().unwrap() = TransportStatus::Stopped;
            Ok(())
        }
        fn status(&self) -> TransportStatus {
            self.status.lock().unwrap().clone()
        }
        fn is_starting(&self) -> bool {
            false
        }
    }

    fn make_recorder() -> (
        Box<dyn Transport>,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) {
        let starts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let stops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let status = std::sync::Arc::new(std::sync::Mutex::new(TransportStatus::Idle));
        let transport = RecorderTransport {
            starts: starts.clone(),
            stops: stops.clone(),
            status,
        };
        (Box::new(transport), starts, stops)
    }

    #[test]
    fn direct_transport_passes_address_through() {
        let mut t = DirectTransport::new("wss://home.example.com:34123");
        let ctx = TransportContext {
            local_port: 34123,
            token: "tok".into(),
        };
        let handle = futures_lite::future::block_on(t.start(&ctx)).unwrap();
        assert_eq!(handle.address, "wss://home.example.com:34123");
        assert_eq!(handle.pid, None);
    }

    #[test]
    fn direct_transport_generates_qr_payload() {
        let t = DirectTransport::new("wss://home.example.com:34123");
        let ctx = TransportContext {
            local_port: 34123,
            token: "tok".into(),
        };
        let payload = t.qr_payload(&ctx).unwrap();
        assert_eq!(payload.kind, HostKind::Direct);
        assert_eq!(payload.url, "wss://home.example.com:34123");
        assert_eq!(payload.token, "tok");
    }

    #[test]
    fn registry_starts_one_transport_per_active_host() {
        let registry = TransportRegistry::new();
        let (a, _a_starts, _a_stops) = make_recorder();
        let (b, _b_starts, _b_stops) = make_recorder();
        registry.register("host-a", a);
        registry.register("host-b", b);
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn registry_shutdown_kills_running_transports() {
        let registry = TransportRegistry::new();
        let (transport, _starts, stops) = make_recorder();
        registry.register("host-a", transport);
        futures_lite::future::block_on(registry.shutdown_all());
        assert_eq!(stops.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(registry.is_empty());
    }

    #[test]
    fn registry_replaces_existing_transport_and_stops_old() {
        let registry = TransportRegistry::new();
        let (t1, _s1, stops1) = make_recorder();
        let (t2, _s2, _stops2) = make_recorder();
        registry.register("host-a", t1);
        registry.register("host-a", t2);
        assert_eq!(registry.len(), 1);
        // The old transport should have been stopped exactly once on replace.
        assert_eq!(stops1.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
