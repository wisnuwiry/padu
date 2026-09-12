use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TranscriptLinkRoute {
    ProjectFile(String),
    Finder(PathBuf),
    External,
}

pub(crate) fn positive_number(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<usize>().is_ok_and(|value| value > 0)
}

pub(crate) fn line_fragment(fragment: &str) -> bool {
    let Some(location) = fragment.strip_prefix('L') else {
        return false;
    };
    match location.split_once('C') {
        Some((line, column)) => positive_number(line) && positive_number(column),
        None => positive_number(location),
    }
}

/// Removes the `:line`, `:line:column`, or `#LlineCcolumn` suffixes Codex uses
/// in clickable local-file references. The location is not yet consumed by
/// Padu's compact editor, but it must not become part of the filesystem path.
pub(crate) fn strip_file_location(target: &str) -> &str {
    if let Some((path, fragment)) = target.rsplit_once('#')
        && line_fragment(fragment)
    {
        return path;
    }

    let Some((before_last, last)) = target.rsplit_once(':') else {
        return target;
    };
    if !positive_number(last) {
        return target;
    }
    if let Some((path, line)) = before_last.rsplit_once(':')
        && positive_number(line)
    {
        path
    } else {
        before_last
    }
}

pub(crate) fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(crate) fn percent_decode_file_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(index + 1).copied().and_then(hex_value),
                bytes.get(index + 2).copied().and_then(hex_value),
            )
        {
            decoded.push(high << 4 | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).unwrap_or_else(|_| path.to_owned())
}

pub(crate) fn markdown_file_link_path(target: &str) -> Option<PathBuf> {
    let target = strip_file_location(target.trim());
    if target
        .get(..5)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("file:"))
    {
        return url::Url::parse(target).ok()?.to_file_path().ok();
    }

    let path = PathBuf::from(percent_decode_file_path(target));
    path.is_absolute().then_some(path)
}

pub(crate) fn normalized_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

pub(crate) fn workspace_relative_file_path(workspace: &Path, target: &Path) -> Option<String> {
    fn relative(workspace: &Path, target: &Path) -> Option<String> {
        let relative = target.strip_prefix(workspace).ok()?;
        if relative.as_os_str().is_empty() {
            return None;
        }
        Some(relative.to_string_lossy().into_owned())
    }

    let workspace = normalized_path(workspace);
    let target = normalized_path(target);
    // These are daemon-host paths. Routing is intentionally lexical: probing
    // the desktop filesystem would reinterpret a remote workspace locally.
    relative(&workspace, &target)
}

pub(crate) fn transcript_link_route(target: &str, workspace: Option<&Path>) -> TranscriptLinkRoute {
    let target = strip_file_location(target.trim());
    let path = markdown_file_link_path(target).or_else(|| {
        let workspace = workspace?;
        let decoded = percent_decode_file_path(target);
        let path = Path::new(&decoded);
        (!path.is_absolute()
            && !decoded.is_empty()
            && !target.contains("://")
            && !target
                .get(..7)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("mailto:")))
        .then(|| workspace.join(path))
    });
    let Some(path) = path else {
        return TranscriptLinkRoute::External;
    };
    let path = normalized_path(&path);
    if let Some(relative_path) =
        workspace.and_then(|workspace| workspace_relative_file_path(workspace, &path))
    {
        TranscriptLinkRoute::ProjectFile(relative_path)
    } else if workspace.is_some()
        && !target.starts_with('/')
        && !target.starts_with("file:")
        && !target.contains("://")
    {
        // A relative Markdown reference must stay inside the active workspace;
        // never reinterpret an escaping relative target as a Finder path.
        TranscriptLinkRoute::External
    } else {
        TranscriptLinkRoute::Finder(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_markdown_links_route_to_workspace_files() {
        assert_eq!(
            transcript_link_route(
                "apps/desktop/src/app/right_panel/files.rs",
                Some(Path::new("/work/repo")),
            ),
            TranscriptLinkRoute::ProjectFile(
                "apps/desktop/src/app/right_panel/files.rs".to_owned()
            )
        );
    }

    #[test]
    fn relative_links_cannot_escape_the_workspace() {
        assert_eq!(
            transcript_link_route("../outside.rs", Some(Path::new("/work/repo"))),
            TranscriptLinkRoute::External
        );
    }
}
