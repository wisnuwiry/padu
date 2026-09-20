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

use parking_lot::Mutex;
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

/// Watch a spawned child process and mark its transport failed when it exits.
///
/// Without this, a `cloudflared` or `ssh` that dies after a successful start
/// would leave `TransportStatus` reporting `Ready` forever: the settings row
/// would claim a dead tunnel is connected, and the desktop would silently
/// retry the daemon connection behind it with nothing to show the user. A
/// deliberate `stop()` records `Stopped` first, which the watcher respects.
pub(crate) fn watch_child(
    mut child: std::process::Child,
    status: Arc<Mutex<TransportStatus>>,
    label: &'static str,
) {
    let spawned = std::thread::Builder::new()
        .name(format!("{label}-watch"))
        .spawn(move || {
            let outcome = child.wait();
            let mut guard = status.lock();
            if !matches!(
                *guard,
                TransportStatus::Ready { .. } | TransportStatus::Starting
            ) {
                return;
            }
            *guard = TransportStatus::Failed {
                error: match outcome {
                    Ok(exit) => format!("{label} exited ({exit})"),
                    Err(error) => format!("{label} could not be waited on: {error}"),
                },
            };
        });
    if let Err(error) = spawned {
        eprintln!("could not start the {label} watcher: {error}");
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
            let mut guard = self.inner.lock();
            guard.remove(&id)
        };
        if let Some(prev) = previous {
            let mut prev = futures_lite::future::block_on(prev.lock());
            let _ = futures_lite::future::block_on(prev.stop());
        }
        self.inner
            .lock()
            .insert(id, Arc::new(async_lock::Mutex::new(transport)));
    }

    pub fn get(&self, host_id: &str) -> Option<TransportSlot> {
        self.inner.lock().get(host_id).cloned()
    }

    /// Drop a transport slot without stopping it. Callers that need a graceful
    /// shutdown should `stop()` the transport first.
    pub fn remove(&self, host_id: &str) -> Option<TransportSlot> {
        self.inner.lock().remove(host_id)
    }

    /// Move a running transport to a different key, preserving the subprocess.
    /// Used when a host dialog's provisional tunnel is handed off to the saved
    /// profile's id. Returns `false` when no transport exists under `from`.
    pub fn rekey(&self, from: &str, to: &str) -> bool {
        let mut guard = self.inner.lock();
        match guard.remove(from) {
            Some(slot) => {
                guard.insert(to.to_string(), slot);
                true
            }
            None => false,
        }
    }

    /// Ensure a transport exists under `key` and is ready, returning its
    /// resolved address.
    ///
    /// An already-ready transport is reused, so switching back to a host does
    /// not spawn a second subprocess or rotate a Quick Tunnel URL.
    pub async fn ensure_started(
        &self,
        key: &str,
        make: impl FnOnce() -> Box<dyn Transport>,
        context: &TransportContext,
    ) -> Result<TransportHandle, TransportError> {
        if let Some(slot) = self.get(key) {
            let guard = slot.lock().await;
            if let TransportStatus::Ready { address, .. } = guard.status() {
                return Ok(TransportHandle {
                    address,
                    qr_payload: None,
                    pid: None,
                });
            }
            // A stopped or failed transport is replaced rather than reused.
            drop(guard);
            self.remove(key);
        }
        let mut transport = make();
        let handle = transport.start(context).await?;
        self.register(key, transport);
        Ok(handle)
    }

    /// Number of registered hosts.
    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Tear everything down — called from the `App` shutdown hook (§3.5).
    /// The registry's own lock is released before any transport is awaited.
    pub async fn shutdown_all(&self) {
        let entries: Vec<TransportSlot> = {
            let mut guard = self.inner.lock();
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
    fn ensure_started_reuses_a_ready_transport() {
        let registry = TransportRegistry::new();
        let (transport, starts, _stops) = make_recorder();
        let context = TransportContext {
            local_port: 34123,
            token: "tok".into(),
        };
        let first = futures_lite::future::block_on(registry.ensure_started(
            "host-a",
            || transport,
            &context,
        ))
        .unwrap();
        assert_eq!(first.address, "wss://recorder.test");

        // A second call with a factory that would panic if invoked: reuse
        // must win over starting a new transport.
        let second = futures_lite::future::block_on(registry.ensure_started(
            "host-a",
            || panic!("must not rebuild a ready transport"),
            &context,
        ))
        .unwrap();
        assert_eq!(second.address, "wss://recorder.test");
        assert_eq!(starts.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn ensure_started_replaces_a_stopped_transport() {
        let registry = TransportRegistry::new();
        let (transport, _starts, stops) = make_recorder();
        let context = TransportContext {
            local_port: 34123,
            token: "tok".into(),
        };
        futures_lite::future::block_on(registry.ensure_started("host-a", || transport, &context))
            .unwrap();
        // Stop it, then ensure again: the slot is replaced, not reused.
        let slot = registry.get("host-a").unwrap();
        futures_lite::future::block_on(async {
            slot.lock().await.stop().await.unwrap();
        });
        let (replacement, starts, _) = make_recorder();
        futures_lite::future::block_on(registry.ensure_started("host-a", || replacement, &context))
            .unwrap();
        assert_eq!(stops.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(starts.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn rekey_preserves_the_slot_and_moves_it() {
        let registry = TransportRegistry::new();
        let (transport, _starts, stops) = make_recorder();
        registry.register("draft-key", transport);
        assert!(registry.rekey("draft-key", "host-7"));
        assert!(registry.get("draft-key").is_none());
        assert!(registry.get("host-7").is_some());
        // Moving does not stop the transport.
        assert_eq!(stops.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(!registry.rekey("missing", "host-8"));
    }

    #[cfg(unix)]
    #[test]
    fn watch_child_marks_a_dead_transport_failed() {
        let status = Arc::new(Mutex::new(TransportStatus::Ready {
            since: Instant::now(),
            address: "wss://dead.test".into(),
        }));
        let child = std::process::Command::new("true").spawn().unwrap();
        watch_child(child, status.clone(), "test");

        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        while Instant::now() < deadline {
            if matches!(*status.lock(), TransportStatus::Failed { .. }) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            matches!(*status.lock(), TransportStatus::Failed { .. }),
            "a dead child must flip the status to Failed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn watch_child_respects_a_deliberate_stop() {
        // `stop()` records `Stopped` before the child is reaped, so the
        // watcher must not overwrite it with a failure.
        let status = Arc::new(Mutex::new(TransportStatus::Stopped));
        let child = std::process::Command::new("true").spawn().unwrap();
        watch_child(child, status.clone(), "test");

        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(matches!(*status.lock(), TransportStatus::Stopped));
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
