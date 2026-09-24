//! Cross-platform resolution of external transport binaries.
//!
//! `which` only inspects the process `PATH`, which is not the `PATH` the user
//! installed into. A macOS app launched from the Dock or Finder inherits
//! launchd's minimal `PATH` (`/usr/bin:/bin:/usr/sbin:/sbin`), so a Homebrew
//! `cloudflared` in `/opt/homebrew/bin` is invisible and Padu reports it as
//! "not installed" on a machine whose terminal runs it fine. Searching the
//! platform's well-known install prefixes after `PATH` makes detection agree
//! with the shell the user installed from, without spawning a login shell.

use std::path::{Path, PathBuf};

/// Resolve `name` to an executable, preferring `PATH` and falling back to the
/// platform's well-known install directories. Returns `None` when nothing
/// executable matches.
pub fn find_binary(name: &str) -> Option<PathBuf> {
    if let Ok(path) = which::which(name) {
        return Some(path);
    }
    let file_name = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    search_dirs()
        .into_iter()
        .map(|directory| directory.join(&file_name))
        .find(|candidate| is_executable(candidate))
}

/// Directories package managers install executables into on this platform,
/// ordered most-specific first so a user-scoped install wins over a stale
/// system copy of the same name.
pub fn search_dirs() -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    directories.extend(
        // Apple-silicon Homebrew, Intel Homebrew / manual installs, MacPorts.
        [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/opt/local/bin",
            "/usr/bin",
        ]
        .map(PathBuf::from),
    );

    // Homebrew on Linux deliberately lives outside `$HOME`.
    #[cfg(all(unix, not(target_os = "macos")))]
    directories.extend(
        [
            "/usr/local/bin",
            "/usr/bin",
            "/bin",
            "/snap/bin",
            "/home/linuxbrew/.linuxbrew/bin",
        ]
        .map(PathBuf::from),
    );

    #[cfg(target_os = "windows")]
    directories.extend(windows_dirs());

    if let Some(home) = dirs::home_dir() {
        directories.push(home.join(".local").join("bin"));
        directories.push(home.join("bin"));
    }

    directories
}

#[cfg(target_os = "windows")]
fn windows_dirs() -> Vec<PathBuf> {
    let mut directories = Vec::new();

    // `winget install` puts its shim here.
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let local_app_data = PathBuf::from(local_app_data);
        directories.push(local_app_data.join("Microsoft/WinGet/Links"));
        directories.push(local_app_data.join("cloudflared"));
    }

    // The Cloudflare `.msi` installs into a 32-bit-layout program directory.
    for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(program_files) = std::env::var_os(variable) {
            directories.push(PathBuf::from(program_files).join("cloudflared"));
        }
    }

    directories.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin"));
    if let Some(chocolatey) = std::env::var_os("ChocolateyInstall") {
        directories.push(PathBuf::from(chocolatey).join("bin"));
    }

    if let Some(home) = dirs::home_dir() {
        directories.push(home.join("scoop").join("shims"));
    }

    directories
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_binary_resolves_to_none() {
        assert!(find_binary("padu-no-such-binary-4c1f").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn find_binary_uses_path_when_available() {
        // `sh` is guaranteed to be on `PATH` on every Unix the app targets.
        let found = find_binary("sh").expect("sh must resolve on PATH");
        assert!(found.is_absolute());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_search_dirs_cover_both_homebrew_prefixes() {
        // Regression: the Dock-launched app's PATH omits Homebrew, so these
        // directories must be searched even when `which` fails.
        let directories = search_dirs();
        assert!(directories.contains(&PathBuf::from("/opt/homebrew/bin")));
        assert!(directories.contains(&PathBuf::from("/usr/local/bin")));
    }

    #[test]
    fn search_dirs_are_absolute() {
        assert!(
            search_dirs()
                .iter()
                .all(|directory| directory.is_absolute())
        );
    }
}
