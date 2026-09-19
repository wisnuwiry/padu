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
        "enter" | "return" => {
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
        "escape" => "Esc".to_string(),
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "pageup" => "PageUp".to_string(),
        "pagedown" => "PageDown".to_string(),
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
        if keystroke.modifiers.control {
            parts.push("Ctrl");
        }
        if keystroke.modifiers.alt {
            parts.push("Alt");
        }
        if keystroke.modifiers.shift {
            parts.push("Shift");
        }
        if keystroke.modifiers.platform {
            parts.push("Win");
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
        || matches!(key_lower.as_str(), "cmd" | "command" | "meta" | "super");
    let has_control =
        keystroke.modifiers.control || matches!(key_lower.as_str(), "ctrl" | "control");
    let has_alt = keystroke.modifiers.alt || matches!(key_lower.as_str(), "alt" | "option");
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
        if has_control {
            parts.push("Ctrl");
        }
        if has_alt {
            parts.push("Alt");
        }
        if has_shift {
            parts.push("Shift");
        }
        if has_platform {
            parts.push("Win");
        }
        parts.join("+")
    }
}

pub fn normalize_key(k: &str) -> String {
    match k.trim().to_ascii_lowercase().as_str() {
        "↵" | "return" | "enter" => "enter".to_string(),
        "⇥" | "tab" => "tab".to_string(),
        "⌫" | "backspace" => "backspace".to_string(),
        "⌦" | "delete" | "del" => "delete".to_string(),
        "↑" | "up" => "up".to_string(),
        "↓" | "down" => "down".to_string(),
        "←" | "left" => "left".to_string(),
        "→" | "right" => "right".to_string(),
        "esc" | "escape" => "escape".to_string(),
        " " | "space" => "space".to_string(),
        other => other.to_string(),
    }
}

/// Parses a shortcut string (such as "⌘⇧R", "Cmd+Shift+R", "ctrl+alt+t", "F5")
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
        for part in remaining_trimmed.split(delimiter) {
            let token = part.trim().to_lowercase();
            match token.as_str() {
                "cmd" | "command" | "meta" | "super" | "win" => has_platform = true,
                "ctrl" | "control" => has_control = true,
                "alt" | "option" => has_alt = true,
                "shift" => has_shift = true,
                "" => {}
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

/// Returns true if the stored shortcut string matches the incoming `Keystroke`.
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

    sc_platform == ks_platform
        && sc_ctrl == ks_ctrl
        && sc_alt == ks_alt
        && sc_shift == ks_shift
        && sc_key == norm_ks
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
        } else {
            assert_eq!(formatted, "Shift+Win+R");
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

        // Does not match wrong modifiers
        assert!(!matches_keystroke("⌘R", &ks));
        assert!(!matches_keystroke("Ctrl+R", &ks));
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
        } else {
            assert!(formatted.contains("Win"));
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
        } else {
            assert_eq!(formatted, "Shift+Win+B");
        }
    }
}
