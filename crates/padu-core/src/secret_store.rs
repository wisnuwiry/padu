//! OS keychain token store (P0-04).
//!
//! GitHub/GitLab tokens for the Phase 2 auth chain live in the OS
//! credential store and never in the database or logs: macOS Keychain,
//! Windows Credential Manager, Linux Secret Service (`secret-tool`).
//!
//! [`SecretStore`] is the abstraction; [`OsSecretStore`] dispatches to the
//! per-OS helper and [`MemorySecretStore`] backs tests. All helpers spawn
//! subprocesses, so callers must run them off the UI thread (daemon
//! background executor / blocking task) — never from `render`.
//!
//! No new dependencies: the backends shell out to OS helpers, following
//! the existing `/usr/bin/security` precedent.

use std::collections::HashMap;
use std::fmt;
use std::io::Write;
use std::process::{Command, Output, Stdio};

/// Failure modes for credential-store access.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecretStoreError {
    /// The OS helper is missing or the platform has no backend wired in.
    Unavailable(String),
    /// The helper ran but reported a failure.
    Backend(String),
}

impl fmt::Display for SecretStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(f, "credential store unavailable: {reason}"),
            Self::Backend(reason) => write!(f, "credential store error: {reason}"),
        }
    }
}

impl std::error::Error for SecretStoreError {}

/// Keychain-style credential storage keyed by `(service, account)`.
///
/// `delete` of a missing item succeeds: teardown paths stay idempotent.
pub trait SecretStore: Send + Sync {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, SecretStoreError>;
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), SecretStoreError>;
    fn delete(&self, service: &str, account: &str) -> Result<(), SecretStoreError>;
}

/// In-memory store for tests and for platforms without an OS backend.
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    items: std::sync::Mutex<HashMap<(String, String), String>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, SecretStoreError> {
        Ok(self
            .items
            .lock()
            .expect("secret store lock")
            .get(&(service.to_owned(), account.to_owned()))
            .cloned())
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), SecretStoreError> {
        self.items
            .lock()
            .expect("secret store lock")
            .insert((service.to_owned(), account.to_owned()), secret.to_owned());
        Ok(())
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), SecretStoreError> {
        self.items
            .lock()
            .expect("secret store lock")
            .remove(&(service.to_owned(), account.to_owned()));
        Ok(())
    }
}

/// OS-backed store: macOS Keychain / Windows Credential Manager / Linux
/// Secret Service, selected at compile time.
#[derive(Debug, Default)]
pub struct OsSecretStore;

impl OsSecretStore {
    pub fn new() -> Self {
        Self
    }
}

impl SecretStore for OsSecretStore {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, SecretStoreError> {
        os_get(service, account)
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), SecretStoreError> {
        os_set(service, account, secret)
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), SecretStoreError> {
        os_delete(service, account)
    }
}

/// Run an OS credential helper. A missing binary maps to [`SecretStoreError::Unavailable`];
/// anything else the caller interprets from the exit status.
fn run_helper(
    program: &str,
    args: &[&str],
    stdin_bytes: Option<&[u8]>,
    envs: &[(&str, &str)],
) -> Result<Output, SecretStoreError> {
    let mut command = Command::new(program);
    command.args(args).envs(envs.iter().copied());
    // Pipe both streams: inheriting stdout would print a retrieved credential
    // to the daemon's own stdout while `wait_with_output` returns empty, and
    // inheriting stderr would break the miss-vs-error classification below.
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if stdin_bytes.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            SecretStoreError::Unavailable(format!("{program} is not installed"))
        } else {
            SecretStoreError::Backend(format!("could not run {program}: {error}"))
        }
    })?;
    if let Some(stdin_bytes) = stdin_bytes {
        child
            .stdin
            .as_mut()
            .expect("piped stdin")
            .write_all(stdin_bytes)
            .map_err(|error| SecretStoreError::Backend(format!("{program} stdin: {error}")))?;
    }
    child.wait_with_output().map_err(|error| {
        SecretStoreError::Backend(format!("could not read {program} output: {error}"))
    })
}

#[cfg(target_os = "macos")]
fn os_get(service: &str, account: &str) -> Result<Option<String>, SecretStoreError> {
    let output = run_helper(
        "/usr/bin/security",
        &["find-generic-password", "-s", service, "-a", account, "-w"],
        None,
        &[],
    )?;
    if output.status.success() {
        return Ok(Some(
            String::from_utf8_lossy(&output.stdout)
                .trim_end()
                .to_owned(),
        ));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("could not be found") {
        Ok(None)
    } else {
        Err(SecretStoreError::Backend(format!(
            "security find-generic-password failed: {}",
            stderr.trim()
        )))
    }
}

#[cfg(target_os = "macos")]
fn os_set(service: &str, account: &str, secret: &str) -> Result<(), SecretStoreError> {
    // NOTE: `security add-generic-password` takes the secret as a `-w`
    // argument — there is no stdin path — so the value is briefly visible to
    // same-user process inspection while the helper runs. The daemon never
    // logs it. The proper fix is a Keychain Services binding instead of the
    // CLI; tracked as future work on this abstraction.
    let output = run_helper(
        "/usr/bin/security",
        &[
            "add-generic-password",
            "-s",
            service,
            "-a",
            account,
            "-w",
            secret,
            "-U",
        ],
        None,
        &[],
    )?;
    if output.status.success() {
        Ok(())
    } else {
        Err(SecretStoreError::Backend(format!(
            "security add-generic-password failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

#[cfg(target_os = "macos")]
fn os_delete(service: &str, account: &str) -> Result<(), SecretStoreError> {
    let output = run_helper(
        "/usr/bin/security",
        &["delete-generic-password", "-s", service, "-a", account],
        None,
        &[],
    )?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("could not be found") {
        Ok(())
    } else {
        Err(SecretStoreError::Backend(format!(
            "security delete-generic-password failed: {}",
            stderr.trim()
        )))
    }
}

#[cfg(target_os = "linux")]
fn os_get(service: &str, account: &str) -> Result<Option<String>, SecretStoreError> {
    let output = run_helper(
        "secret-tool",
        &["lookup", "service", service, "account", account],
        None,
        &[],
    )?;
    linux_lookup_outcome(&output)
}

/// Classify a `secret-tool lookup` result: `secret-tool` exits nonzero with
/// no output when nothing matches, while a real backend failure reports on
/// stderr — so only empty stderr reads as a miss.
#[cfg(target_os = "linux")]
fn linux_lookup_outcome(output: &Output) -> Result<Option<String>, SecretStoreError> {
    if output.status.success() {
        return Ok(Some(
            String::from_utf8_lossy(&output.stdout)
                .trim_end()
                .to_owned(),
        ));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.trim().is_empty() {
        Ok(None)
    } else {
        Err(SecretStoreError::Backend(format!(
            "secret-tool lookup failed: {}",
            stderr.trim()
        )))
    }
}

#[cfg(target_os = "linux")]
fn os_set(service: &str, account: &str, secret: &str) -> Result<(), SecretStoreError> {
    let label = format!("Padu {service}/{account}");
    let output = run_helper(
        "secret-tool",
        &[
            "store", "--label", &label, "service", service, "account", account,
        ],
        Some(secret.as_bytes()),
        &[],
    )?;
    if output.status.success() {
        Ok(())
    } else {
        Err(SecretStoreError::Backend(format!(
            "secret-tool store failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

#[cfg(target_os = "linux")]
fn os_delete(service: &str, account: &str) -> Result<(), SecretStoreError> {
    let output = run_helper(
        "secret-tool",
        &["clear", "service", service, "account", account],
        None,
        &[],
    )?;
    if output.status.success() {
        Ok(())
    } else {
        Err(SecretStoreError::Backend(format!(
            "secret-tool clear failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

/// Windows Credential Manager via the WinRT `PasswordVault`, driven through
/// PowerShell. Values travel in environment variables and the script itself
/// is passed as `-EncodedCommand` (UTF-16LE base64), so secrets never
/// appear in a command line — mirroring the `curl -K -` hygiene used for
/// bearer tokens elsewhere.
#[cfg(target_os = "windows")]
fn powershell_vault(script: &str, envs: &[(&str, &str)]) -> Result<Output, SecretStoreError> {
    use base64::Engine as _;
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
    run_helper(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded],
        None,
        envs,
    )
}

#[cfg(target_os = "windows")]
fn os_get(service: &str, account: &str) -> Result<Option<String>, SecretStoreError> {
    let output = powershell_vault(
        "$v = New-Object Windows.Security.Credentials.PasswordVault; \
         try { $c = $v.Retrieve($env:PADU_SS_SERVICE, $env:PADU_SS_ACCOUNT); \
         $c.RetrievePassword(); [Console]::Out.Write($c.Password) } \
         catch { exit 3 }",
        &[("PADU_SS_SERVICE", service), ("PADU_SS_ACCOUNT", account)],
    )?;
    if output.status.success() {
        return Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()));
    }
    if output.status.code() == Some(3) {
        Ok(None)
    } else {
        Err(SecretStoreError::Backend(format!(
            "PasswordVault retrieve failed with status {}",
            output.status
        )))
    }
}

#[cfg(target_os = "windows")]
fn os_set(service: &str, account: &str, secret: &str) -> Result<(), SecretStoreError> {
    let output = powershell_vault(
        "$v = New-Object Windows.Security.Credentials.PasswordVault; \
         try { $v.Remove($v.Retrieve($env:PADU_SS_SERVICE, $env:PADU_SS_ACCOUNT)) } catch {}; \
         $v.Add((New-Object Windows.Security.Credentials.PasswordCredential($env:PADU_SS_SERVICE, $env:PADU_SS_ACCOUNT, $env:PADU_SS_SECRET)))",
        &[
            ("PADU_SS_SERVICE", service),
            ("PADU_SS_ACCOUNT", account),
            ("PADU_SS_SECRET", secret),
        ],
    )?;
    if output.status.success() {
        Ok(())
    } else {
        Err(SecretStoreError::Backend(format!(
            "PasswordVault store failed with status {}",
            output.status
        )))
    }
}

#[cfg(target_os = "windows")]
fn os_delete(service: &str, account: &str) -> Result<(), SecretStoreError> {
    let output = powershell_vault(
        "$v = New-Object Windows.Security.Credentials.PasswordVault; \
         try { $v.Remove($v.Retrieve($env:PADU_SS_SERVICE, $env:PADU_SS_ACCOUNT)) } \
         catch { exit 3 }",
        &[("PADU_SS_SERVICE", service), ("PADU_SS_ACCOUNT", account)],
    )?;
    if output.status.success() || output.status.code() == Some(3) {
        Ok(())
    } else {
        Err(SecretStoreError::Backend(format!(
            "PasswordVault remove failed with status {}",
            output.status
        )))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn os_get(_service: &str, _account: &str) -> Result<Option<String>, SecretStoreError> {
    Err(SecretStoreError::Unavailable(
        "no credential backend on this platform".to_owned(),
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn os_set(_service: &str, _account: &str, _secret: &str) -> Result<(), SecretStoreError> {
    Err(SecretStoreError::Unavailable(
        "no credential backend on this platform".to_owned(),
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn os_delete(_service: &str, _account: &str) -> Result<(), SecretStoreError> {
    Err(SecretStoreError::Unavailable(
        "no credential backend on this platform".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips_and_overwrites() {
        let store = MemorySecretStore::new();
        assert_eq!(store.get("github", "padu").unwrap(), None);

        store.set("github", "padu", "token-one").unwrap();
        assert_eq!(
            store.get("github", "padu").unwrap().as_deref(),
            Some("token-one")
        );

        store.set("github", "padu", "token-two").unwrap();
        assert_eq!(
            store.get("github", "padu").unwrap().as_deref(),
            Some("token-two")
        );

        // Scoping is per (service, account).
        assert_eq!(store.get("gitlab", "padu").unwrap(), None);
        assert_eq!(store.get("github", "other").unwrap(), None);
    }

    #[test]
    fn memory_delete_is_idempotent() {
        let store = MemorySecretStore::new();
        store.delete("github", "nobody").unwrap();

        store.set("github", "padu", "token").unwrap();
        store.delete("github", "padu").unwrap();
        assert_eq!(store.get("github", "padu").unwrap(), None);
        store.delete("github", "padu").unwrap();
    }

    #[test]
    fn errors_describe_the_failure() {
        assert_eq!(
            SecretStoreError::Unavailable("no helper".to_owned()).to_string(),
            "credential store unavailable: no helper"
        );
        assert_eq!(
            SecretStoreError::Backend("denied".to_owned()).to_string(),
            "credential store error: denied"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_lookup_distinguishes_miss_from_backend_error() {
        use std::os::unix::process::ExitStatusExt;
        use std::process::Output;

        let outcome = |status: i32, stdout: &[u8], stderr: &[u8]| {
            linux_lookup_outcome(&Output {
                status: ExitStatusExt::from_raw(status),
                stdout: stdout.to_vec(),
                stderr: stderr.to_vec(),
            })
        };

        // exit 1 with no output: nothing matches.
        assert_eq!(outcome(256, b"", b"").unwrap(), None);
        // exit 1 with stderr: the backend itself failed.
        assert!(matches!(
            outcome(256, b"", b"secret-tool: no secret service"),
            Err(SecretStoreError::Backend(_))
        ));
        // success trims the trailing newline.
        assert_eq!(
            outcome(0, b"token\n", b"").unwrap().as_deref(),
            Some("token")
        );
    }
}
