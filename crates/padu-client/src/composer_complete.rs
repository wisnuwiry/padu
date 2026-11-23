//! Pure client-side composer matching over daemon-provided command/file lists.

use std::fmt;
use std::ops::Range;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32Str};
pub use padu_protocol::composer::{
    CommandScope, FileEntry, SKILL_CAPABILITY_PROBE_VERSION, SkillCapabilityMetadata,
    SkillNativeSyntax, SkillProviderCapability, SlashCommand, parse_compaction_submission,
};
use padu_protocol::model::{ProviderKind, ProviderModelOption, ReportedCommand};

pub const FILTER_CAP: usize = 64;
pub const FILE_INDEX_CAP: usize = 50_000;
pub const SKILL_CATALOG_UNAVAILABLE_WARNING: &str =
    "Skill catalog is unavailable; $skill mentions were left unchanged.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriggerKind {
    Command,
    Skill,
    File,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trigger {
    pub kind: TriggerKind,
    pub query: String,
    pub range: Range<usize>,
}

#[derive(Clone, Copy)]
struct Fence {
    marker: u8,
    len: usize,
}

fn markdown_code_ranges(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut ranges = Vec::new();
    let mut position = 0;
    let mut line_start = true;
    while position < text.len() {
        if line_start && let Some((fence, after_opener)) = fence_opener(text, position) {
            let end = closing_fence_end(text, after_opener, fence).unwrap_or(text.len());
            ranges.push(position..end);
            position = end;
            line_start = end > 0 && bytes.get(end - 1) == Some(&b'\n');
            continue;
        }

        if bytes[position] == b'`' {
            let len = byte_run_len(bytes, position, b'`');
            if is_backslash_escaped(bytes, position) {
                position += len;
                line_start = false;
                continue;
            }
            let content_start = position + len;
            let end = matching_backtick_run(bytes, content_start, len)
                .map(|closing| closing + len)
                .unwrap_or(text.len());
            ranges.push(position..end);
            position = end;
            line_start = end > 0 && bytes.get(end - 1) == Some(&b'\n');
            continue;
        }

        if bytes[position] == b'\n' {
            line_start = true;
            position += 1;
        } else {
            line_start = false;
            position += text[position..]
                .chars()
                .next()
                .expect("position is a character boundary")
                .len_utf8();
        }
    }
    ranges
}

fn fence_opener(text: &str, line_start: usize) -> Option<(Fence, usize)> {
    let bytes = text.as_bytes();
    let (content_end, after_line) = line_bounds(text, line_start);
    let mut position = line_start;
    while position < content_end && position - line_start < 3 && bytes[position] == b' ' {
        position += 1;
    }
    let marker = *bytes.get(position)?;
    if !matches!(marker, b'`' | b'~') {
        return None;
    }
    let len = byte_run_len(bytes, position, marker);
    if len < 3 {
        return None;
    }
    if marker == b'`' && text[position + len..content_end].contains('`') {
        return None;
    }
    Some((Fence { marker, len }, after_line))
}

fn closing_fence_end(text: &str, mut line_start: usize, fence: Fence) -> Option<usize> {
    let bytes = text.as_bytes();
    while line_start < text.len() {
        let (content_end, after_line) = line_bounds(text, line_start);
        let mut position = line_start;
        while position < content_end && position - line_start < 3 && bytes[position] == b' ' {
            position += 1;
        }
        if bytes.get(position) == Some(&fence.marker) {
            let len = byte_run_len(bytes, position, fence.marker);
            if len >= fence.len
                && text[position + len..content_end]
                    .bytes()
                    .all(|byte| matches!(byte, b' ' | b'\t'))
            {
                return Some(after_line);
            }
        }
        line_start = after_line;
    }
    None
}

fn line_bounds(text: &str, line_start: usize) -> (usize, usize) {
    let Some(relative_newline) = text[line_start..].find('\n') else {
        return (text.len(), text.len());
    };
    let newline = line_start + relative_newline;
    let content_end = if newline > line_start && text.as_bytes()[newline - 1] == b'\r' {
        newline - 1
    } else {
        newline
    };
    (content_end, newline + 1)
}

fn matching_backtick_run(bytes: &[u8], mut position: usize, len: usize) -> Option<usize> {
    while position < bytes.len() {
        if bytes[position] == b'`' {
            let run = byte_run_len(bytes, position, b'`');
            if run == len {
                return Some(position);
            }
            position += run;
        } else {
            position += 1;
        }
    }
    None
}

fn byte_run_len(bytes: &[u8], start: usize, marker: u8) -> usize {
    bytes[start..]
        .iter()
        .take_while(|byte| **byte == marker)
        .count()
}

fn is_backslash_escaped(bytes: &[u8], position: usize) -> bool {
    let backslashes = bytes[..position]
        .iter()
        .rev()
        .take_while(|byte| **byte == b'\\')
        .count();
    backslashes % 2 == 1
}

fn is_code_position(ranges: &[Range<usize>], position: usize) -> bool {
    ranges
        .iter()
        .any(|range| range.start <= position && position < range.end)
}

pub fn detect_trigger(text: &str, cursor: usize) -> Option<Trigger> {
    let cursor = cursor.min(text.len());
    if !text.is_char_boundary(cursor) {
        return None;
    }
    let line_start = text[..cursor].rfind('\n').map_or(0, |index| index + 1);
    let line_prefix = &text[line_start..cursor];
    if let Some(query) = line_prefix.strip_prefix('/') {
        if !query.chars().any(char::is_whitespace) {
            return Some(Trigger {
                kind: TriggerKind::Command,
                query: query.to_owned(),
                range: line_start..cursor,
            });
        }
    }
    let mut token_start = text[..cursor]
        .rfind(char::is_whitespace)
        .map_or(0, |index| {
            index + text[index..].chars().next().unwrap().len_utf8()
        });
    let code_ranges = markdown_code_ranges(&text[..cursor]);
    if let Some(range_end) = code_ranges
        .iter()
        .map(|range| range.end)
        .filter(|end| *end >= token_start && *end <= cursor)
        .max()
        .filter(|end| text.as_bytes().get(*end) == Some(&b'$'))
    {
        token_start = range_end;
    }
    let token = &text[token_start..cursor];
    if let Some(query) = token.strip_prefix('$') {
        if !is_code_position(&code_ranges, token_start) && !is_code_position(&code_ranges, cursor) {
            return Some(Trigger {
                kind: TriggerKind::Skill,
                query: query.to_owned(),
                range: token_start..cursor,
            });
        }
        return None;
    }
    Some(Trigger {
        kind: TriggerKind::File,
        query: token.strip_prefix('@')?.to_owned(),
        range: token_start..cursor,
    })
}

pub fn merge_reported_commands(
    discovered: &[SlashCommand],
    reported: &[ReportedCommand],
) -> Vec<SlashCommand> {
    let mut merged = discovered.to_vec();
    for report in reported {
        if let Some(known) = merged
            .iter_mut()
            .find(|command| command.name == report.name)
        {
            if known.description.is_empty() {
                known.description = report.description.clone();
            }
        } else {
            merged.push(SlashCommand {
                name: report.name.clone(),
                description: report.description.clone(),
                scope: CommandScope::Builtin,
                argument_hint: None,
                template: None,
                skill_file: None,
            });
        }
    }
    merged
        .sort_by(|a, b| (a.scope.display_rank(), &a.name).cmp(&(b.scope.display_rank(), &b.name)));
    merged
}

/// Build the slash-prefixed text shown for an autocomplete command.
pub fn command_composer_text(command: &SlashCommand) -> String {
    format!("/{}", command.name)
}

/// Build the dollar-prefixed text inserted from a skill autocomplete row.
pub fn skill_composer_text(command: &SlashCommand) -> String {
    format!("${}", command.name)
}

/// Whether the composer submitted Padu's global terminal-session picker.
/// The command is reserved by daemon-side discovery, so it is intentionally
/// provider-neutral and never crosses into a provider transport.
pub fn is_resume_submission(prompt: &str) -> bool {
    prompt.trim() == "/resume"
}

/// Whether the submitted text resolves to Codex's native fast-mode command,
/// which Padu bridges to the provider's service-tier control. Checking the
/// resolved entry preserves project/user command precedence when one of them
/// intentionally owns `/fast`.
pub fn is_fast_mode_toggle_submission(
    provider: ProviderKind,
    prompt: &str,
    commands: &[SlashCommand],
) -> bool {
    provider == ProviderKind::Codex
        && prompt.trim() == "/fast"
        && commands.iter().any(|command| {
            command.name == "fast"
                && command.scope == CommandScope::Builtin
                && command.template.is_none()
        })
}

/// Resolve the next concrete service-tier ID for Codex's Fast toggle. Model
/// metadata may expose the Fast tier as `fast` or as `priority`; the display
/// label is the stable product vocabulary, while the ID is provider-owned.
pub fn toggled_fast_service_tier(
    current: Option<&str>,
    service_tiers: &[ProviderModelOption],
) -> Option<String> {
    let fast = service_tiers.iter().find(|tier| {
        matches!(tier.id.as_str(), "fast" | "priority") || tier.label.eq_ignore_ascii_case("fast")
    })?;
    Some(if current == Some(fast.id.as_str()) {
        "default".to_owned()
    } else {
        fast.id.clone()
    })
}

/// A `/goal` composer submission parsed into its intent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GoalCommand {
    /// Bare `/goal` — show the current goal (and offer to create one).
    Show,
    Edit,
    Pause,
    Resume,
    Clear,
    /// `/goal <objective>` — start or replace the goal with this objective.
    Set(String),
}

/// Parse the submitted text as Codex's native `/goal` command, which Padu
/// bridges to `thread/goal/*`. `None` when it is not one — wrong provider,
/// other text, or a project/user command that deliberately owns `/goal`
/// (resolution precedence stands).
pub fn parse_goal_submission(
    provider: ProviderKind,
    prompt: &str,
    commands: &[SlashCommand],
) -> Option<GoalCommand> {
    if provider != ProviderKind::Codex {
        return None;
    }
    let invocation = prompt.trim().strip_prefix('/')?;
    let (name, arguments) = invocation
        .split_once(char::is_whitespace)
        .map_or((invocation, ""), |(name, arguments)| {
            (name, arguments.trim())
        });
    if name != "goal" {
        return None;
    }
    let goal_is_codex_builtin = commands.iter().any(|command| {
        command.name == "goal"
            && command.scope == CommandScope::Builtin
            && command.template.is_none()
    });
    if !goal_is_codex_builtin {
        return None;
    }
    Some(match arguments {
        "" => GoalCommand::Show,
        "edit" => GoalCommand::Edit,
        "pause" => GoalCommand::Pause,
        "resume" => GoalCommand::Resume,
        "clear" => GoalCommand::Clear,
        objective => GoalCommand::Set(objective.to_owned()),
    })
}

pub fn expand_command_template(template: &str, args: &str) -> String {
    let positional = args.split_whitespace().collect::<Vec<_>>();
    let mut expanded = String::with_capacity(template.len() + args.len());
    let mut consumed_args = false;
    let mut rest = template;
    while let Some(index) = rest.find('$') {
        expanded.push_str(&rest[..index]);
        let after = &rest[index + 1..];
        if let Some(tail) = after.strip_prefix("ARGUMENTS") {
            expanded.push_str(args);
            consumed_args = true;
            rest = tail;
        } else if let Some(tail) = after.strip_prefix('@') {
            expanded.push_str(args);
            consumed_args = true;
            rest = tail;
        } else if let Some(digit) = after
            .chars()
            .next()
            .and_then(|character| character.to_digit(10))
            .filter(|digit| (1..=9).contains(digit))
        {
            if let Some(argument) = positional.get(digit as usize - 1) {
                expanded.push_str(argument);
            }
            consumed_args = true;
            rest = &after[1..];
        } else {
            expanded.push('$');
            rest = after;
        }
    }
    expanded.push_str(rest);
    if !consumed_args && !args.is_empty() {
        expanded.push_str("\n\n");
        expanded.push_str(args);
    }
    expanded
}

/// Resolve composer text into the exact prompt expected by the provider.
///
/// Template commands expand to their body. Skills keep a slash in the
/// composer and transcript, then resolve to each provider's native syntax at
/// the transport boundary.
pub fn resolved_submission(
    provider: ProviderKind,
    prompt: &str,
    commands: &[SlashCommand],
) -> Option<String> {
    if let Some(skill) = resolved_skill_submission(provider, prompt, commands) {
        return Some(skill);
    }
    let invocation = prompt.strip_prefix('/')?;
    let (name, args) = invocation
        .split_once(char::is_whitespace)
        .map_or((invocation, ""), |(name, args)| (name, args.trim()));
    let command = commands.iter().find(|command| command.name == name)?;
    let template = command.template.as_deref()?;
    Some(expand_command_template(template, args))
}

/// Resolve only provider-native skill syntax, without expanding templates.
pub fn resolved_skill_submission(
    provider: ProviderKind,
    prompt: &str,
    commands: &[SlashCommand],
) -> Option<String> {
    if !matches!(
        provider,
        ProviderKind::Codex | ProviderKind::Fx | ProviderKind::Pi | ProviderKind::OhMyPi
    ) {
        return None;
    }
    let invocation = prompt.strip_prefix('/')?;
    let name = invocation
        .split_once(char::is_whitespace)
        .map_or(invocation, |(name, _)| name);
    if !commands
        .iter()
        .any(|command| command.name == name && command.scope == CommandScope::Skill)
    {
        return None;
    }
    Some(match provider {
        ProviderKind::Codex | ProviderKind::Fx => format!("${invocation}"),
        ProviderKind::Pi | ProviderKind::OhMyPi => format!("/skill:{invocation}"),
        _ => unreachable!("non-native skill providers returned above"),
    })
}

/// Safe default before the daemon supplies probe-backed catalog metadata.
///
/// A provider's syntax alone is not evidence that leading, inline, or repeated
/// mentions work, so every capability stays disabled until the daemon sends
/// version-scoped probe evidence.
pub const fn provider_skill_capability(_provider: ProviderKind) -> SkillProviderCapability {
    SkillProviderCapability {
        native_syntax: None,
        leading_invocation: false,
        inline_mentions: false,
        repeated_mentions: false,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillRenderWarning {
    MissingSkillFile { name: String },
    InvalidSkillFile { name: String, path: String },
}

impl fmt::Display for SkillRenderWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSkillFile { name } => write!(
                formatter,
                "Skill \"{name}\" was left unchanged because it has no SKILL.md path."
            ),
            Self::InvalidSkillFile { name, .. } => write!(
                formatter,
                "Skill \"{name}\" was left unchanged because its SKILL.md path cannot be represented safely."
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillRenderResult {
    pub provider_text: String,
    pub warnings: Vec<SkillRenderWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SkillMention {
    name: String,
    range: Range<usize>,
}

fn collect_skill_mentions(text: &str) -> Vec<SkillMention> {
    let code_ranges = markdown_code_ranges(text);
    let mut mentions = Vec::new();
    for (position, character) in text.char_indices() {
        if character != '$'
            || is_code_position(&code_ranges, position)
            || !is_skill_boundary(text, &code_ranges, position)
        {
            continue;
        }
        let end = text[position..]
            .find(char::is_whitespace)
            .map_or(text.len(), |offset| position + offset);
        if end > position + 1 {
            mentions.push(SkillMention {
                name: text[position + 1..end].to_owned(),
                range: position..end,
            });
        }
    }
    mentions
}

pub fn has_skill_mentions(text: &str) -> bool {
    !collect_skill_mentions(text).is_empty()
}

/// A skill token starts at a whitespace boundary or directly after a closed
/// Markdown code range. The latter keeps parity with web/mobile and lets
/// `` `$example`$deploy `` resolve the second token.
fn is_skill_boundary(text: &str, code_ranges: &[Range<usize>], position: usize) -> bool {
    position == 0
        || text[..position]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
        || code_ranges.iter().any(|range| range.end == position)
}

fn fallback_skill_marker(path: &str) -> Option<String> {
    if path.contains(']')
        || path.contains('\r')
        || path.contains('\n')
        || !is_absolute_skill_path(path)
    {
        return None;
    }
    Some(format!("[mandatory! open skill: {path}]"))
}

/// Skill paths cross client platforms. A Windows desktop may receive a Linux
/// daemon's `/home/.../SKILL.md`, and a Unix desktop may receive a Windows
/// daemon's drive path, so do not use the client host's path parser.
fn is_absolute_skill_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    if path.is_empty() || path.ends_with(['/', '\\']) {
        return false;
    }

    let absolute = if path.starts_with("\\\\") {
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
    };
    if !absolute {
        return false;
    }

    if path.starts_with("\\\\") || (bytes.len() >= 3 && bytes[1] == b':') {
        path.rsplit(['/', '\\']).next() == Some("SKILL.md")
    } else {
        // Backslash is a literal filename character on a POSIX daemon host.
        path.rsplit('/').next() == Some("SKILL.md")
    }
}

fn native_skill_text(syntax: SkillNativeSyntax, name: &str) -> String {
    match syntax {
        SkillNativeSyntax::Dollar => format!("${name}"),
        SkillNativeSyntax::SkillSlash => format!("/skill:{name}"),
    }
}

/// Render visible `$skill` mentions into provider-facing prompt text.
///
/// Matching is exact and case-sensitive. Every recognized occurrence is
/// considered, repeated mentions are preserved, and unknown names stay
/// untouched. Providers without verified safe syntax receive the exact
/// `SKILL.md` fallback marker.
pub fn render_skill_mentions(
    provider: ProviderKind,
    text: &str,
    commands: &[SlashCommand],
) -> SkillRenderResult {
    render_skill_mentions_with_capability(provider_skill_capability(provider), text, commands)
}

pub fn render_skill_mentions_with_capability(
    capability: SkillProviderCapability,
    text: &str,
    commands: &[SlashCommand],
) -> SkillRenderResult {
    let recognized = collect_skill_mentions(text)
        .into_iter()
        .filter_map(|mention| {
            commands
                .iter()
                .find(|command| {
                    command.scope == CommandScope::Skill && command.name == mention.name
                })
                .map(|command| (mention, command))
        })
        .collect::<Vec<_>>();
    if recognized.is_empty() {
        return SkillRenderResult {
            provider_text: text.to_owned(),
            warnings: Vec::new(),
        };
    }

    let position_supported = recognized.iter().all(|(mention, _)| {
        if text[..mention.range.start].trim().is_empty() {
            capability.leading_invocation
        } else {
            capability.inline_mentions
        }
    });
    let count_supported = recognized.len() == 1 || capability.repeated_mentions;
    let native = position_supported && count_supported && capability.native_syntax.is_some();

    let mut provider_text = String::with_capacity(text.len());
    let mut warnings = Vec::new();
    let mut copied_until = 0;
    for (mention, command) in recognized {
        provider_text.push_str(&text[copied_until..mention.range.start]);
        if native {
            provider_text.push_str(&native_skill_text(
                capability
                    .native_syntax
                    .expect("native rendering requires a syntax"),
                &mention.name,
            ));
        } else {
            match command.skill_file.as_deref() {
                Some(path) => {
                    if let Some(marker) = fallback_skill_marker(path) {
                        provider_text.push_str(&marker);
                    } else {
                        provider_text.push_str(&text[mention.range.clone()]);
                        let warning = SkillRenderWarning::InvalidSkillFile {
                            name: mention.name,
                            path: path.to_owned(),
                        };
                        if !warnings.contains(&warning) {
                            warnings.push(warning);
                        }
                    }
                }
                None => {
                    provider_text.push_str(&text[mention.range.clone()]);
                    let warning = SkillRenderWarning::MissingSkillFile { name: mention.name };
                    if !warnings.contains(&warning) {
                        warnings.push(warning);
                    }
                }
            }
        }
        copied_until = mention.range.end;
    }
    provider_text.push_str(&text[copied_until..]);
    SkillRenderResult {
        provider_text,
        warnings,
    }
}

pub fn matcher() -> Matcher {
    Matcher::new(nucleo_matcher::Config::DEFAULT.match_paths())
}

#[derive(Clone, Debug)]
pub struct Scored<T> {
    pub item: T,
    pub positions: Vec<u32>,
}

fn filter_scored(
    haystack: &[&str],
    query: &str,
    matcher: &mut Matcher,
    cap: usize,
) -> Vec<(usize, Vec<u32>)> {
    if query.trim().is_empty() {
        return (0..haystack.len().min(cap))
            .map(|index| (index, Vec::new()))
            .collect();
    }
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut buf = Vec::new();
    let mut scored = Vec::new();
    for (index, text) in haystack.iter().enumerate() {
        if let Some(score) = pattern.score(Utf32Str::new(text, &mut buf), matcher) {
            scored.push((score, index));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.truncate(cap);
    scored
        .into_iter()
        .map(|(_, index)| {
            let mut positions = Vec::new();
            pattern.indices(
                Utf32Str::new(haystack[index], &mut buf),
                matcher,
                &mut positions,
            );
            positions.sort_unstable();
            positions.dedup();
            (index, positions)
        })
        .collect()
}

pub fn filter_commands(
    commands: &[SlashCommand],
    query: &str,
    matcher: &mut Matcher,
) -> Vec<Scored<SlashCommand>> {
    let names = commands
        .iter()
        .map(|command| command.name.as_str())
        .collect::<Vec<_>>();
    filter_scored(&names, query, matcher, FILTER_CAP)
        .into_iter()
        .map(|(index, positions)| Scored {
            item: commands[index].clone(),
            positions,
        })
        .collect()
}

pub fn filter_skill_commands(
    commands: &[SlashCommand],
    query: &str,
    matcher: &mut Matcher,
) -> Vec<Scored<SlashCommand>> {
    let skills = commands
        .iter()
        .filter(|command| command.scope == CommandScope::Skill)
        .cloned()
        .collect::<Vec<_>>();
    filter_commands(&skills, query, matcher)
}

/// Filter the `/` picker without exposing skill entries. Skills are selected
/// only with the separate `$` trigger.
pub fn filter_non_skill_commands(
    commands: &[SlashCommand],
    query: &str,
    matcher: &mut Matcher,
) -> Vec<Scored<SlashCommand>> {
    let commands = commands
        .iter()
        .filter(|command| command.scope != CommandScope::Skill)
        .cloned()
        .collect::<Vec<_>>();
    filter_commands(&commands, query, matcher)
}

pub fn filter_files(
    files: &[FileEntry],
    query: &str,
    matcher: &mut Matcher,
) -> Vec<Scored<FileEntry>> {
    let paths = files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<Vec<_>>();
    filter_scored(&paths, query, matcher, FILTER_CAP)
        .into_iter()
        .map(|(index, positions)| Scored {
            item: files[index].clone(),
            positions,
        })
        .collect()
}

pub fn highlight_byte_ranges(
    text: &str,
    positions: &[u32],
    char_offset: usize,
) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for (char_index, (byte_index, character)) in (char_offset..).zip(text.char_indices()) {
        if positions.binary_search(&(char_index as u32)).is_ok() {
            let byte_end = byte_index + character.len_utf8();
            match ranges.last_mut() {
                Some(last) if last.end == byte_index => last.end = byte_end,
                _ => ranges.push(byte_index..byte_end),
            }
        }
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKILL_PATH: &str = "/home/alice/.agents/skills/deploy/SKILL.md";

    #[test]
    fn skill_trigger_starts_only_at_a_whitespace_token_boundary() {
        let trigger = detect_trigger("$bra", 4).expect("a leading skill token triggers");
        assert_eq!(trigger.kind, TriggerKind::Skill);
        assert_eq!(trigger.query, "bra");
        assert_eq!(trigger.range, 0..4);

        let text = "please $verify";
        let trigger = detect_trigger(text, text.len()).expect("a mid-prompt skill token triggers");
        assert_eq!(trigger.kind, TriggerKind::Skill);
        assert_eq!(trigger.query, "verify");
        assert_eq!(trigger.range, 7..text.len());

        assert!(detect_trigger("price$verify", 12).is_none());
        assert!(detect_trigger("\\$verify", 8).is_none());
        assert!(detect_trigger("$verify done", 8).is_none());
        assert_eq!(
            detect_trigger("/compact", 8).map(|trigger| trigger.kind),
            Some(TriggerKind::Command)
        );

        let text = "/review $bra";
        let trigger =
            detect_trigger(text, text.len()).expect("a skill after slash arguments triggers");
        assert_eq!(trigger.kind, TriggerKind::Skill);
        assert_eq!(trigger.query, "bra");
        assert_eq!(trigger.range, 8..text.len());
    }

    #[test]
    fn skill_trigger_ignores_markdown_code_constructs() {
        for text in [
            "`$deploy`",
            "``$deploy``",
            "````$deploy````",
            "```\n$deploy\n```",
            "````\n$deploy\n````",
            "~~~\n$deploy\n~~~",
            "~~~\r\n$deploy\r\n~~~",
            "  ```\n$deploy\n  ```",
            "~~~\n$deploy",
            "``$deploy```",
        ] {
            assert!(
                detect_trigger(text, text.len()).is_none(),
                "skill trigger leaked from Markdown code: {text:?}"
            );
        }

        for text in [
            "`$deploy` $verify",
            "````$deploy```` $verify",
            "```\n$deploy\n```\n$verify",
            "~~~\r\n$deploy\r\n~~~\r\n$verify",
            "`\\$deploy` $verify",
            "`$deploy`$verify",
            "\\` $verify",
            "~~ $verify",
        ] {
            let trigger =
                detect_trigger(text, text.len()).expect("mention after Markdown code must trigger");
            assert_eq!(trigger.kind, TriggerKind::Skill, "{text:?}");
            assert_eq!(trigger.query, "verify", "{text:?}");
        }
    }

    #[test]
    fn skill_rows_are_filtered_and_inserted_with_a_dollar_sigil() {
        let commands = vec![
            skill("deploy", SKILL_PATH),
            command("deploy", CommandScope::Project),
            skill("verify", "/home/alice/.agents/skills/verify/SKILL.md"),
        ];
        let mut matcher = matcher();
        let matched = filter_skill_commands(&commands, "dep", &mut matcher);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].item.scope, CommandScope::Skill);
        assert_eq!(matched[0].item.name, "deploy");
        assert_eq!(skill_composer_text(&matched[0].item), "$deploy");
    }

    #[test]
    fn skill_rendering_protects_all_supported_markdown_code_forms() {
        let commands = [skill("deploy", SKILL_PATH)];
        for (input, expected) in [
            ("`$deploy`", "`$deploy`"),
            (
                "````$deploy```` $deploy",
                "````$deploy```` [mandatory! open skill: /home/alice/.agents/skills/deploy/SKILL.md]",
            ),
            (
                "```text\n$deploy\n```\n$deploy",
                "```text\n$deploy\n```\n[mandatory! open skill: /home/alice/.agents/skills/deploy/SKILL.md]",
            ),
            (
                "````\n$deploy\n````\n$deploy",
                "````\n$deploy\n````\n[mandatory! open skill: /home/alice/.agents/skills/deploy/SKILL.md]",
            ),
            (
                "~~~\n$deploy\n~~~\n$deploy",
                "~~~\n$deploy\n~~~\n[mandatory! open skill: /home/alice/.agents/skills/deploy/SKILL.md]",
            ),
            (
                "~~~\r\n$deploy\r\n~~~\r\n$deploy",
                "~~~\r\n$deploy\r\n~~~\r\n[mandatory! open skill: /home/alice/.agents/skills/deploy/SKILL.md]",
            ),
            (
                "  ```\n$deploy\n  ```\n$deploy",
                "  ```\n$deploy\n  ```\n[mandatory! open skill: /home/alice/.agents/skills/deploy/SKILL.md]",
            ),
            (
                "`\\$deploy` $deploy",
                "`\\$deploy` [mandatory! open skill: /home/alice/.agents/skills/deploy/SKILL.md]",
            ),
            (
                "`$ignore`$deploy",
                "`$ignore`[mandatory! open skill: /home/alice/.agents/skills/deploy/SKILL.md]",
            ),
            ("~~~\n$deploy", "~~~\n$deploy"),
            ("``$deploy``` $deploy", "``$deploy``` $deploy"),
            ("\\$deploy", "\\$deploy"),
            (
                "~~ $deploy",
                "~~ [mandatory! open skill: /home/alice/.agents/skills/deploy/SKILL.md]",
            ),
        ] {
            let rendered = render_skill_mentions(ProviderKind::Claude, input, &commands);
            assert_eq!(rendered.provider_text, expected, "input: {input:?}");
            assert!(rendered.warnings.is_empty(), "input: {input:?}");
        }
    }

    #[test]
    fn fallback_preserves_repeats_spaces_and_ignores_unknown_or_wrong_case_names() {
        let spaced_path = "/home/alice/.agents/skills/deploy release/SKILL.md";
        let commands = [skill("deploy", spaced_path)];
        let marker = "[mandatory! open skill: /home/alice/.agents/skills/deploy release/SKILL.md]";

        let repeated = render_skill_mentions(ProviderKind::Claude, "$deploy $deploy", &commands);
        assert_eq!(repeated.provider_text, format!("{marker} {marker}"));
        assert!(repeated.warnings.is_empty());

        let unknown =
            render_skill_mentions(ProviderKind::Claude, "$Deploy $unknown $deploy", &commands);
        assert_eq!(unknown.provider_text, format!("$Deploy $unknown {marker}"));
        assert!(unknown.warnings.is_empty());

        let literal_backslash_path = "/home/alice/.agents/skills/deploy\\release/SKILL.md";
        let backslash = render_skill_mentions(
            ProviderKind::Claude,
            "$deploy",
            &[skill("deploy", literal_backslash_path)],
        );
        assert_eq!(
            backslash.provider_text,
            format!("[mandatory! open skill: {literal_backslash_path}]")
        );
        assert!(backslash.warnings.is_empty());
    }

    #[test]
    fn missing_and_unrepresentable_skill_paths_stay_visible_with_warnings() {
        let mut missing = command("deploy", CommandScope::Skill);
        missing.skill_file = None;
        let rendered = render_skill_mentions(ProviderKind::Claude, "$deploy now", &[missing]);
        assert_eq!(rendered.provider_text, "$deploy now");
        assert_eq!(
            rendered.warnings,
            [SkillRenderWarning::MissingSkillFile {
                name: "deploy".into()
            }]
        );
        assert_eq!(
            rendered.warnings[0].to_string(),
            "Skill \"deploy\" was left unchanged because it has no SKILL.md path."
        );

        for path in [
            "/home/alice/skills/deploy]release/SKILL.md",
            "/home/alice/skills/deploy\rrelease/SKILL.md",
            "/home/alice/skills/deploy\nrelease/SKILL.md",
            "relative/skills/deploy/SKILL.md",
        ] {
            let rendered =
                render_skill_mentions(ProviderKind::Claude, "$deploy", &[skill("deploy", path)]);
            assert_eq!(rendered.provider_text, "$deploy");
            assert_eq!(
                rendered.warnings,
                [SkillRenderWarning::InvalidSkillFile {
                    name: "deploy".into(),
                    path: path.into(),
                }]
            );
            assert_eq!(
                rendered.warnings[0].to_string(),
                "Skill \"deploy\" was left unchanged because its SKILL.md path cannot be represented safely."
            );
        }

        let repeated = render_skill_mentions(
            ProviderKind::Claude,
            "$deploy and $deploy",
            &[skill(
                "deploy",
                "/home/alice/skills/deploy]release/SKILL.md",
            )],
        );
        assert_eq!(repeated.warnings.len(), 1);
    }

    #[test]
    fn absolute_skill_paths_are_platform_neutral() {
        for path in [
            "/home/alice/.agents/skills/deploy/SKILL.md",
            "C:\\Users\\alice\\skills\\deploy\\SKILL.md",
            "\\\\server\\share\\skills\\deploy\\SKILL.md",
        ] {
            assert!(fallback_skill_marker(path).is_some(), "{path}");
        }
        for path in [
            "relative/skills/deploy/SKILL.md",
            "~/skills/deploy/SKILL.md",
            "C:relative\\SKILL.md",
            "/home/alice/skills/deploy/",
            "/home/alice/skills/deploy/SKILL.md/",
            "/home/alice/skills/deploy/not-a-skill.md",
            "C:\\Users\\alice\\skills\\deploy\\",
            "C:\\Users\\alice\\skills\\deploy\\notes.md",
            "\\\\server\\share\\",
            "\\\\server\\SKILL.md",
        ] {
            assert!(fallback_skill_marker(path).is_none(), "{path}");
        }
    }

    #[test]
    fn fallback_matches_the_shared_cross_surface_fixture() {
        #[derive(serde::Deserialize)]
        struct Case {
            provider: ProviderKind,
            text: String,
            command: String,
            path: Option<String>,
            capability: Option<SkillProviderCapability>,
            expected: String,
            #[serde(default)]
            warnings: Vec<String>,
        }

        let fixture = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/padu-client/testdata/skill-fallback-parity.json"),
        )
        .expect("shared parity fixture must exist");
        let cases = serde_json::from_str::<Vec<Case>>(&fixture).unwrap();
        for case in cases {
            let mut command = command(&case.command, CommandScope::Skill);
            command.skill_file = case.path;
            let rendered = render_skill_mentions_with_capability(
                case.capability
                    .unwrap_or_else(|| provider_skill_capability(case.provider)),
                &case.text,
                &[command],
            );
            assert_eq!(rendered.provider_text, case.expected, "{}", case.text);
            assert_eq!(
                rendered
                    .warnings
                    .into_iter()
                    .map(|warning| warning.to_string())
                    .collect::<Vec<_>>(),
                case.warnings,
                "{}",
                case.text
            );
        }
    }

    #[test]
    fn native_skill_rendering_is_gated_by_probe_evidence() {
        for (provider, syntax) in [
            (ProviderKind::Codex, SkillNativeSyntax::Dollar),
            (ProviderKind::Fx, SkillNativeSyntax::Dollar),
            (ProviderKind::Pi, SkillNativeSyntax::SkillSlash),
            (ProviderKind::OhMyPi, SkillNativeSyntax::SkillSlash),
        ] {
            let capability = provider_skill_capability(provider);
            assert_eq!(capability.native_syntax, None);
            assert!(!capability.leading_invocation);
            assert!(!capability.inline_mentions);
            assert!(!capability.repeated_mentions);
            assert_ne!(provider, ProviderKind::Claude);
            assert!(matches!(
                syntax,
                SkillNativeSyntax::Dollar | SkillNativeSyntax::SkillSlash
            ));
        }
        assert_eq!(
            provider_skill_capability(ProviderKind::Claude).native_syntax,
            None
        );

        let commands = [skill("deploy", SKILL_PATH)];
        assert_eq!(
            render_skill_mentions(ProviderKind::Pi, "$deploy now", &commands).provider_text,
            "[mandatory! open skill: /home/alice/.agents/skills/deploy/SKILL.md] now"
        );
        for text in ["please $deploy now", "$deploy $deploy"] {
            let rendered = render_skill_mentions(ProviderKind::Pi, text, &commands);
            assert_eq!(
                rendered.provider_text,
                text.replace(
                    "$deploy",
                    "[mandatory! open skill: /home/alice/.agents/skills/deploy/SKILL.md]"
                )
            );
        }

        let proven = SkillProviderCapability {
            native_syntax: Some(SkillNativeSyntax::SkillSlash),
            leading_invocation: true,
            inline_mentions: true,
            repeated_mentions: true,
        };
        assert_eq!(
            render_skill_mentions_with_capability(proven, "please $deploy and $deploy", &commands)
                .provider_text,
            "please /skill:deploy and /skill:deploy"
        );

        let no_leading_evidence = SkillProviderCapability {
            native_syntax: Some(SkillNativeSyntax::SkillSlash),
            leading_invocation: false,
            inline_mentions: true,
            repeated_mentions: true,
        };
        assert_eq!(
            render_skill_mentions_with_capability(no_leading_evidence, "$deploy", &commands)
                .provider_text,
            "[mandatory! open skill: /home/alice/.agents/skills/deploy/SKILL.md]"
        );
    }

    #[test]
    fn merged_command_picker_puts_builtins_first_and_skills_last() {
        let discovered = vec![
            command("deploy", CommandScope::Skill),
            command("format", CommandScope::User),
            command("lint", CommandScope::Project),
            command("review", CommandScope::Builtin),
        ];
        let reported = vec![ReportedCommand {
            name: "compact".into(),
            description: "Free up context".into(),
        }];

        let merged = merge_reported_commands(&discovered, &reported);
        assert_eq!(
            merged
                .iter()
                .map(|command| (command.scope, command.name.as_str()))
                .collect::<Vec<_>>(),
            [
                (CommandScope::Builtin, "compact"),
                (CommandScope::Builtin, "review"),
                (CommandScope::Project, "lint"),
                (CommandScope::User, "format"),
                (CommandScope::Skill, "deploy"),
            ]
        );
    }

    #[test]
    fn fast_toggle_is_codex_only_and_respects_command_overrides() {
        let builtin = command("fast", CommandScope::Builtin);
        assert!(is_fast_mode_toggle_submission(
            ProviderKind::Codex,
            "/fast ",
            std::slice::from_ref(&builtin),
        ));
        assert!(!is_fast_mode_toggle_submission(
            ProviderKind::Claude,
            "/fast",
            std::slice::from_ref(&builtin),
        ));
        assert!(!is_fast_mode_toggle_submission(
            ProviderKind::Codex,
            "/fast now",
            std::slice::from_ref(&builtin),
        ));
        assert!(!is_fast_mode_toggle_submission(
            ProviderKind::Codex,
            "/fast",
            &[command("fast", CommandScope::Project)],
        ));
    }

    #[test]
    fn resume_is_an_exact_provider_neutral_local_command() {
        assert!(is_resume_submission("/resume"));
        assert!(is_resume_submission("  /resume  "));
        assert!(!is_resume_submission("/resume latest"));
        assert!(!is_resume_submission("please /resume"));
    }

    #[test]
    fn fast_toggle_uses_the_models_concrete_service_tier_id() {
        let tiers = [ProviderModelOption::new("priority", "Fast")];
        assert_eq!(
            toggled_fast_service_tier(Some("default"), &tiers).as_deref(),
            Some("priority")
        );
        assert_eq!(
            toggled_fast_service_tier(Some("priority"), &tiers).as_deref(),
            Some("default")
        );
        assert_eq!(toggled_fast_service_tier(None, &[]), None);
    }

    #[test]
    fn codex_skill_completion_keeps_slash_in_the_composer() {
        let skill = command("mattpocock-skills:to-spec", CommandScope::Skill);
        assert_eq!(command_composer_text(&skill), "/mattpocock-skills:to-spec");
        assert_eq!(
            command_composer_text(&command("fast", CommandScope::Builtin)),
            "/fast"
        );
    }

    #[test]
    fn codex_skill_submission_uses_the_catalog_invocation() {
        let skill = command("mattpocock-skills:to-spec", CommandScope::Skill);
        assert_eq!(
            resolved_submission(
                ProviderKind::Codex,
                "/mattpocock-skills:to-spec carefully",
                std::slice::from_ref(&skill)
            )
            .as_deref(),
            Some("$mattpocock-skills:to-spec carefully")
        );
        assert_eq!(
            resolved_submission(
                ProviderKind::Claude,
                "/mattpocock-skills:to-spec carefully",
                std::slice::from_ref(&skill)
            ),
            None
        );
    }

    #[test]
    fn fx_skill_submission_uses_the_catalog_invocation() {
        let skill = command("deploy", CommandScope::Skill);
        assert_eq!(
            resolved_submission(
                ProviderKind::Fx,
                "/deploy production",
                std::slice::from_ref(&skill)
            )
            .as_deref(),
            Some("$deploy production")
        );
    }

    #[test]
    fn pi_skill_submission_uses_the_skill_command() {
        for provider in [ProviderKind::Pi, ProviderKind::OhMyPi] {
            for name in ["to-spec", "to-tickets"] {
                let skill = command(name, CommandScope::Skill);
                let expected = format!("/skill:{name} carefully");
                assert_eq!(
                    resolved_skill_submission(provider, &format!("/{name} carefully"), &[skill])
                        .as_deref(),
                    Some(expected.as_str())
                );
            }
        }
    }

    fn command(name: &str, scope: CommandScope) -> SlashCommand {
        SlashCommand {
            name: name.into(),
            description: String::new(),
            scope,
            argument_hint: None,
            template: None,
            skill_file: None,
        }
    }

    fn skill(name: &str, path: &str) -> SlashCommand {
        let mut command = command(name, CommandScope::Skill);
        command.skill_file = Some(path.into());
        command
    }

    #[test]
    fn goal_submissions_parse_into_their_intent() {
        let builtin = command("goal", CommandScope::Builtin);
        let commands = std::slice::from_ref(&builtin);
        let parse = |prompt: &str| parse_goal_submission(ProviderKind::Codex, prompt, commands);

        assert_eq!(parse("/goal"), Some(GoalCommand::Show));
        assert_eq!(parse("/goal "), Some(GoalCommand::Show));
        assert_eq!(parse("/goal edit"), Some(GoalCommand::Edit));
        assert_eq!(parse("/goal pause"), Some(GoalCommand::Pause));
        assert_eq!(parse("/goal resume"), Some(GoalCommand::Resume));
        assert_eq!(parse("/goal clear"), Some(GoalCommand::Clear));
        assert_eq!(
            parse("/goal improve benchmark coverage"),
            Some(GoalCommand::Set("improve benchmark coverage".into()))
        );
        assert_eq!(parse("/goals"), None);
        assert_eq!(parse("ship /goal"), None);
    }

    #[test]
    fn goal_command_is_codex_only_and_respects_overrides() {
        let builtin = command("goal", CommandScope::Builtin);
        assert_eq!(
            parse_goal_submission(
                ProviderKind::Claude,
                "/goal",
                std::slice::from_ref(&builtin)
            ),
            None
        );
        // A project command deliberately owning /goal wins the collision.
        let mut project = command("goal", CommandScope::Project);
        project.template = Some("do project things".into());
        assert_eq!(
            parse_goal_submission(ProviderKind::Codex, "/goal", std::slice::from_ref(&project)),
            None
        );
    }
}
