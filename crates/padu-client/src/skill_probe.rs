//! Opt-in evidence and cache for `$skill` provider capability probes.
//!
//! Normal composer startup never runs a provider. This module records the
//! result of an explicit disposable-session probe and invalidates the evidence
//! when the exact provider executable, binary version, or probe-evidence
//! semantics change.

use std::path::{Path, PathBuf};

pub use crate::composer_complete::SkillNativeSyntax;
use crate::composer_complete::{SkillProviderCapability, provider_skill_capability};
pub use padu_protocol::composer::SKILL_CAPABILITY_PROBE_VERSION;
pub use padu_protocol::composer::{
    SkillBinaryIdentity, SkillCapabilityEvidence, SkillCapabilityMetadata,
    SkillCapabilityProbeObservations,
};
use padu_protocol::model::ProviderKind;

const CACHE_FILE: &str = "skill-capabilities.json";

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillCapabilityCache {
    version: u32,
    entries: Vec<SkillCapabilityEvidence>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillInvocationObservation {
    LeadingInvocation,
    InlineMention,
    RepeatedSameMention,
    RepeatedDifferentMentions,
}

/// A provider-emitted signal that the exact probe observed a skill invocation.
///
/// Model-authored text is deliberately not representable here. A future
/// driver event can populate this type only after its provider payload proves
/// that the provider interpreted the native skill syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillInvocationEvidence {
    pub provider: ProviderKind,
    pub binary_version: String,
    pub binary_identity: SkillBinaryIdentity,
    pub probe_version: u32,
    pub observation: SkillInvocationObservation,
}

impl SkillInvocationEvidence {
    pub fn new(
        provider: ProviderKind,
        binary_version: impl Into<String>,
        binary_identity: SkillBinaryIdentity,
        probe_version: u32,
        observation: SkillInvocationObservation,
    ) -> Self {
        Self {
            provider,
            binary_version: binary_version.into(),
            binary_identity,
            probe_version,
            observation,
        }
    }
}

/// Convert structured provider invocation observations into renderer capability
/// evidence. Only exact provider, executable identity, binary-version, and
/// probe-version matches count; missing or mismatched evidence never enables
/// native syntax.
pub fn evaluate_skill_capability_probe(
    provider: ProviderKind,
    binary_version: Option<&str>,
    binary_identity: Option<&SkillBinaryIdentity>,
    evidence: &[SkillInvocationEvidence],
) -> Option<SkillCapabilityEvidence> {
    let binary_version = binary_version?.trim();
    let binary_identity = binary_identity.filter(|identity| identity.is_valid())?;
    if binary_version.is_empty() {
        return None;
    }
    let native_syntax = native_skill_syntax(provider);
    let mut observations = SkillCapabilityProbeObservations {
        leading_invocation: false,
        inline_mention: false,
        repeated_same_mention: false,
        repeated_different_mentions: false,
    };
    if native_syntax.is_some() {
        for event in evidence.iter().filter(|event| {
            event.provider == provider
                && event.binary_version.trim() == binary_version
                && &event.binary_identity == binary_identity
                && event.probe_version == SKILL_CAPABILITY_PROBE_VERSION
        }) {
            match event.observation {
                SkillInvocationObservation::LeadingInvocation => {
                    observations.leading_invocation = true;
                }
                SkillInvocationObservation::InlineMention => {
                    observations.inline_mention = true;
                }
                SkillInvocationObservation::RepeatedSameMention => {
                    observations.repeated_same_mention = true;
                }
                SkillInvocationObservation::RepeatedDifferentMentions => {
                    observations.repeated_different_mentions = true;
                }
            }
        }
    }
    Some(SkillCapabilityEvidence {
        provider,
        binary_version: binary_version.to_owned(),
        binary_identity: Some(binary_identity.clone()),
        probe_version: SKILL_CAPABILITY_PROBE_VERSION,
        observations,
        capability: SkillProviderCapability {
            native_syntax,
            leading_invocation: observations.leading_invocation,
            inline_mentions: observations.inline_mention,
            repeated_mentions: observations.repeated_same_mention
                && observations.repeated_different_mentions,
        },
    })
}

pub fn native_skill_syntax(provider: ProviderKind) -> Option<SkillNativeSyntax> {
    padu_protocol::composer::native_skill_syntax(provider)
}

/// Return metadata-backed capability only when it belongs to the exact
/// executable and version resolved immediately before use. Any unknown or
/// mismatched input returns the conservative provider default.
pub fn verified_skill_capability(
    metadata: Option<&SkillCapabilityMetadata>,
    provider: ProviderKind,
    binary_version: Option<&str>,
    binary_identity: Option<&SkillBinaryIdentity>,
) -> SkillProviderCapability {
    let fallback = provider_skill_capability(provider);
    let Some(metadata) = metadata else {
        return fallback;
    };
    let Some(binary_version) = binary_version.map(str::trim).filter(|v| !v.is_empty()) else {
        return fallback;
    };
    let Some(binary_identity) = binary_identity.filter(|identity| identity.is_valid()) else {
        return fallback;
    };
    if metadata.probe_version != SKILL_CAPABILITY_PROBE_VERSION
        || metadata.binary_version.trim() != binary_version
        || metadata.binary_identity.as_ref() != Some(binary_identity)
        || metadata.capability.native_syntax != native_skill_syntax(provider)
    {
        return fallback;
    }
    metadata.capability
}

pub fn skill_capability_cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|root| root.join("padu").join(CACHE_FILE))
}

pub fn read_skill_capability_evidence(
    cache_path: &Path,
    provider: ProviderKind,
    binary_version: Option<&str>,
    binary_identity: Option<&SkillBinaryIdentity>,
) -> Option<SkillCapabilityEvidence> {
    let binary_version = binary_version?.trim();
    let binary_identity = binary_identity.filter(|identity| identity.is_valid())?;
    if binary_version.is_empty() {
        return None;
    }
    let cache = read_cache(cache_path)?;
    cache.entries.into_iter().find(|entry| {
        entry.probe_version == SKILL_CAPABILITY_PROBE_VERSION
            && entry.provider == provider
            && entry.binary_version == binary_version
            && entry.binary_identity.as_ref() == Some(binary_identity)
            && entry.is_consistent()
    })
}

pub fn write_skill_capability_evidence(
    cache_path: &Path,
    evidence: &SkillCapabilityEvidence,
) -> std::io::Result<()> {
    if !evidence.is_consistent() {
        return Ok(());
    }
    let Some(binary_identity) = evidence
        .binary_identity
        .as_ref()
        .filter(|identity| identity.is_valid())
    else {
        return Ok(());
    };
    if evidence.binary_version.trim().is_empty()
        || evidence.probe_version != SKILL_CAPABILITY_PROBE_VERSION
    {
        return Ok(());
    }
    let mut cache = read_cache(cache_path).unwrap_or(SkillCapabilityCache {
        version: SKILL_CAPABILITY_PROBE_VERSION,
        entries: Vec::new(),
    });
    cache.version = SKILL_CAPABILITY_PROBE_VERSION;
    cache.entries.retain(|entry| {
        entry.provider != evidence.provider
            || entry
                .binary_identity
                .as_ref()
                .map(|identity| identity.canonical_path.as_str())
                != Some(binary_identity.canonical_path.as_str())
    });
    cache.entries.push(evidence.clone());
    cache.entries.sort_by(|left, right| {
        (
            left.provider.id(),
            left.binary_identity
                .as_ref()
                .map(|identity| identity.canonical_path.as_str())
                .unwrap_or_default(),
            left.binary_identity
                .as_ref()
                .map(|identity| identity.content_sha256.as_str())
                .unwrap_or_default(),
            &left.binary_version,
        )
            .cmp(&(
                right.provider.id(),
                right
                    .binary_identity
                    .as_ref()
                    .map(|identity| identity.canonical_path.as_str())
                    .unwrap_or_default(),
                right
                    .binary_identity
                    .as_ref()
                    .map(|identity| identity.content_sha256.as_str())
                    .unwrap_or_default(),
                &right.binary_version,
            ))
    });

    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_vec_pretty(&cache).map_err(std::io::Error::other)?;
    let temporary = cache_path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temporary, serialized)?;
    std::fs::rename(temporary, cache_path)
}

fn read_cache(cache_path: &Path) -> Option<SkillCapabilityCache> {
    let bytes = std::fs::read(cache_path).ok()?;
    let cache = serde_json::from_slice::<SkillCapabilityCache>(&bytes).ok()?;
    (cache.version == SKILL_CAPABILITY_PROBE_VERSION).then_some(cache)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(path: &str, digest_byte: char) -> SkillBinaryIdentity {
        SkillBinaryIdentity {
            canonical_path: path.into(),
            content_sha256: digest_byte.to_string().repeat(64),
        }
    }

    fn complete_evidence(binary_identity: &SkillBinaryIdentity) -> Vec<SkillInvocationEvidence> {
        [
            SkillInvocationObservation::LeadingInvocation,
            SkillInvocationObservation::InlineMention,
            SkillInvocationObservation::RepeatedSameMention,
            SkillInvocationObservation::RepeatedDifferentMentions,
        ]
        .into_iter()
        .map(|observation| {
            SkillInvocationEvidence::new(
                ProviderKind::Codex,
                "1.2.3",
                binary_identity.clone(),
                SKILL_CAPABILITY_PROBE_VERSION,
                observation,
            )
        })
        .collect()
    }

    #[test]
    fn matching_structured_evidence_enables_each_observed_capability() {
        let binary_identity = identity("/opt/codex/bin/codex", 'a');
        let evidence = evaluate_skill_capability_probe(
            ProviderKind::Codex,
            Some("1.2.3"),
            Some(&binary_identity),
            &complete_evidence(&binary_identity),
        )
        .expect("matching binary identity and version");
        assert_eq!(
            evidence.capability,
            SkillProviderCapability {
                native_syntax: Some(SkillNativeSyntax::Dollar),
                leading_invocation: true,
                inline_mentions: true,
                repeated_mentions: true,
            }
        );
        assert_eq!(evidence.binary_identity, Some(binary_identity));
    }

    #[test]
    fn verified_capability_requires_matching_identity_and_version() {
        let binary_identity = identity("/opt/codex/bin/codex", 'a');
        let evidence = evaluate_skill_capability_probe(
            ProviderKind::Codex,
            Some("1.2.3"),
            Some(&binary_identity),
            &complete_evidence(&binary_identity),
        )
        .expect("matching binary identity and version");
        let metadata = SkillCapabilityMetadata {
            binary_version: evidence.binary_version.clone(),
            binary_identity: evidence.binary_identity.clone(),
            probe_version: evidence.probe_version,
            capability: evidence.capability,
        };
        assert_eq!(
            verified_skill_capability(
                Some(&metadata),
                ProviderKind::Codex,
                Some("1.2.3"),
                Some(&binary_identity)
            ),
            evidence.capability
        );
        assert!(
            !verified_skill_capability(
                Some(&metadata),
                ProviderKind::Codex,
                Some("1.2.3"),
                Some(&identity("/opt/other/codex", 'a'))
            )
            .leading_invocation
        );
        assert!(
            !verified_skill_capability(
                Some(&metadata),
                ProviderKind::Codex,
                Some("1.2.4"),
                Some(&binary_identity)
            )
            .leading_invocation
        );
        assert!(
            !verified_skill_capability(Some(&metadata), ProviderKind::Codex, Some("1.2.3"), None)
                .leading_invocation
        );
    }

    #[test]
    fn two_executables_reporting_the_same_version_do_not_share_evidence() {
        let first = identity("/opt/codex-a", 'a');
        let second = identity("/opt/codex-b", 'a');
        let result = evaluate_skill_capability_probe(
            ProviderKind::Codex,
            Some("1.2.3"),
            Some(&second),
            &complete_evidence(&first),
        )
        .expect("the target itself is known");
        assert_eq!(
            result.capability,
            SkillProviderCapability {
                native_syntax: Some(SkillNativeSyntax::Dollar),
                leading_invocation: false,
                inline_mentions: false,
                repeated_mentions: false,
            }
        );
        assert_eq!(result.binary_identity, Some(second));
    }

    #[test]
    fn missing_or_mismatched_identity_fails_closed() {
        let binary_identity = identity("/opt/codex/bin/codex", 'a');
        let missing = evaluate_skill_capability_probe(
            ProviderKind::Codex,
            Some("1.2.3"),
            Some(&binary_identity),
            &[],
        )
        .expect("matching binary identity and version");
        assert_eq!(
            missing.capability,
            SkillProviderCapability {
                native_syntax: Some(SkillNativeSyntax::Dollar),
                leading_invocation: false,
                inline_mentions: false,
                repeated_mentions: false,
            }
        );

        let mismatched_identity = identity("/opt/other/codex", 'a');
        let mismatched = [
            SkillInvocationEvidence::new(
                ProviderKind::Pi,
                "1.2.3",
                binary_identity.clone(),
                SKILL_CAPABILITY_PROBE_VERSION,
                SkillInvocationObservation::LeadingInvocation,
            ),
            SkillInvocationEvidence::new(
                ProviderKind::Codex,
                "9.9.9",
                binary_identity.clone(),
                SKILL_CAPABILITY_PROBE_VERSION,
                SkillInvocationObservation::InlineMention,
            ),
            SkillInvocationEvidence::new(
                ProviderKind::Codex,
                "1.2.3",
                mismatched_identity,
                SKILL_CAPABILITY_PROBE_VERSION,
                SkillInvocationObservation::RepeatedSameMention,
            ),
            SkillInvocationEvidence::new(
                ProviderKind::Codex,
                "1.2.3",
                binary_identity.clone(),
                SKILL_CAPABILITY_PROBE_VERSION - 1,
                SkillInvocationObservation::RepeatedDifferentMentions,
            ),
        ];
        let result = evaluate_skill_capability_probe(
            ProviderKind::Codex,
            Some("1.2.3"),
            Some(&binary_identity),
            &mismatched,
        )
        .expect("the target itself is known");
        assert_eq!(
            result.capability,
            SkillProviderCapability {
                native_syntax: Some(SkillNativeSyntax::Dollar),
                leading_invocation: false,
                inline_mentions: false,
                repeated_mentions: false,
            }
        );
        assert!(
            evaluate_skill_capability_probe(
                ProviderKind::Codex,
                Some("1.2.3"),
                None,
                &complete_evidence(&binary_identity),
            )
            .is_none()
        );
        assert!(
            evaluate_skill_capability_probe(
                ProviderKind::Codex,
                None,
                Some(&binary_identity),
                &complete_evidence(&binary_identity),
            )
            .is_none()
        );
    }

    #[test]
    fn repeated_capability_requires_both_structured_observations() {
        let binary_identity = identity("/opt/codex/bin/codex", 'a');
        let only_same = complete_evidence(&binary_identity)
            .into_iter()
            .filter(|event| {
                event.observation != SkillInvocationObservation::RepeatedDifferentMentions
            })
            .collect::<Vec<_>>();
        let evidence = evaluate_skill_capability_probe(
            ProviderKind::Codex,
            Some("1.2.3"),
            Some(&binary_identity),
            &only_same,
        )
        .expect("matching binary identity and version");
        assert!(!evidence.capability.repeated_mentions);
    }

    #[test]
    fn providers_without_a_native_mapping_stay_fail_closed() {
        let binary_identity = identity("/opt/deepseek/bin/dsh", 'b');
        let leading = [SkillInvocationEvidence::new(
            ProviderKind::DeepSeek,
            "1.2.3",
            binary_identity.clone(),
            SKILL_CAPABILITY_PROBE_VERSION,
            SkillInvocationObservation::LeadingInvocation,
        )];
        let evidence = evaluate_skill_capability_probe(
            ProviderKind::DeepSeek,
            Some("1.2.3"),
            Some(&binary_identity),
            &leading,
        )
        .expect("matching binary identity and version");
        assert_eq!(
            evidence.capability,
            SkillProviderCapability {
                native_syntax: None,
                leading_invocation: false,
                inline_mentions: false,
                repeated_mentions: false,
            }
        );
    }

    #[test]
    fn cache_is_scoped_to_identity_and_invalidates_changed_content_or_old_evidence() {
        let root = std::env::temp_dir().join(format!(
            "padu-skill-capability-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let path = root.join(CACHE_FILE);
        let first_identity = identity("/opt/pi-a", 'a');
        let second_identity = identity("/opt/pi-b", 'a');
        let changed_identity = identity("/opt/pi-a", 'c');
        let first = evaluate_skill_capability_probe(
            ProviderKind::Pi,
            Some("4.0.0"),
            Some(&first_identity),
            &[],
        )
        .expect("matching first binary identity");
        write_skill_capability_evidence(&path, &first).unwrap();

        assert_eq!(
            read_skill_capability_evidence(
                &path,
                ProviderKind::Pi,
                Some("4.0.0"),
                Some(&first_identity)
            ),
            Some(first.clone())
        );
        assert!(
            read_skill_capability_evidence(
                &path,
                ProviderKind::Pi,
                Some("4.0.0"),
                Some(&second_identity)
            )
            .is_none()
        );
        assert!(
            read_skill_capability_evidence(
                &path,
                ProviderKind::Pi,
                Some("4.0.0"),
                Some(&changed_identity)
            )
            .is_none()
        );
        assert!(
            read_skill_capability_evidence(&path, ProviderKind::Pi, Some("4.0.0"), None).is_none()
        );
        assert!(
            read_skill_capability_evidence(
                &path,
                ProviderKind::Pi,
                Some("4.0.1"),
                Some(&first_identity)
            )
            .is_none()
        );

        let second = evaluate_skill_capability_probe(
            ProviderKind::Pi,
            Some("4.0.0"),
            Some(&second_identity),
            &[],
        )
        .expect("matching second binary identity");
        write_skill_capability_evidence(&path, &second).unwrap();
        assert!(
            read_skill_capability_evidence(
                &path,
                ProviderKind::Pi,
                Some("4.0.0"),
                Some(&first_identity)
            )
            .is_some()
        );
        assert_eq!(
            read_skill_capability_evidence(
                &path,
                ProviderKind::Pi,
                Some("4.0.0"),
                Some(&second_identity)
            ),
            Some(second)
        );

        let changed = evaluate_skill_capability_probe(
            ProviderKind::Pi,
            Some("4.0.0"),
            Some(&changed_identity),
            &[],
        )
        .expect("matching changed binary identity");
        write_skill_capability_evidence(&path, &changed).unwrap();
        assert!(
            read_skill_capability_evidence(
                &path,
                ProviderKind::Pi,
                Some("4.0.0"),
                Some(&first_identity)
            )
            .is_none()
        );

        let mut old_cache: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        old_cache["version"] = serde_json::json!(SKILL_CAPABILITY_PROBE_VERSION - 1);
        std::fs::write(&path, serde_json::to_vec(&old_cache).unwrap()).unwrap();
        assert!(
            read_skill_capability_evidence(
                &path,
                ProviderKind::Pi,
                Some("4.0.0"),
                Some(&changed_identity)
            )
            .is_none()
        );

        write_skill_capability_evidence(&path, &changed).unwrap();
        assert_eq!(
            read_skill_capability_evidence(
                &path,
                ProviderKind::Pi,
                Some("4.0.0"),
                Some(&changed_identity)
            ),
            Some(changed)
        );

        let mut missing_identity: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        missing_identity["entries"][0]
            .as_object_mut()
            .unwrap()
            .remove("binaryIdentity");
        std::fs::write(&path, serde_json::to_vec(&missing_identity).unwrap()).unwrap();
        assert!(
            read_skill_capability_evidence(
                &path,
                ProviderKind::Pi,
                Some("4.0.0"),
                Some(&changed_identity)
            )
            .is_none()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn inconsistent_cached_capabilities_are_rejected() {
        let root = std::env::temp_dir().join(format!(
            "padu-skill-capability-inconsistent-{}",
            std::process::id()
        ));
        let path = root.join(CACHE_FILE);
        let binary_identity = identity("/opt/codex/bin/codex", 'a');
        let evidence = evaluate_skill_capability_probe(
            ProviderKind::Codex,
            Some("1.2.3"),
            Some(&binary_identity),
            &complete_evidence(&binary_identity),
        )
        .expect("matching binary identity and version");
        let mut inconsistent_flags = evidence.clone();
        inconsistent_flags.capability.leading_invocation = false;
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            &path,
            serde_json::to_vec(&SkillCapabilityCache {
                version: SKILL_CAPABILITY_PROBE_VERSION,
                entries: vec![inconsistent_flags],
            })
            .unwrap(),
        )
        .unwrap();
        assert!(
            read_skill_capability_evidence(
                &path,
                ProviderKind::Codex,
                Some("1.2.3"),
                Some(&binary_identity)
            )
            .is_none()
        );

        let mut mismatched_syntax = evidence.clone();
        mismatched_syntax.capability.native_syntax = Some(SkillNativeSyntax::SkillSlash);
        std::fs::write(
            &path,
            serde_json::to_vec(&SkillCapabilityCache {
                version: SKILL_CAPABILITY_PROBE_VERSION,
                entries: vec![mismatched_syntax],
            })
            .unwrap(),
        )
        .unwrap();
        assert!(
            read_skill_capability_evidence(
                &path,
                ProviderKind::Codex,
                Some("1.2.3"),
                Some(&binary_identity)
            )
            .is_none()
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
