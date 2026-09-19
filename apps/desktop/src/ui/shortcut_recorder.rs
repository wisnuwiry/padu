use gpui::{App, FocusHandle, KeyDownEvent, Keystroke, Window};

/// Formats a complete shortcut combination into a human-readable display string.
/// Returns `None` if no modifier is pressed (or not a function key), as single plain
/// characters shouldn't be valid shortcuts.
pub fn format_keystroke_shortcut(keystroke: &Keystroke) -> Option<String> {
    let key_lower = keystroke.key.to_lowercase();
    if matches!(
        key_lower.as_str(),
        "cmd"
            | "command"
            | "meta"
            | "shift"
            | "control"
            | "ctrl"
            | "alt"
            | "option"
            | "fn"
            | "super"
            | "hyper"
            | "win"
            | "windows"
            | "altgr"
            | "altgraph"
            | "menu"
    ) {
        return None;
    }

    let is_fn_key = key_lower.starts_with('f')
        && key_lower.len() > 1
        && key_lower[1..].chars().all(|c| c.is_ascii_digit());

    if !keystroke.modifiers.platform
        && !keystroke.modifiers.control
        && !keystroke.modifiers.alt
        && !is_fn_key
    {
        return None;
    }

    let is_macos = cfg!(target_os = "macos");

    let display_key = match key_lower.as_str() {
        "enter" | "return" | "numpadenter" => {
            if is_macos {
                "↵".to_string()
            } else {
                "Enter".to_string()
            }
        }
        "tab" => {
            if is_macos {
                "⇥".to_string()
            } else {
                "Tab".to_string()
            }
        }
        "space" => "Space".to_string(),
        "backspace" => {
            if is_macos {
                "⌫".to_string()
            } else {
                "Backspace".to_string()
            }
        }
        "delete" => {
            if is_macos {
                "⌦".to_string()
            } else {
                "Delete".to_string()
            }
        }
        "insert" | "ins" => "Insert".to_string(),
        "escape" => "Esc".to_string(),
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "pageup" | "pgup" => "PageUp".to_string(),
        "pagedown" | "pgdn" => "PageDown".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        k if k.starts_with('f') && k[1..].chars().all(|c| c.is_ascii_digit()) => k.to_uppercase(),
        k if k.len() == 1 => k.to_uppercase(),
        other => {
            let mut c = other.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        }
    };

    if is_macos {
        let mut s = String::new();
        if keystroke.modifiers.platform {
            s.push('⌘');
        }
        if keystroke.modifiers.control {
            s.push('⌃');
        }
        if keystroke.modifiers.alt {
            s.push('⌥');
        }
        if keystroke.modifiers.shift {
            s.push('⇧');
        }
        s.push_str(&display_key);
        Some(s)
    } else {
        let mut parts = Vec::new();
        let is_windows = cfg!(target_os = "windows");
        let platform_name = if is_windows { "Win" } else { "Super" };

        if keystroke.modifiers.platform {
            parts.push(platform_name);
        }
        if keystroke.modifiers.control {
            parts.push("Ctrl");
        }
        if keystroke.modifiers.alt {
            parts.push("Alt");
        }
        if keystroke.modifiers.shift {
            parts.push("Shift");
        }
        parts.push(&display_key);
        Some(parts.join("+"))
    }
}

/// Formats any keystroke in real-time, including when only modifier keys are pressed.
pub fn format_keystroke_realtime(keystroke: &Keystroke) -> String {
    if let Some(shortcut) = format_keystroke_shortcut(keystroke) {
        return shortcut;
    }

    let is_macos = cfg!(target_os = "macos");
    let key_lower = keystroke.key.to_lowercase();
    let has_platform = keystroke.modifiers.platform
        || matches!(
            key_lower.as_str(),
            "cmd" | "command" | "meta" | "super" | "win" | "windows"
        );
    let has_control =
        keystroke.modifiers.control || matches!(key_lower.as_str(), "ctrl" | "control");
    let has_alt = keystroke.modifiers.alt
        || matches!(key_lower.as_str(), "alt" | "option" | "altgr" | "altgraph");
    let has_shift = keystroke.modifiers.shift || matches!(key_lower.as_str(), "shift");

    if is_macos {
        let mut s = String::new();
        if has_platform {
            s.push('⌘');
        }
        if has_control {
            s.push('⌃');
        }
        if has_alt {
            s.push('⌥');
        }
        if has_shift {
            s.push('⇧');
        }
        s
    } else {
        let mut parts = Vec::new();
        let is_windows = cfg!(target_os = "windows");
        let platform_name = if is_windows { "Win" } else { "Super" };

        if has_platform {
            parts.push(platform_name);
        }
        if has_control {
            parts.push("Ctrl");
        }
        if has_alt {
            parts.push("Alt");
        }
        if has_shift {
            parts.push("Shift");
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("{}+", parts.join("+"))
        }
    }
}

pub fn normalize_key(k: &str) -> String {
    match k.trim().to_ascii_lowercase().as_str() {
        "↵" | "return" | "enter" | "numpadenter" => "enter".to_string(),
        "⇥" | "tab" => "tab".to_string(),
        "⌫" | "backspace" => "backspace".to_string(),
        "⌦" | "delete" | "del" => "delete".to_string(),
        "insert" | "ins" => "insert".to_string(),
        "↑" | "up" => "up".to_string(),
        "↓" | "down" => "down".to_string(),
        "←" | "left" => "left".to_string(),
        "→" | "right" => "right".to_string(),
        "esc" | "escape" => "escape".to_string(),
        " " | "space" => "space".to_string(),
        "pgup" | "pageup" => "pageup".to_string(),
        "pgdn" | "pagedown" => "pagedown".to_string(),
        other => other.to_string(),
    }
}

/// Parses a shortcut string (such as "⌘⇧R", "Cmd+Shift+R", "ctrl+alt+t", "Win+Shift+S", "Super+T", "F5")
/// into its modifier flags and the normalized primary key name.
pub fn parse_shortcut(shortcut: &str) -> (bool, bool, bool, bool, String) {
    let mut has_platform = false;
    let mut has_control = false;
    let mut has_alt = false;
    let mut has_shift = false;

    let mut remaining = String::new();

    // Check for macOS unicode symbols
    for ch in shortcut.chars() {
        match ch {
            '⌘' => has_platform = true,
            '⌃' => has_control = true,
            '⌥' => has_alt = true,
            '⇧' => has_shift = true,
            other => remaining.push(other),
        }
    }

    let remaining_trimmed = remaining.trim();
    // If it was formatted with + or - separators, parse tokens
    if remaining_trimmed.contains('+') || remaining_trimmed.contains('-') {
        let delimiter = if remaining_trimmed.contains('+') {
            '+'
        } else {
            '-'
        };
        let mut final_key = String::new();
        let parts: Vec<&str> = remaining_trimmed.split(delimiter).collect();
        for (i, part) in parts.iter().enumerate() {
            let token = part.trim().to_lowercase();
            match token.as_str() {
                "cmd" | "command" | "meta" | "super" | "win" | "windows" => has_platform = true,
                "ctrl" | "control" => has_control = true,
                "alt" | "option" | "altgr" | "altgraph" => has_alt = true,
                "shift" => has_shift = true,
                "" => {
                    // Handles cases where the key itself is + or - (e.g. "Ctrl++" or "Ctrl+-")
                    if i == parts.len() - 1 && parts.len() > 1 {
                        final_key = delimiter.to_string();
                    }
                }
                _ => final_key = part.trim().to_string(),
            }
        }
        (
            has_platform,
            has_control,
            has_alt,
            has_shift,
            normalize_key(&final_key),
        )
    } else {
        (
            has_platform,
            has_control,
            has_alt,
            has_shift,
            normalize_key(remaining_trimmed),
        )
    }
}

/// Formats a stored shortcut string for human-readable display on the current platform.
/// On macOS, formats with Mac symbols ("⌘⇧R").
/// On Windows, formats with "Ctrl+Shift+R" or "Win+Shift+R".
/// On Linux, formats with "Ctrl+Shift+R" or "Super+Shift+R".
pub fn format_shortcut_for_display(shortcut: &str) -> String {
    let trimmed = shortcut.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let is_macos = cfg!(target_os = "macos");
    let (mut has_platform, mut has_control, has_alt, has_shift, key) = parse_shortcut(trimmed);

    if is_macos {
        // If a shortcut was authored on Windows/Linux as Ctrl+Key without platform modifier,
        // and doesn't explicitly use Control glyphs, adapt it to ⌘ for macOS display.
        if has_control && !has_platform && !trimmed.contains('⌃') {
            has_platform = true;
            has_control = false;
        }

        let mut s = String::new();
        if has_platform {
            s.push('⌘');
        }
        if has_control {
            s.push('⌃');
        }
        if has_alt {
            s.push('⌥');
        }
        if has_shift {
            s.push('⇧');
        }
        let display_key = match key.as_str() {
            "enter" => "↵",
            "tab" => "⇥",
            "backspace" => "⌫",
            "delete" => "⌦",
            "escape" => "Esc",
            "space" => "Space",
            "up" => "↑",
            "down" => "↓",
            "left" => "←",
            "right" => "→",
            k if k.len() == 1 => {
                s.push_str(&k.to_uppercase());
                return s;
            }
            other => other,
        };
        s.push_str(display_key);
        s
    } else {
        // Windows / Linux:
        // If a shortcut was authored on macOS with ⌘/Cmd and no Ctrl, adapt to Ctrl for display.
        if has_platform
            && !has_control
            && (trimmed.contains('⌘') || trimmed.to_lowercase().contains("cmd"))
        {
            has_control = true;
            has_platform = false;
        }

        let mut parts = Vec::new();
        let is_windows = cfg!(target_os = "windows");
        let platform_name = if is_windows { "Win" } else { "Super" };

        if has_platform {
            parts.push(platform_name);
        }
        if has_control {
            parts.push("Ctrl");
        }
        if has_alt {
            parts.push("Alt");
        }
        if has_shift {
            parts.push("Shift");
        }
        let display_key = match key.as_str() {
            "enter" => "Enter".to_string(),
            "tab" => "Tab".to_string(),
            "backspace" => "Backspace".to_string(),
            "delete" => "Delete".to_string(),
            "escape" => "Esc".to_string(),
            "space" => "Space".to_string(),
            "up" => "↑".to_string(),
            "down" => "↓".to_string(),
            "left" => "←".to_string(),
            "right" => "→".to_string(),
            k if k.len() == 1 => k.to_uppercase(),
            other => {
                let mut c = other.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            }
        };
        parts.push(&display_key);
        parts.join("+")
    }
}

/// Returns true if the stored shortcut string matches the incoming `Keystroke`.
/// Supports direct exact matches and cross-platform primary modifier translation:
/// - macOS Command (⌘ / `platform`) matches Windows/Linux `Ctrl` (`control`)
/// - Windows/Linux `Ctrl` matches macOS `Command`
pub fn matches_keystroke(shortcut: &str, keystroke: &Keystroke) -> bool {
    let trimmed = shortcut.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Direct match against formatted representation
    if let Some(formatted) = format_keystroke_shortcut(keystroke) {
        if trimmed.eq_ignore_ascii_case(&formatted) {
            return true;
        }
    }

    // Canonical parsed match
    let (sc_platform, sc_ctrl, sc_alt, sc_shift, sc_key) = parse_shortcut(trimmed);
    let ks_platform = keystroke.modifiers.platform;
    let ks_ctrl = keystroke.modifiers.control;
    let ks_alt = keystroke.modifiers.alt;
    let ks_shift = keystroke.modifiers.shift;
    let norm_ks = normalize_key(&keystroke.key);

    if sc_key != norm_ks || sc_alt != ks_alt || sc_shift != ks_shift {
        return false;
    }

    // 1. Direct exact modifier match
    if sc_platform == ks_platform && sc_ctrl == ks_ctrl {
        return true;
    }

    // 2. Cross-platform primary modifier mapping:
    // On macOS: Primary is Cmd (platform).
    // On Linux/Windows: Primary is Ctrl (control).
    // A shortcut stored with the primary modifier from one OS (e.g. Cmd on macOS,
    // or Ctrl on Windows/Linux) will match the active OS's primary modifier when
    // evaluated across platforms.
    let is_macos = cfg!(target_os = "macos");
    if is_macos {
        // Saved on Windows/Linux as Ctrl+Key without platform modifier,
        // user on macOS presses Cmd+Key (platform).
        if sc_ctrl && !sc_platform && ks_platform && !ks_ctrl {
            return true;
        }
    } else {
        // Saved on macOS as Cmd+Key (platform) without control modifier,
        // user on Linux/Windows presses Ctrl+Key (control).
        if sc_platform && !sc_ctrl && ks_ctrl && !ks_platform {
            return true;
        }
    }

    false
}

/// State for a shortcut recording session.
#[derive(Debug)]
pub struct ShortcutRecorderState {
    pub is_recording: bool,
    pub live_preview: Option<String>,
    pub original_keybinding: Option<String>,
    pub focus: FocusHandle,
}

impl ShortcutRecorderState {
    pub fn new(focus: FocusHandle) -> Self {
        Self {
            is_recording: false,
            live_preview: None,
            original_keybinding: None,
            focus,
        }
    }

    pub fn start_recording(&mut self, current: Option<String>, window: &mut Window, cx: &mut App) {
        self.is_recording = true;
        self.original_keybinding = current;
        self.live_preview = None;
        window.focus(&self.focus, cx);
    }

    pub fn stop_recording(&mut self) {
        self.is_recording = false;
        self.live_preview = None;
        self.original_keybinding = None;
    }

    pub fn cancel_recording(&mut self) -> Option<String> {
        let orig = self.original_keybinding.take();
        self.is_recording = false;
        self.live_preview = None;
        orig
    }

    /// Handles a KeyDownEvent during recording. Returns `Some(formatted_shortcut)` if a full
    /// chord was recognized, or `None` if it was a modifier key, navigation, or cancellation.
    pub fn handle_key_down(&mut self, event: &KeyDownEvent) -> KeyDownResult {
        let key_lower = event.keystroke.key.to_lowercase();

        if !self.is_recording {
            if matches!(key_lower.as_str(), "enter" | "return" | "space")
                && !event.keystroke.modifiers.modified()
            {
                return KeyDownResult::StartRecording;
            }
            return KeyDownResult::Ignored;
        }

        if matches!(key_lower.as_str(), "enter" | "return") {
            self.stop_recording();
            return KeyDownResult::Committed;
        }

        if key_lower == "escape" {
            let orig = self.cancel_recording();
            return KeyDownResult::Cancelled(orig);
        }

        if key_lower == "tab" {
            self.stop_recording();
            return KeyDownResult::Ignored;
        }

        if (key_lower == "backspace" || key_lower == "delete")
            && !event.keystroke.modifiers.modified()
        {
            self.live_preview = None;
            return KeyDownResult::Cleared;
        }

        let preview = format_keystroke_realtime(&event.keystroke);
        if !preview.is_empty() {
            self.live_preview = Some(preview);
        }

        if let Some(formatted) = format_keystroke_shortcut(&event.keystroke) {
            KeyDownResult::Shortcut(formatted)
        } else {
            KeyDownResult::PendingPreview
        }
    }
}

pub enum KeyDownResult {
    StartRecording,
    Committed,
    Cancelled(Option<String>),
    Cleared,
    Shortcut(String),
    PendingPreview,
    Ignored,
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Modifiers;

    #[test]
    fn test_format_keystroke_shortcut_with_modifiers() {
        let is_macos = cfg!(target_os = "macos");
        let is_windows = cfg!(target_os = "windows");
        let ks = Keystroke {
            modifiers: Modifiers {
                platform: true,
                shift: true,
                control: false,
                alt: false,
                function: false,
            },
            key: "r".to_string(),
            key_char: None,
        };
        let formatted = format_keystroke_shortcut(&ks).unwrap();
        if is_macos {
            assert_eq!(formatted, "⌘⇧R");
        } else if is_windows {
            assert_eq!(formatted, "Win+Shift+R");
        } else {
            assert_eq!(formatted, "Super+Shift+R");
        }
    }

    #[test]
    fn test_format_keystroke_shortcut_plain_key_rejected() {
        let ks = Keystroke {
            modifiers: Modifiers::default(),
            key: "r".to_string(),
            key_char: None,
        };
        assert!(format_keystroke_shortcut(&ks).is_none());
    }

    #[test]
    fn test_format_keystroke_shortcut_function_key_allowed() {
        let ks = Keystroke {
            modifiers: Modifiers::default(),
            key: "f5".to_string(),
            key_char: None,
        };
        let formatted = format_keystroke_shortcut(&ks).unwrap();
        assert_eq!(formatted, "F5");
    }

    #[test]
    fn test_matches_keystroke_canonical() {
        let ks = Keystroke {
            modifiers: Modifiers {
                platform: true,
                shift: true,
                control: false,
                alt: false,
                function: false,
            },
            key: "r".to_string(),
            key_char: None,
        };

        // Matches unicode symbols
        assert!(matches_keystroke("⌘⇧R", &ks));
        assert!(matches_keystroke("⌘⇧r", &ks));

        // Matches text chord
        assert!(matches_keystroke("cmd+shift+r", &ks));
        assert!(matches_keystroke("Cmd+Shift+R", &ks));

        // Does not match wrong key
        assert!(!matches_keystroke("⌘⇧T", &ks));
        // Does not match wrong modifiers (missing shift)
        assert!(!matches_keystroke("⌘R", &ks));
    }

    #[test]
    fn test_cross_platform_matching_and_delimiters() {
        // Keystroke with Ctrl+Shift+R (standard Windows/Linux chord)
        let ks_ctrl = Keystroke {
            modifiers: Modifiers {
                platform: false,
                shift: true,
                control: true,
                alt: false,
                function: false,
            },
            key: "r".to_string(),
            key_char: None,
        };

        // Keystroke with Cmd+Shift+R (standard macOS chord)
        let ks_cmd = Keystroke {
            modifiers: Modifiers {
                platform: true,
                shift: true,
                control: false,
                alt: false,
                function: false,
            },
            key: "r".to_string(),
            key_char: None,
        };

        // Exact match
        assert!(matches_keystroke("Ctrl+Shift+R", &ks_ctrl));
        assert!(matches_keystroke("⌘⇧R", &ks_cmd));

        // Cross-platform primary modifier translation:
        // On macOS: Ctrl+Shift+R matches Cmd+Shift+R keystroke
        // On Windows/Linux: ⌘⇧R matches Ctrl+Shift+R keystroke
        if cfg!(target_os = "macos") {
            assert!(matches_keystroke("Ctrl+Shift+R", &ks_cmd));
        } else {
            assert!(matches_keystroke("⌘⇧R", &ks_ctrl));
            assert!(matches_keystroke("Cmd+Shift+R", &ks_ctrl));
        }

        // Test zoom shortcuts with delimiter edge cases
        let (p, c, a, s, k) = parse_shortcut("Ctrl++");
        assert!(!p && c && !a && !s && k == "+");

        let (p, c, a, s, k) = parse_shortcut("Ctrl+-");
        assert!(!p && c && !a && !s && k == "-");

        // Test Windows and Linux modifier names in parse_shortcut
        let (p, c, a, s, k) = parse_shortcut("Win+Shift+S");
        assert!(p && !c && !a && s && k == "s");

        let (p, c, a, s, k) = parse_shortcut("Super+Alt+T");
        assert!(p && !c && a && !s && k == "t");
    }

    #[test]
    fn test_format_shortcut_for_display() {
        let is_macos = cfg!(target_os = "macos");
        let is_windows = cfg!(target_os = "windows");

        let formatted = format_shortcut_for_display("⌘⇧R");
        if is_macos {
            assert_eq!(formatted, "⌘⇧R");
        } else {
            assert_eq!(formatted, "Ctrl+Shift+R");
        }

        let formatted_win = format_shortcut_for_display("Win+Shift+S");
        if is_macos {
            assert_eq!(formatted_win, "⌘⇧S");
        } else if is_windows {
            assert_eq!(formatted_win, "Win+Shift+S");
        } else {
            assert_eq!(formatted_win, "Super+Shift+S");
        }
    }

    #[test]
    fn test_format_keystroke_realtime_pure_modifier() {
        let ks = Keystroke {
            modifiers: Modifiers {
                platform: true,
                ..Default::default()
            },
            key: "cmd".to_string(),
            key_char: None,
        };
        let formatted = format_keystroke_realtime(&ks);
        if cfg!(target_os = "macos") {
            assert!(formatted.contains('⌘'));
        } else if cfg!(target_os = "windows") {
            assert!(formatted.contains("Win"));
        } else {
            assert!(formatted.contains("Super"));
        }
    }

    #[test]
    fn test_format_keystroke_realtime_complete_chord() {
        let ks = Keystroke {
            modifiers: Modifiers {
                platform: true,
                shift: true,
                ..Default::default()
            },
            key: "b".to_string(),
            key_char: None,
        };
        let formatted = format_keystroke_realtime(&ks);
        if cfg!(target_os = "macos") {
            assert_eq!(formatted, "⌘⇧B");
        } else if cfg!(target_os = "windows") {
            assert_eq!(formatted, "Win+Shift+B");
        } else {
            assert_eq!(formatted, "Super+Shift+B");
        }
    }

    #[gpui::test]
    fn test_shortcut_recorder_state_lifecycle(cx: &mut gpui::TestAppContext) {
        let focus = cx.update(|cx| cx.focus_handle());
        let mut state = ShortcutRecorderState::new(focus);
        assert!(!state.is_recording);
        assert!(state.original_keybinding.is_none());
        assert!(state.live_preview.is_none());

        // Simulate start recording
        state.is_recording = true;
        state.original_keybinding = Some("⌘B".to_string());
        state.live_preview = Some("⌘".to_string());

        // Stop recording
        state.stop_recording();
        assert!(!state.is_recording);
        assert!(state.original_keybinding.is_none());
        assert!(state.live_preview.is_none());

        // Simulate cancel recording
        state.is_recording = true;
        state.original_keybinding = Some("⌘B".to_string());
        state.live_preview = Some("⌘".to_string());
        let restored = state.cancel_recording();
        assert_eq!(restored, Some("⌘B".to_string()));
        assert!(!state.is_recording);
        assert!(state.original_keybinding.is_none());
        assert!(state.live_preview.is_none());
    }

    #[gpui::test]
    fn test_shortcut_recorder_key_down_modes(cx: &mut gpui::TestAppContext) {
        let focus = cx.update(|cx| cx.focus_handle());
        let mut state = ShortcutRecorderState::new(focus);

        // When not recording: ordinary keys are ignored
        let regular_key = gpui::KeyDownEvent {
            keystroke: Keystroke {
                modifiers: Modifiers::default(),
                key: "a".to_string(),
                key_char: Some("a".into()),
            },
            is_held: false,
            prefer_character_input: false,
        };
        assert!(matches!(
            state.handle_key_down(&regular_key),
            KeyDownResult::Ignored
        ));

        // When not recording: space or enter triggers StartRecording
        let space_key = gpui::KeyDownEvent {
            keystroke: Keystroke {
                modifiers: Modifiers::default(),
                key: "space".to_string(),
                key_char: Some(" ".into()),
            },
            is_held: false,
            prefer_character_input: false,
        };
        assert!(matches!(
            state.handle_key_down(&space_key),
            KeyDownResult::StartRecording
        ));

        // Now enter recording mode
        state.is_recording = true;

        // Escape cancels
        let esc_key = gpui::KeyDownEvent {
            keystroke: Keystroke {
                modifiers: Modifiers::default(),
                key: "escape".to_string(),
                key_char: None,
            },
            is_held: false,
            prefer_character_input: false,
        };
        assert!(matches!(
            state.handle_key_down(&esc_key),
            KeyDownResult::Cancelled(_)
        ));
        assert!(!state.is_recording);

        // Re-enter recording mode
        state.is_recording = true;

        // Enter commits
        let enter_key = gpui::KeyDownEvent {
            keystroke: Keystroke {
                modifiers: Modifiers::default(),
                key: "enter".to_string(),
                key_char: None,
            },
            is_held: false,
            prefer_character_input: false,
        };
        assert!(matches!(
            state.handle_key_down(&enter_key),
            KeyDownResult::Committed
        ));
        assert!(!state.is_recording);
    }
}
