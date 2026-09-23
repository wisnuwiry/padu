//! Cross-platform stand-ins for the POSIX filesystem calls Padu relies on.
//!
//! Provider isolation directories, the Computer Use install root, and the
//! bundle copies that feed it are all written in POSIX terms. Windows has no
//! mode bits and reaches symlinks through two separate calls, so the callers
//! go through these helpers instead of `std::os::unix` directly.

use std::io;
use std::path::Path;

/// Create `path` and any missing parents so only the current user can reach
/// it.
///
/// Windows has no mode bits: the locations Padu creates here live under the
/// user's own profile (`%LOCALAPPDATA%`, `%TEMP%`), which already inherits an
/// ACL granting the owner and administrators alone.
pub(crate) fn create_private_dir_all(path: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(path)
}

/// Drop group and other access on an existing directory. A no-op on Windows,
/// where the inherited profile ACL already restricts it.
pub(crate) fn restrict_to_owner(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

/// Link `link` to `original`.
///
/// Windows picks the symlink flavor from the target's kind, and creating
/// either needs Developer Mode or `SeCreateSymbolicLinkPrivilege`; callers
/// treat a failure as "this resource could not be mirrored" rather than
/// assuming POSIX semantics.
pub(crate) fn symlink(original: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(original, link)
    }
    #[cfg(windows)]
    {
        if original.is_dir() {
            std::os::windows::fs::symlink_dir(original, link)
        } else {
            std::os::windows::fs::symlink_file(original, link)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (original, link);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symlinks are not supported on this platform",
        ))
    }
}

/// Fold a Windows verbatim path back to plain form (`\\?\C:\…` → `C:\…`,
/// `\\?\UNC\server\share` → `server\share`). `canonicalize` returns the
/// verbatim form, which no slug scheme, transcript header, or display string
/// includes; a no-op everywhere else.
pub(crate) fn strip_verbatim_prefix(path: &str) -> &str {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        rest
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest
    } else {
        path
    }
}

/// Canonicalized `path` as a portable display string: verbatim prefixes
/// folded away, falling back to the path as given when it does not exist.
pub(crate) fn canonical_display_string(path: &Path) -> String {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    strip_verbatim_prefix(&resolved.to_string_lossy()).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_prefixes_fold_back_to_plain_paths() {
        assert_eq!(strip_verbatim_prefix(r"\\?\C:\Users\dev"), r"C:\Users\dev");
        assert_eq!(
            strip_verbatim_prefix(r"\\?\UNC\server\share"),
            r"server\share"
        );
        assert_eq!(strip_verbatim_prefix("/private/tmp"), "/private/tmp");
    }
}
