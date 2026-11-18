//! Daemon-only provider discovery layered over shared protocol models.

use std::io::Read;
use std::path::Path;

use padu_protocol::composer::SkillBinaryIdentity;
pub use padu_protocol::model::*;
use sha2::{Digest, Sha256};

pub fn provider_probe(provider: ProviderKind, binary_override: Option<&str>) -> ProviderProbe {
    let path = match binary_override {
        Some(binary) => crate::command_env::resolve_binary_override(binary),
        None => crate::command_env::find_executable(provider.command()),
    };
    ProviderProbe {
        provider,
        installed: path.is_some(),
        path,
        models: crate::model_catalog::fallback_models(provider),
        agent_presets: crate::model_catalog::fallback_agent_presets(provider),
    }
}

/// Detect a provider and hydrate its catalog from the daemon-owned cache.
///
/// This is the fast half of stale-while-revalidate: clients can render the
/// last successful catalog immediately, then request live discovery to replace
/// it. Cache I/O stays in the daemon instead of leaking host filesystem access
/// into desktop or Web clients.
pub fn cached_provider_probe(
    provider: ProviderKind,
    binary_override: Option<&str>,
) -> ProviderProbe {
    let cached = crate::model_catalog::cached_models(provider);
    apply_cached_models(provider_probe(provider, binary_override), cached)
}

fn apply_cached_models(
    mut probe: ProviderProbe,
    cached_models: Option<Vec<ProviderModel>>,
) -> ProviderProbe {
    if probe.provider.supports_model_discovery()
        && let Some(models) = cached_models
    {
        probe.models = models;
    }
    probe
}

pub fn discover_provider_models(mut probe: ProviderProbe) -> ProviderProbe {
    if probe.provider.supports_model_discovery()
        && let Some(path) = probe.path.as_deref()
    {
        let (models, agent_presets) = crate::model_catalog::discover_catalog(probe.provider, path);
        probe.models = models;
        probe.agent_presets = agent_presets;
    }
    probe
}

/// Run `<cli> --version` on the daemon host and extract its first version-like
/// token. Provider CLIs decorate this output differently, so clients receive a
/// normalized value rather than subprocess output.
pub fn probe_provider_version(binary: &Path) -> Option<String> {
    let mut command = crate::command_env::command(binary);
    let command = command.arg("--version").stdin(std::process::Stdio::null());
    let output = crate::command_env::output(command).ok()?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_cli_version(&combined)
}

/// Identify the exact provider executable used for capability evidence.
///
/// The path is canonicalized first, so relative paths and symlink aliases
/// resolve to the same absolute target. The content hash is SHA-256 over every
/// byte of that target. A path or digest that cannot be computed is unknown,
/// and callers must keep native skill syntax disabled.
pub fn provider_binary_identity(binary: &Path) -> Option<SkillBinaryIdentity> {
    let canonical = std::fs::canonicalize(binary).ok()?;
    if !canonical.is_file() {
        return None;
    }
    let canonical_path = canonical.to_str()?.to_owned();
    let mut file = std::fs::File::open(&canonical).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(SkillBinaryIdentity {
        canonical_path,
        content_sha256: format!("{:x}", hasher.finalize()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temporary_binary(label: &str, contents: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "padu-provider-identity-{}-{label}",
            std::process::id()
        ));
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn cached_catalog_replaces_fallback_before_live_discovery() {
        let probe = ProviderProbe {
            provider: ProviderKind::Codex,
            installed: true,
            path: Some("/usr/bin/codex".into()),
            models: crate::model_catalog::fallback_models(ProviderKind::Codex),
            agent_presets: Vec::new(),
        };
        let cached = vec![ProviderModel::new("cached-model", "Cached model").default()];

        let probe = apply_cached_models(probe, Some(cached));

        assert_eq!(probe.models.len(), 1);
        assert_eq!(probe.models[0].id, "cached-model");
    }

    #[test]
    fn binary_identity_is_canonical_path_plus_full_content_sha256() {
        let path = temporary_binary("hash", b"abc");
        let identity = provider_binary_identity(&path).expect("hash executable contents");
        assert_eq!(
            identity.content_sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            identity.canonical_path,
            std::fs::canonicalize(&path).unwrap().to_str().unwrap()
        );
        assert!(identity.is_valid());

        std::fs::write(&path, b"abcd").unwrap();
        let changed = provider_binary_identity(&path).expect("hash changed executable");
        assert_eq!(changed.canonical_path, identity.canonical_path);
        assert_ne!(changed.content_sha256, identity.content_sha256);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn identical_contents_at_different_paths_have_different_identities() {
        let first_path = temporary_binary("first", b"same executable");
        let second_path = temporary_binary("second", b"same executable");
        let first = provider_binary_identity(&first_path).expect("hash first executable");
        let second = provider_binary_identity(&second_path).expect("hash second executable");

        assert_eq!(first.content_sha256, second.content_sha256);
        assert_ne!(first.canonical_path, second.canonical_path);
        assert_ne!(first, second);

        let _ = std::fs::remove_file(first_path);
        let _ = std::fs::remove_file(second_path);
    }
}
