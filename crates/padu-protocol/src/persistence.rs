use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::model::{AgentSession, MessageRole};
use crate::notes::EmbeddedNote;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ComposerDraftAttachment {
    #[ts(type = "string")]
    pub path: PathBuf,
    pub mention: String,
    pub name: String,
    pub is_dir: bool,
    pub is_image: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob_reference: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ComposerDraft {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<ComposerDraftAttachment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub embedded_notes: Vec<EmbeddedNote>,
}

impl ComposerDraft {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.attachments.is_empty() && self.embedded_notes.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ComposerDrafts {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub new_sessions: HashMap<Uuid, ComposerDraft>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub sessions: HashMap<Uuid, ComposerDraft>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComposerDraftKey {
    NewSession(Uuid),
    Session(Uuid),
}

/// Wire-safe identity for one independently persisted composer draft.
///
/// Draft updates are keyed so multiple connected clients cannot overwrite
/// unrelated drafts by sending stale whole-file snapshots.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ComposerDraftTarget {
    NewSession {
        #[ts(type = "string")]
        project_id: Uuid,
    },
    Session {
        #[ts(type = "string")]
        session_id: Uuid,
    },
}

impl From<ComposerDraftKey> for ComposerDraftTarget {
    fn from(key: ComposerDraftKey) -> Self {
        match key {
            ComposerDraftKey::NewSession(project_id) => Self::NewSession { project_id },
            ComposerDraftKey::Session(session_id) => Self::Session { session_id },
        }
    }
}

impl From<ComposerDraftTarget> for ComposerDraftKey {
    fn from(target: ComposerDraftTarget) -> Self {
        match target {
            ComposerDraftTarget::NewSession { project_id } => Self::NewSession(project_id),
            ComposerDraftTarget::Session { session_id } => Self::Session(session_id),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ComposerDraftChange {
    pub target: ComposerDraftTarget,
    /// `None` removes the target. Empty drafts are normalized to removal too.
    pub draft: Option<ComposerDraft>,
}

impl ComposerDraftKey {
    pub fn for_session(session: &AgentSession) -> Self {
        if session.has_started() {
            Self::Session(session.id)
        } else {
            Self::NewSession(session.project_id)
        }
    }
}

impl ComposerDrafts {
    pub fn get_for(&self, session: &AgentSession) -> Option<&ComposerDraft> {
        self.get(ComposerDraftKey::for_session(session))
    }

    pub fn get(&self, key: ComposerDraftKey) -> Option<&ComposerDraft> {
        match key {
            ComposerDraftKey::NewSession(project_id) => self.new_sessions.get(&project_id),
            ComposerDraftKey::Session(session_id) => self.sessions.get(&session_id),
        }
    }

    pub fn set(&mut self, key: ComposerDraftKey, draft: ComposerDraft) -> bool {
        let (drafts, id) = match key {
            ComposerDraftKey::NewSession(project_id) => (&mut self.new_sessions, project_id),
            ComposerDraftKey::Session(session_id) => (&mut self.sessions, session_id),
        };
        if draft.is_empty() {
            drafts.remove(&id).is_some()
        } else if drafts.get(&id) == Some(&draft) {
            false
        } else {
            drafts.insert(id, draft);
            true
        }
    }

    pub fn remove(&mut self, key: ComposerDraftKey) -> bool {
        match key {
            ComposerDraftKey::NewSession(project_id) => {
                self.new_sessions.remove(&project_id).is_some()
            }
            ComposerDraftKey::Session(session_id) => self.sessions.remove(&session_id).is_some(),
        }
    }

    pub fn move_to_empty(
        &mut self,
        source: ComposerDraftKey,
        destination: ComposerDraftKey,
    ) -> bool {
        if source == destination || self.get(destination).is_some_and(|draft| !draft.is_empty()) {
            return false;
        }
        let Some(draft) = self.get(source).cloned() else {
            return false;
        };
        self.remove(source);
        self.set(destination, draft)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct SessionMessageMatch {
    pub session_id: Uuid,
    pub source: MessageRole,
    pub snippet: String,
}

/// Discriminator for how a desktop / mobile / web client reaches a Padu
/// daemon over the network. Drives which desktop transport is spawned and
/// which fields on `HostProfile` are populated.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum HostKind {
    /// User-supplied WebSocket URL — the original behaviour, used whenever
    /// a static reachable address is available.
    #[default]
    Direct,
    /// Tailnet address: MagicDNS name or 100.x.y.z IP plus the daemon's
    /// bound port. The daemon must be exposed on a non-loopback bind.
    Tailscale,
    /// `trycloudflare.com` Quick Tunnel (or future named tunnel) fronting
    /// the local daemon. The desktop generates a QR code; the mobile app
    /// scans it to capture `address` + `token`.
    Cloudflare,
    /// Reverse SSH relay: the desktop opens `ssh -R` to a jump host and
    /// reaches the daemon at the remote-bound port.
    SshRelay,
}

/// Tailnet-specific knobs. Resolved `address` is still `wss://<host>:<port>`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TailscaleHostConfig {
    /// MagicDNS name (e.g. `my-mac.tail-abc.ts.net`) or a raw 100.x.y.z IP.
    pub magic_dns: String,
    /// Daemon-side port that must be reachable over the Tailnet.
    pub port: u16,
}

/// Cloudflare Tunnel knobs. The resolved `address` is the Quick-Tunnel
/// hostname observed from the `cloudflared` startup banner.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CloudflareHostConfig {
    /// Hostname observed from cloudflared (e.g. `xyz.trycloudflare.com`).
    pub hostname: String,
    /// `true` for the account-less `cloudflared tunnel --url ...` flow;
    /// `false` for a future named-tunnel integration.
    #[serde(default)]
    pub quick_tunnel: bool,
}

/// Reverse-SSH-relay knobs. The resolved `address` is
/// `wss://<ssh.host>:<remote_port>`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SshHostConfig {
    pub user: String,
    pub host: String,
    /// Port the remote sshd binds for the `-R` forward.
    pub remote_port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_file: Option<String>,
    /// Reserved for the future `--bind-tls` daemon flag. v1 always uses
    /// plain ws:// on the loopback end; cloudflared/SSH terminate TLS.
    #[serde(default)]
    pub use_tls: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HostProfile {
    pub id: String,
    pub name: String,
    /// Transport discriminator. Defaults to `Direct` when absent (legacy
    /// `app.json` files written before this field landed).
    #[serde(default)]
    pub kind: HostKind,
    /// Resolved WebSocket URL — the only field `DaemonSupervisor::connect`
    /// ever reads. Populated from the transport at save time.
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tailscale: Option<TailscaleHostConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloudflare: Option<CloudflareHostConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh: Option<SshHostConfig>,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_connected_at: Option<u64>,
}

impl HostProfile {
    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.address
        } else {
            &self.name
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_profile(kind: HostKind) -> HostProfile {
        HostProfile {
            id: "host-1".into(),
            name: "Sample".into(),
            kind,
            address: "wss://example.test:34123".into(),
            token: None,
            tailscale: None,
            cloudflare: None,
            ssh: None,
            created_at: 1,
            updated_at: 1,
            last_connected_at: None,
        }
    }

    #[test]
    fn host_profile_round_trips_with_all_kinds() {
        for kind in [
            HostKind::Direct,
            HostKind::Tailscale,
            HostKind::Cloudflare,
            HostKind::SshRelay,
        ] {
            let profile = base_profile(kind);
            let json = serde_json::to_string(&profile).unwrap();
            let parsed: HostProfile = serde_json::from_str(&json).unwrap();
            assert_eq!(profile, parsed, "round-trip failed for {kind:?}");
            assert_eq!(parsed.kind, kind);
        }
    }

    #[test]
    fn host_profile_defaults_to_direct_when_kind_missing() {
        let json = r#"{"id":"x","name":"y","address":"wss://z","createdAt":0,"updatedAt":0}"#;
        let parsed: HostProfile = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.kind, HostKind::Direct);
    }

    #[test]
    fn host_profile_omits_transport_blocks_when_none() {
        let profile = base_profile(HostKind::Direct);
        let json = serde_json::to_string(&profile).unwrap();
        assert!(!json.contains("\"tailscale\""));
        assert!(!json.contains("\"cloudflare\""));
        assert!(!json.contains("\"ssh\""));
    }

    #[test]
    fn transport_specific_blocks_round_trip() {
        let profile = HostProfile {
            tailscale: Some(TailscaleHostConfig {
                magic_dns: "mac.tail-abc.ts.net".into(),
                port: 34123,
            }),
            cloudflare: Some(CloudflareHostConfig {
                hostname: "xyz.trycloudflare.com".into(),
                quick_tunnel: true,
            }),
            ssh: Some(SshHostConfig {
                user: "alice".into(),
                host: "jump.example.com".into(),
                remote_port: 19999,
                identity_file: Some("/Users/alice/.ssh/id_ed25519".into()),
                use_tls: false,
            }),
            ..base_profile(HostKind::SshRelay)
        };
        let json = serde_json::to_string(&profile).unwrap();
        let parsed: HostProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile, parsed);
        assert_eq!(
            parsed.tailscale.as_ref().unwrap().magic_dns,
            "mac.tail-abc.ts.net"
        );
        assert!(parsed.cloudflare.as_ref().unwrap().quick_tunnel);
        assert_eq!(parsed.ssh.as_ref().unwrap().remote_port, 19999);
    }
}
