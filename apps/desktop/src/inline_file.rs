use std::ops::Range;

const CHIP_OUTER_SPACE: &str = " ";
// Reserve room for the icon plus a visible gap before the basename. The icon is
// painted over the first em-space; the thin space keeps the label from touching it.
const CHIP_ICON_SPACE: &str = "\u{2003}\u{2009}";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InlineFileReference {
    pub source_range: Range<usize>,
    pub target: String,
    pub basename: String,
    pub is_dir: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectedInlineFile {
    pub source_range: Range<usize>,
    pub display_range: Range<usize>,
    pub chip_range: Range<usize>,
    pub label_range: Range<usize>,
    pub target: String,
    pub basename: String,
    pub is_dir: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InlineFileProjection {
    pub display: String,
    pub files: Vec<ProjectedInlineFile>,
    source_len: usize,
}

impl InlineFileProjection {
    pub(crate) fn plain(source: &str) -> Self {
        Self {
            display: source.to_owned(),
            files: Vec::new(),
            source_len: source.len(),
        }
    }

    pub(crate) fn new(source: &str) -> Self {
        let references = parse_inline_file_references(source);
        let mut display = String::with_capacity(source.len());
        let mut files = Vec::with_capacity(references.len());
        let mut source_cursor = 0;

        for reference in references {
            display.push_str(&source[source_cursor..reference.source_range.start]);
            let display_start = display.len();
            display.push_str(CHIP_OUTER_SPACE);
            let chip_start = display.len();
            display.push_str(CHIP_ICON_SPACE);
            let label_start = display.len();
            display.push_str(&reference.basename);
            let label_end = display.len();
            let chip_end = display.len();
            display.push_str(CHIP_OUTER_SPACE);
            let display_end = display.len();
            files.push(ProjectedInlineFile {
                source_range: reference.source_range.clone(),
                display_range: display_start..display_end,
                chip_range: chip_start..chip_end,
                label_range: label_start..label_end,
                target: reference.target,
                basename: reference.basename,
                is_dir: reference.is_dir,
            });
            source_cursor = reference.source_range.end;
        }
        display.push_str(&source[source_cursor..]);

        Self {
            display,
            files,
            source_len: source.len(),
        }
    }

    pub(crate) fn source_to_display(&self, offset: usize) -> usize {
        let offset = offset.min(self.source_len);
        let mut source_cursor = 0;
        let mut display_cursor = 0;
        for file in &self.files {
            if offset <= file.source_range.start {
                return display_cursor + offset.saturating_sub(source_cursor);
            }
            if offset < file.source_range.end {
                let source_mid = file.source_range.start + file.source_range.len() / 2;
                return if offset <= source_mid {
                    file.display_range.start
                } else {
                    file.display_range.end
                };
            }
            display_cursor = file.display_range.end;
            source_cursor = file.source_range.end;
            if offset == source_cursor {
                return display_cursor;
            }
        }
        display_cursor + offset.saturating_sub(source_cursor)
    }

    pub(crate) fn display_to_source(&self, offset: usize) -> usize {
        let offset = offset.min(self.display.len());
        let mut source_cursor = 0;
        let mut display_cursor = 0;
        for file in &self.files {
            if offset <= file.display_range.start {
                return source_cursor + offset.saturating_sub(display_cursor);
            }
            if offset < file.display_range.end {
                let display_mid = file.display_range.start + file.display_range.len() / 2;
                return if offset <= display_mid {
                    file.source_range.start
                } else {
                    file.source_range.end
                };
            }
            source_cursor = file.source_range.end;
            display_cursor = file.display_range.end;
            if offset == display_cursor {
                return source_cursor;
            }
        }
        source_cursor + offset.saturating_sub(display_cursor)
    }

    pub(crate) fn source_range_to_display(&self, range: &Range<usize>) -> Range<usize> {
        self.source_to_display(range.start)..self.source_to_display(range.end)
    }

    pub(crate) fn file_at_display_offset(&self, offset: usize) -> Option<&ProjectedInlineFile> {
        self.files
            .iter()
            .find(|file| file.chip_range.contains(&offset))
    }
}

pub(crate) fn markdown_file_reference(target: &str) -> String {
    let normalized = target.replace('\\', "/");
    let basename = target_basename(&normalized);
    let label = escape_markdown_label(&basename);
    let destination = escape_markdown_destination(&normalized);
    format!("[{label}]({destination})")
}

pub(crate) fn inline_reference_insertion(content: &str, cursor: usize, target: &str) -> String {
    let cursor = cursor.min(content.len());
    let mut insertion = String::new();
    if cursor > 0 && !content[..cursor].ends_with(char::is_whitespace) {
        insertion.push(' ');
    }
    insertion.push_str(&markdown_file_reference(target));
    if cursor < content.len() && !content[cursor..].starts_with(char::is_whitespace) {
        insertion.push(' ');
    }
    insertion
}

pub(crate) fn file_backspace_range(source: &str, cursor: usize) -> Option<Range<usize>> {
    parse_inline_file_references(source)
        .into_iter()
        .find_map(|file| {
            if cursor == file.source_range.end {
                Some(file.source_range)
            } else if cursor == file.source_range.end + 1
                && source.as_bytes().get(file.source_range.end) == Some(&b' ')
            {
                Some(file.source_range.start..cursor)
            } else if cursor > file.source_range.start && cursor < file.source_range.end {
                Some(file.source_range)
            } else {
                None
            }
        })
}

pub(crate) fn file_delete_range(source: &str, cursor: usize) -> Option<Range<usize>> {
    parse_inline_file_references(source)
        .into_iter()
        .find(|file| cursor == file.source_range.start)
        .map(|file| {
            let end = if source.as_bytes().get(file.source_range.end) == Some(&b' ') {
                file.source_range.end + 1
            } else {
                file.source_range.end
            };
            file.source_range.start..end
        })
}

pub(crate) fn parse_inline_file_references(source: &str) -> Vec<InlineFileReference> {
    let bytes = source.as_bytes();
    let mut references = Vec::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        let Some(open_rel) = source[cursor..].find('[') else {
            break;
        };
        let start = cursor + open_rel;
        if start > 0 && bytes[start - 1] == b'!' {
            cursor = start + 1;
            continue;
        }
        let Some(label_end) = find_unescaped_byte(source, start + 1, b']') else {
            break;
        };
        if bytes.get(label_end + 1) != Some(&b'(') {
            cursor = start + 1;
            continue;
        }
        let target_start = label_end + 2;
        let Some((target_end, end)) = markdown_destination_end(source, target_start) else {
            cursor = start + 1;
            continue;
        };
        let raw_target = &source[target_start..target_end];
        let raw_target = raw_target
            .strip_prefix('<')
            .and_then(|target| target.strip_suffix('>'))
            .unwrap_or(raw_target);
        let target = display_target(raw_target);
        if is_local_file_target(&target) {
            let is_dir = target.ends_with(['/', '\\']);
            references.push(InlineFileReference {
                source_range: start..end,
                basename: target_basename(&target),
                target,
                is_dir,
            });
            cursor = end;
        } else {
            cursor = start + 1;
        }
    }

    references
}

pub(crate) fn is_local_file_target(target: &str) -> bool {
    let target = target.trim();
    let has_uri_scheme = target.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme.chars().enumerate().all(|(index, ch)| {
                if index == 0 {
                    ch.is_ascii_alphabetic()
                } else {
                    ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.')
                }
            })
    });
    let is_windows_drive_path = target.as_bytes().get(1) == Some(&b':')
        && matches!(target.as_bytes().get(2), Some(b'/') | Some(b'\\'));
    !target.is_empty() && !target.starts_with('#') && (!has_uri_scheme || is_windows_drive_path)
}

pub(crate) fn display_target(target: &str) -> String {
    percent_decode(&unescape_markdown(target))
}

pub(crate) fn target_basename(target: &str) -> String {
    let decoded = display_target(target);
    let trimmed = decoded.trim().trim_end_matches(['/', '\\']);
    let trimmed = strip_line_location(trimmed);
    trimmed
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(trimmed)
        .to_owned()
}

fn strip_line_location(target: &str) -> &str {
    if let Some((path, fragment)) = target.rsplit_once('#')
        && (fragment.strip_prefix('L').unwrap_or(fragment))
            .split("-L")
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
    {
        return path;
    }
    if let Some((path, line)) = target.rsplit_once(':')
        && !line.is_empty()
        && line.chars().all(|ch| ch.is_ascii_digit())
    {
        return path;
    }
    target
}

fn escape_markdown_label(label: &str) -> String {
    label
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn escape_markdown_destination(target: &str) -> String {
    target
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace(' ', "%20")
}

fn find_unescaped_byte(source: &str, mut cursor: usize, needle: u8) -> Option<usize> {
    let bytes = source.as_bytes();
    while cursor < bytes.len() {
        if bytes[cursor] == b'\\' {
            cursor += 2;
            continue;
        }
        if bytes[cursor] == needle {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

fn markdown_destination_end(source: &str, start: usize) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    let mut depth = 0;
    let angle = bytes.get(start) == Some(&b'<');
    while cursor < bytes.len() {
        if bytes[cursor] == b'\\' {
            cursor += 2;
            continue;
        }
        if angle && bytes[cursor] == b'>' && bytes.get(cursor + 1) == Some(&b')') {
            return Some((cursor + 1, cursor + 2));
        }
        match bytes[cursor] {
            b'(' if !angle => depth += 1,
            b')' if !angle && depth == 0 => return Some((cursor, cursor + 1)),
            b')' if !angle => depth -= 1,
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn unescape_markdown(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(index + 1).copied().and_then(hex_value),
                bytes.get(index + 2).copied().and_then(hex_value),
            )
        {
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).unwrap_or_else(|_| value.to_owned())
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_basename_and_preserves_full_target() {
        assert_eq!(
            markdown_file_reference("apps/desktop/src/app/right_panel/files.rs"),
            "[files.rs](apps/desktop/src/app/right_panel/files.rs)"
        );
        assert_eq!(
            markdown_file_reference("docs/a file (old).md"),
            "[a file (old).md](docs/a%20file%20\\(old\\).md)"
        );
        assert_eq!(
            markdown_file_reference("src/widgets/"),
            "[widgets](src/widgets/)"
        );
    }

    #[test]
    fn parses_only_local_markdown_links_and_uses_target_basename() {
        let source =
            "see [wrong](src/right.rs) [web](https://example.com) ![img](shot.png) [dir](src/ui/)";
        let parsed = parse_inline_file_references(source);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].basename, "right.rs");
        assert_eq!(parsed[0].target, "src/right.rs");
        assert!(!parsed[0].is_dir);
        assert_eq!(parsed[1].basename, "ui");
        assert!(parsed[1].is_dir);
    }

    #[test]
    fn parses_escaped_destinations_and_line_locations() {
        let source = r"[old](docs/a%20file%20\(old\).md#L12) and [rust](src/main.rs:42)";
        let parsed = parse_inline_file_references(source);
        assert_eq!(parsed[0].target, "docs/a file (old).md#L12");
        assert_eq!(parsed[0].basename, "a file (old).md");
        assert_eq!(parsed[1].basename, "main.rs");
    }

    #[test]
    fn rejects_uri_schemes_but_accepts_windows_drive_paths() {
        assert!(!is_local_file_target("javascript:alert(1)"));
        assert!(!is_local_file_target("data:text/plain,x"));
        assert!(!is_local_file_target("VBScript:msgbox(1)"));
        assert!(is_local_file_target("C:/src/app.ts"));
    }

    #[test]
    fn insertion_adds_only_needed_outer_whitespace() {
        assert_eq!(
            inline_reference_insertion("hello", 5, "src/a.rs"),
            " [a.rs](src/a.rs)"
        );
        assert_eq!(
            inline_reference_insertion("hello world", 5, "src/a.rs"),
            " [a.rs](src/a.rs)"
        );
        assert_eq!(
            inline_reference_insertion("hello world", 6, "src/a.rs"),
            "[a.rs](src/a.rs) "
        );
    }

    #[test]
    fn projection_maps_each_chip_atomically_in_both_directions() {
        let source = "before [a.rs](src/a.rs) after";
        let projection = InlineFileProjection::new(source);
        let file = &projection.files[0];
        assert_eq!(&projection.display[file.label_range.clone()], "a.rs");
        assert_eq!(
            projection.source_to_display(file.source_range.start),
            file.display_range.start
        );
        assert_eq!(
            projection.source_to_display(file.source_range.end),
            file.display_range.end
        );
        assert_eq!(
            projection.display_to_source(file.display_range.start),
            file.source_range.start
        );
        assert_eq!(
            projection.display_to_source(file.display_range.end),
            file.source_range.end
        );
        for offset in file.chip_range.clone() {
            assert!(matches!(
                projection.display_to_source(offset),
                value if value == file.source_range.start || value == file.source_range.end
            ));
        }
    }

    #[test]
    fn deletion_ranges_remove_a_whole_chip_and_its_trailing_separator() {
        let source = "before [a.rs](src/a.rs) after";
        let file = &parse_inline_file_references(source)[0];
        assert_eq!(
            file_backspace_range(source, file.source_range.end + 1),
            Some(file.source_range.start..file.source_range.end + 1)
        );
        assert_eq!(
            file_delete_range(source, file.source_range.start),
            Some(file.source_range.start..file.source_range.end + 1)
        );
        assert_eq!(file_delete_range(source, 0), None);
    }

    #[test]
    fn projection_keeps_plain_text_and_multiple_file_order() {
        let source = "[a](one/a.rs) + [b](two/b.rs)";
        let projection = InlineFileProjection::new(source);
        assert_eq!(projection.files.len(), 2);
        assert!(projection.display.contains("a.rs"));
        assert!(projection.display.contains(" + "));
        assert!(projection.display.contains("b.rs"));
        assert_eq!(
            projection.display_to_source(projection.display.len()),
            source.len()
        );
    }
}
