use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Bumped whenever capability evidence semantics change; older caches are
/// invalidated rather than reinterpreted.
pub const SKILL_CAPABILITY_PROBE_VERSION: u32 = 4;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize, TS)]
pub enum CommandScope {
    Project,
    User,
    Skill,
    Builtin,
}

impl CommandScope {
    /// Presentation order in the composer picker. Resolution precedence is
    /// separate: project and user commands can still override less-specific
    /// commands with the same name.
    pub const fn display_rank(self) -> u8 {
        match self {
            Self::Builtin => 0,
            Self::Project => 1,
            Self::User => 2,
            Self::Skill => 3,
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Project => tr!("command_scope.project"),
            Self::User => tr!("command_scope.user"),
            Self::Skill => tr!("command_scope.skill"),
            Self::Builtin => tr!("command_scope.builtin"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub scope: CommandScope,
    pub argument_hint: Option<String>,
    pub template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_file: Option<String>,
}

/// Parse Padu's `/compact [focus]` command for the three supported harnesses.
///
/// `None` means the text is not a Padu compaction command. `Some(None)` is a
/// valid command without optional focus guidance. Codex has no focus field in
/// its native compaction API, so any typed focus is intentionally discarded.
pub fn parse_compaction_submission(
    provider: crate::model::ProviderKind,
    prompt: &str,
) -> Option<Option<String>> {
    if !provider.supports_compaction() {
        return None;
    }
    let invocation = prompt.trim();
    let remainder = invocation.strip_prefix("/compact")?;
    if !remainder.is_empty() && !remainder.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }
    let focus = remainder.trim();
    Some(
        (provider != crate::model::ProviderKind::Codex && !focus.is_empty())
            .then(|| focus.to_owned()),
    )
}

/// The native spellings supported by a provider's CLI.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SkillNativeSyntax {
    Dollar,
    SkillSlash,
}

/// Native leading-invocation spelling a provider can be probed for. This is
/// not capability evidence; placement and count flags still stay disabled
/// until an exact-binary probe records them.
pub fn native_skill_syntax(provider: crate::model::ProviderKind) -> Option<SkillNativeSyntax> {
    match provider {
        crate::model::ProviderKind::Codex | crate::model::ProviderKind::Fx => {
            Some(SkillNativeSyntax::Dollar)
        }
        crate::model::ProviderKind::Pi | crate::model::ProviderKind::OhMyPi => {
            Some(SkillNativeSyntax::SkillSlash)
        }
        _ => None,
    }
}

/// Probe-backed support for `$skill` mentions in provider-facing prompts.
///
/// This is optional catalog metadata. Missing or stale evidence means the
/// client must render the portable `SKILL.md` fallback instead of guessing.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SkillProviderCapability {
    pub native_syntax: Option<SkillNativeSyntax>,
    pub leading_invocation: bool,
    pub inline_mentions: bool,
    pub repeated_mentions: bool,
}

/// Exact executable that produced a provider capability observation.
///
/// The daemon computes `canonical_path` with filesystem canonicalization,
/// which makes it absolute and resolves symlinks. `content_sha256` is the
/// lowercase SHA-256 hex digest of every byte in that file. Evidence is only
/// valid for an exact match of both fields.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SkillBinaryIdentity {
    pub canonical_path: String,
    pub content_sha256: String,
}

impl SkillBinaryIdentity {
    pub fn is_valid(&self) -> bool {
        is_absolute_host_path(&self.canonical_path)
            && self.content_sha256.len() == 64
            && self
                .content_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }
}

/// Binary identities cross client platforms. A Windows desktop may validate
/// metadata produced by a Linux daemon, and the reverse is also possible, so
/// use the wire string's shape rather than the current host's path parser.
fn is_absolute_host_path(path: &str) -> bool {
    if path.is_empty() || path.contains('\0') || path.ends_with(['/', '\\']) {
        return false;
    }
    let bytes = path.as_bytes();
    if path.starts_with("\\\\") {
        path[2..]
            .split(['/', '\\'])
            .filter(|part| !part.is_empty())
            .count()
            >= 3
    } else if path.starts_with('/') {
        true
    } else {
        bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\')
    }
}

/// Exact-binary version-scoped evidence sent alongside a daemon-produced skill
/// catalog. Missing identity is treated as unknown and must not enable native
/// syntax.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SkillCapabilityMetadata {
    pub binary_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary_identity: Option<SkillBinaryIdentity>,
    pub probe_version: u32,
    pub capability: SkillProviderCapability,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCapabilityProbeObservations {
    pub leading_invocation: bool,
    pub inline_mention: bool,
    pub repeated_same_mention: bool,
    pub repeated_different_mentions: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCapabilityEvidence {
    pub provider: crate::model::ProviderKind,
    pub binary_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary_identity: Option<SkillBinaryIdentity>,
    pub probe_version: u32,
    pub observations: SkillCapabilityProbeObservations,
    pub capability: SkillProviderCapability,
}

impl SkillCapabilityEvidence {
    /// Cached evidence is trustworthy only when every capability flag can be
    /// derived from its observations and the provider's native syntax.
    pub fn is_consistent(&self) -> bool {
        self.capability.native_syntax == native_skill_syntax(self.provider)
            && self.capability.leading_invocation == self.observations.leading_invocation
            && self.capability.inline_mentions == self.observations.inline_mention
            && self.capability.repeated_mentions
                == (self.observations.repeated_same_mention
                    && self.observations.repeated_different_mentions)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct FileEntry {
    pub path: String,
    pub is_dir: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_command_skill_file_is_backward_compatible_and_optional_on_the_wire() {
        let legacy: SlashCommand = serde_json::from_value(serde_json::json!({
            "name": "deploy",
            "description": "Deploy it",
            "scope": "Skill",
            "argument_hint": null,
            "template": null
        }))
        .expect("older catalogs without skill_file must still deserialize");
        assert_eq!(legacy.skill_file, None);

        let serialized = serde_json::to_value(&legacy).unwrap();
        assert!(serialized.get("skill_file").is_none());

        let with_path = SlashCommand {
            skill_file: Some("/home/alice/.agents/skills/deploy/SKILL.md".into()),
            ..legacy
        };
        let serialized = serde_json::to_value(with_path).unwrap();
        assert_eq!(
            serialized["skill_file"],
            "/home/alice/.agents/skills/deploy/SKILL.md"
        );
    }

    #[test]
    fn binary_identity_accepts_paths_from_every_supported_daemon_host() {
        let digest = "a".repeat(64);
        for canonical_path in [
            "/home/alice/.local/bin/codex",
            "C:\\Tools\\codex.exe",
            "\\\\server\\share\\codex.exe",
        ] {
            assert!(
                SkillBinaryIdentity {
                    canonical_path: canonical_path.into(),
                    content_sha256: digest.clone(),
                }
                .is_valid(),
                "{canonical_path} should be accepted independent of the client OS"
            );
        }
        for canonical_path in ["relative/codex", "", "/home/alice/", "C:\\Tools\\"] {
            assert!(
                !SkillBinaryIdentity {
                    canonical_path: canonical_path.into(),
                    content_sha256: digest.clone(),
                }
                .is_valid(),
                "{canonical_path} should be rejected"
            );
        }
    }

    #[test]
    fn capability_evidence_must_be_derived_from_observations() {
        let mut evidence = SkillCapabilityEvidence {
            provider: crate::model::ProviderKind::Codex,
            binary_version: "1.2.3".into(),
            binary_identity: Some(SkillBinaryIdentity {
                canonical_path: "/opt/codex/bin/codex".into(),
                content_sha256: "a".repeat(64),
            }),
            probe_version: SKILL_CAPABILITY_PROBE_VERSION,
            observations: SkillCapabilityProbeObservations {
                leading_invocation: false,
                inline_mention: false,
                repeated_same_mention: false,
                repeated_different_mentions: false,
            },
            capability: SkillProviderCapability {
                native_syntax: Some(SkillNativeSyntax::Dollar),
                leading_invocation: false,
                inline_mentions: false,
                repeated_mentions: false,
            },
        };
        assert!(evidence.is_consistent());

        evidence.capability.leading_invocation = true;
        assert!(!evidence.is_consistent());
        evidence.capability.leading_invocation = false;

        evidence.capability.inline_mentions = true;
        assert!(!evidence.is_consistent());
        evidence.capability.inline_mentions = false;

        evidence.observations.repeated_same_mention = true;
        evidence.capability.repeated_mentions = true;
        assert!(!evidence.is_consistent());

        evidence.observations.repeated_different_mentions = true;
        assert!(evidence.is_consistent());
        evidence.capability.native_syntax = Some(SkillNativeSyntax::SkillSlash);
        assert!(!evidence.is_consistent());
    }
}
