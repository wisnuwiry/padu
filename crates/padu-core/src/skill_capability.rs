//! Read provider skill capability evidence on the daemon host.
//!
//! The opt-in provider probe writes this cache. Catalog discovery only reads
//! it and attaches evidence only when both the resolved executable identity
//! and version match. A missing, stale, or malformed cache always means
//! portable fallback.

use std::path::Path;

use padu_protocol::composer::{
    SKILL_CAPABILITY_PROBE_VERSION, SkillBinaryIdentity, SkillCapabilityEvidence,
    SkillCapabilityMetadata,
};
use padu_protocol::model::ProviderKind;
use serde::Deserialize;

const CACHE_FILE: &str = "skill-capabilities.json";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillCapabilityCache {
    version: u32,
    entries: Vec<SkillCapabilityEvidence>,
}

pub fn read_skill_capability_evidence(
    provider: ProviderKind,
    binary_version: Option<&str>,
    binary_identity: Option<&SkillBinaryIdentity>,
) -> Option<SkillCapabilityMetadata> {
    let cache_path = dirs::cache_dir()?.join("padu").join(CACHE_FILE);
    read_skill_capability_evidence_at(&cache_path, provider, binary_version, binary_identity)
}

fn read_skill_capability_evidence_at(
    cache_path: &Path,
    provider: ProviderKind,
    binary_version: Option<&str>,
    binary_identity: Option<&SkillBinaryIdentity>,
) -> Option<SkillCapabilityMetadata> {
    let binary_version = binary_version?.trim();
    let binary_identity = binary_identity.filter(|identity| identity.is_valid())?;
    if binary_version.is_empty() {
        return None;
    }
    let bytes = std::fs::read(cache_path).ok()?;
    let cache = serde_json::from_slice::<SkillCapabilityCache>(&bytes).ok()?;
    if cache.version != SKILL_CAPABILITY_PROBE_VERSION {
        return None;
    }
    cache
        .entries
        .into_iter()
        .find(|entry| {
            entry.probe_version == SKILL_CAPABILITY_PROBE_VERSION
                && entry.provider == provider
                && entry.binary_version == binary_version
                && entry.binary_identity.as_ref() == Some(binary_identity)
                && entry.is_consistent()
        })
        .map(|entry| SkillCapabilityMetadata {
            binary_version: entry.binary_version,
            binary_identity: entry.binary_identity,
            probe_version: entry.probe_version,
            capability: entry.capability,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use padu_protocol::composer::{SkillNativeSyntax, SkillProviderCapability};
    use std::path::PathBuf;

    fn temporary_cache_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "padu-core-skill-capability-{}-{label}.json",
            std::process::id()
        ))
    }

    fn identity(path: &str, digest_byte: char) -> SkillBinaryIdentity {
        SkillBinaryIdentity {
            canonical_path: path.into(),
            content_sha256: digest_byte.to_string().repeat(64),
        }
    }

    #[test]
    fn reads_only_matching_binary_identity_and_version_evidence() {
        let path = temporary_cache_path("matching");
        let expected_identity = identity("/opt/codex/bin/codex", 'a');
        std::fs::write(
            &path,
            serde_json::json!({
                "version": SKILL_CAPABILITY_PROBE_VERSION,
                "entries": [{
                    "provider": "codex",
                    "binaryVersion": "1.2.3",
                    "binaryIdentity": expected_identity.clone(),
                    "probeVersion": SKILL_CAPABILITY_PROBE_VERSION,
                    "observations": {
                        "leadingInvocation": true,
                        "inlineMention": true,
                        "repeatedSameMention": true,
                        "repeatedDifferentMentions": true
                    },
                    "capability": {
                        "nativeSyntax": "dollar",
                        "leadingInvocation": true,
                        "inlineMentions": true,
                        "repeatedMentions": true
                    }
                }]
            })
            .to_string(),
        )
        .unwrap();

        let expected = SkillCapabilityMetadata {
            binary_version: "1.2.3".into(),
            binary_identity: Some(expected_identity.clone()),
            probe_version: SKILL_CAPABILITY_PROBE_VERSION,
            capability: SkillProviderCapability {
                native_syntax: Some(SkillNativeSyntax::Dollar),
                leading_invocation: true,
                inline_mentions: true,
                repeated_mentions: true,
            },
        };
        assert_eq!(
            read_skill_capability_evidence_at(
                &path,
                ProviderKind::Codex,
                Some("1.2.3"),
                Some(&expected_identity)
            ),
            Some(expected)
        );
        assert_eq!(
            read_skill_capability_evidence_at(
                &path,
                ProviderKind::Codex,
                Some("1.2.4"),
                Some(&expected_identity)
            ),
            None
        );
        assert_eq!(
            read_skill_capability_evidence_at(
                &path,
                ProviderKind::Pi,
                Some("1.2.3"),
                Some(&expected_identity)
            ),
            None
        );
        let other_binary = identity("/opt/other/codex", 'a');
        assert_eq!(
            read_skill_capability_evidence_at(
                &path,
                ProviderKind::Codex,
                Some("1.2.3"),
                Some(&other_binary)
            ),
            None
        );
        assert_eq!(
            read_skill_capability_evidence_at(&path, ProviderKind::Codex, Some("1.2.3"), None),
            None
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn malformed_or_missing_evidence_falls_back() {
        let path = temporary_cache_path("malformed");
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(
            read_skill_capability_evidence_at(
                &path,
                ProviderKind::Codex,
                Some("1.2.3"),
                Some(&identity("/opt/codex/bin/codex", 'b'))
            ),
            None
        );
        let _ = std::fs::remove_file(path);
        assert_eq!(
            read_skill_capability_evidence_at(
                &temporary_cache_path("missing"),
                ProviderKind::Codex,
                Some("1.2.3"),
                Some(&identity("/opt/codex/bin/codex", 'b'))
            ),
            None
        );
    }

    #[test]
    fn same_version_at_a_different_override_or_changed_content_is_rejected() {
        let path = temporary_cache_path("identity-change");
        let first = identity("/opt/codex-a", 'a');
        let second = identity("/opt/codex-b", 'a');
        let changed_content = identity("/opt/codex-a", 'c');
        std::fs::write(
            &path,
            serde_json::json!({
                "version": SKILL_CAPABILITY_PROBE_VERSION,
                "entries": [{
                    "provider": "codex",
                    "binaryVersion": "1.2.3",
                    "binaryIdentity": first,
                    "probeVersion": SKILL_CAPABILITY_PROBE_VERSION,
                    "observations": {
                        "leadingInvocation": true,
                        "inlineMention": true,
                        "repeatedSameMention": true,
                        "repeatedDifferentMentions": true
                    },
                    "capability": {
                        "nativeSyntax": "dollar",
                        "leadingInvocation": true,
                        "inlineMentions": true,
                        "repeatedMentions": true
                    }
                }]
            })
            .to_string(),
        )
        .unwrap();

        assert!(
            read_skill_capability_evidence_at(
                &path,
                ProviderKind::Codex,
                Some("1.2.3"),
                Some(&second)
            )
            .is_none()
        );
        assert!(
            read_skill_capability_evidence_at(
                &path,
                ProviderKind::Codex,
                Some("1.2.3"),
                Some(&changed_content)
            )
            .is_none()
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn inconsistent_cached_capabilities_are_rejected() {
        let path = temporary_cache_path("inconsistent");
        let binary_identity = identity("/opt/codex/bin/codex", 'a');
        let entry = |capability: serde_json::Value| {
            serde_json::json!({
                "version": SKILL_CAPABILITY_PROBE_VERSION,
                "entries": [{
                    "provider": "codex",
                    "binaryVersion": "1.2.3",
                    "binaryIdentity": binary_identity.clone(),
                    "probeVersion": SKILL_CAPABILITY_PROBE_VERSION,
                    "observations": {
                        "leadingInvocation": false,
                        "inlineMention": false,
                        "repeatedSameMention": false,
                        "repeatedDifferentMentions": false
                    },
                    "capability": capability
                }]
            })
            .to_string()
        };

        std::fs::write(
            &path,
            entry(serde_json::json!({
                "nativeSyntax": "dollar",
                "leadingInvocation": true,
                "inlineMentions": false,
                "repeatedMentions": false
            })),
        )
        .unwrap();
        assert!(
            read_skill_capability_evidence_at(
                &path,
                ProviderKind::Codex,
                Some("1.2.3"),
                Some(&binary_identity)
            )
            .is_none()
        );

        std::fs::write(
            &path,
            entry(serde_json::json!({
                "nativeSyntax": "skillSlash",
                "leadingInvocation": false,
                "inlineMentions": false,
                "repeatedMentions": false
            })),
        )
        .unwrap();
        assert!(
            read_skill_capability_evidence_at(
                &path,
                ProviderKind::Codex,
                Some("1.2.3"),
                Some(&binary_identity)
            )
            .is_none()
        );
        let _ = std::fs::remove_file(path);
    }
}
