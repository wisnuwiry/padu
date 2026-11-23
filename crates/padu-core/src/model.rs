//! Daemon-only provider discovery layered over shared protocol models.

use std::io::Read;
use std::path::{Path, PathBuf};

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
        unavailable_reason: None,
        storage_paths: provider_storage_paths(provider),
        models: crate::model_catalog::fallback_models(provider),
        agent_presets: crate::model_catalog::fallback_agent_presets(provider),
    }
}

/// Resolve the exact storage roots Padu expects the harness to own. These are
/// read-only diagnostics for Settings; Padu never writes provider config.
pub fn provider_storage_paths(provider: ProviderKind) -> Vec<ProviderStoragePath> {
    let Some(root) = provider_xdg_roots(provider) else {
        return Vec::new();
    };
    [
        (ProviderStoragePathKind::Config, root.config),
        (ProviderStoragePathKind::Data, root.data),
        (ProviderStoragePathKind::State, root.state),
        (ProviderStoragePathKind::Cache, root.cache),
    ]
    .into_iter()
    .map(|(kind, path)| ProviderStoragePath { kind, path })
    .collect()
}

/// Environment variables passed to an OpenCode child. v2 deliberately keeps
/// the standard OpenCode roots; v1 receives Padu-owned roots so it cannot
/// share credentials, sessions, plugins, skills, or cache with v2.
pub fn provider_xdg_environment(provider: ProviderKind) -> Vec<(String, String)> {
    let Some(root) = provider_xdg_roots(provider).filter(|_| provider == ProviderKind::OpenCode)
    else {
        return Vec::new();
    };
    let environment_root =
        |path: &std::path::Path| path.parent().unwrap_or(path).to_string_lossy().into_owned();
    let mut environment = vec![
        ("XDG_CONFIG_HOME".into(), environment_root(&root.config)),
        ("XDG_DATA_HOME".into(), environment_root(&root.data)),
        ("XDG_STATE_HOME".into(), environment_root(&root.state)),
        ("XDG_CACHE_HOME".into(), environment_root(&root.cache)),
        (
            "OPENCODE_CONFIG_DIR".into(),
            root.config.to_string_lossy().into_owned(),
        ),
        (
            "OPENCODE_CONFIG".into(),
            root.config
                .join("opencode.json")
                .to_string_lossy()
                .into_owned(),
        ),
    ];
    // OpenCode's process-level overrides supersede its XDG files. When one is
    // present in the parent process, replace it with v1-isolated content so the
    // standard v2 configuration cannot leak into the legacy provider.
    if crate::command_env::environment_value("OPENCODE_CONFIG_CONTENT").is_some() {
        environment.push(("OPENCODE_CONFIG_CONTENT".into(), "{}".into()));
    }
    if crate::command_env::environment_value("OPENCODE_AUTH_CONTENT").is_some() {
        environment.push((
            "OPENCODE_AUTH_CONTENT".into(),
            std::fs::read_to_string(root.data.join("auth.json"))
                .ok()
                .filter(|content| !content.trim().is_empty())
                .unwrap_or_else(|| "{}".into()),
        ));
    }
    if crate::command_env::environment_value("OPENCODE_API_KEY").is_some() {
        environment.push(("OPENCODE_API_KEY".into(), String::new()));
    }
    environment
}

pub fn apply_provider_version_compatibility(probe: &mut ProviderProbe, version: Option<&str>) {
    if !matches!(
        probe.provider,
        ProviderKind::OpenCode | ProviderKind::OpenCodeV2
    ) {
        return;
    }
    let Some(major) = version
        .and_then(|version| version.split('.').next())
        .and_then(|major| major.parse::<u64>().ok())
    else {
        if probe.installed {
            probe.unavailable_reason =
                Some("Could not determine the installed OpenCode major version.".into());
        }
        return;
    };
    let supported = match probe.provider {
        ProviderKind::OpenCode => major == 1,
        ProviderKind::OpenCodeV2 => major >= 2,
        _ => true,
    };
    probe.unavailable_reason = (!supported).then(|| {
        format!(
            "{} requires an OpenCode {} binary; detected {}.",
            probe.provider.display_name(),
            if probe.provider == ProviderKind::OpenCode {
                "1.x"
            } else {
                "2.x"
            },
            version.unwrap_or("an unknown version")
        )
    });
}

struct ProviderXdgRoots {
    config: PathBuf,
    data: PathBuf,
    state: PathBuf,
    cache: PathBuf,
}

fn provider_xdg_roots(provider: ProviderKind) -> Option<ProviderXdgRoots> {
    match provider {
        ProviderKind::OpenCode => {
            let base = dirs::data_dir()?
                .join(crate::identity::DATA_DIRECTORY_NAME)
                .join("opencode-v1");
            Some(ProviderXdgRoots {
                config: base.join("config").join("opencode"),
                data: base.join("data").join("opencode"),
                state: base.join("state").join("opencode"),
                cache: base.join("cache").join("opencode"),
            })
        }
        ProviderKind::OpenCodeV2 => Some(ProviderXdgRoots {
            config: xdg_root("XDG_CONFIG_HOME", ".config")?.join("opencode"),
            data: xdg_root("XDG_DATA_HOME", ".local/share")?.join("opencode"),
            state: xdg_root("XDG_STATE_HOME", ".local/state")?.join("opencode"),
            cache: xdg_root("XDG_CACHE_HOME", ".cache")?.join("opencode"),
        }),
        _ => None,
    }
}

fn xdg_root(variable: &str, home_fallback: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    let _ = home_fallback;
    crate::command_env::environment_value(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            #[cfg(windows)]
            {
                match variable {
                    "XDG_CONFIG_HOME" => dirs::config_dir(),
                    "XDG_CACHE_HOME" => dirs::cache_dir(),
                    _ => dirs::data_dir(),
                }
            }
            #[cfg(not(windows))]
            {
                crate::command_env::environment_value("HOME")
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
                    .or_else(dirs::home_dir)
                    .map(|home| home.join(home_fallback))
            }
        })
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
    if probe.available()
        && probe.provider.supports_model_discovery()
        && let Some(path) = probe.path.as_deref()
    {
        let environment = provider_xdg_environment(probe.provider);
        let (models, agent_presets) =
            crate::model_catalog::discover_catalog_with_env(probe.provider, path, &environment);
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
            unavailable_reason: None,
            storage_paths: Vec::new(),
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
