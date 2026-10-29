//! Editor actions and keybinding registrations.

use gpui::actions;

actions!(
    padu_editor,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        Home,
        End,
        SelectAll,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectHome,
        SelectEnd,
        ToggleBold,
        ToggleItalic,
        ToggleStrike,
        ToggleCode,
        ToggleHighlight,
        Undo,
        Redo,
        Cut,
        Copy,
        Paste,
        Indent,
        Outdent,
        SplitBlock,
        InsertParagraph,
        IncreaseTextSize,
        DecreaseTextSize,
        ResetTextSize,
        Dismiss,
    ]
);

/// Register all keybindings for the PaduEditor context.
pub fn init(cx: &mut gpui::App) {
    use gpui::KeyBinding;

    #[cfg(target_os = "macos")]
    let (primary, primary_shift) = ("cmd", "cmd-shift");
    #[cfg(not(target_os = "macos"))]
    let (primary, primary_shift) = ("ctrl", "ctrl-shift");

    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some(super::CONTEXT)),
        KeyBinding::new("delete", Delete, Some(super::CONTEXT)),
        KeyBinding::new("left", Left, Some(super::CONTEXT)),
        KeyBinding::new("right", Right, Some(super::CONTEXT)),
        KeyBinding::new("up", Up, Some(super::CONTEXT)),
        KeyBinding::new("down", Down, Some(super::CONTEXT)),
        KeyBinding::new("home", Home, Some(super::CONTEXT)),
        KeyBinding::new("end", End, Some(super::CONTEXT)),
        KeyBinding::new(&format!("{primary}-a"), SelectAll, Some(super::CONTEXT)),
        KeyBinding::new(&format!("{primary}-c"), Copy, Some(super::CONTEXT)),
        KeyBinding::new(&format!("{primary}-x"), Cut, Some(super::CONTEXT)),
        KeyBinding::new(&format!("{primary}-v"), Paste, Some(super::CONTEXT)),
        KeyBinding::new("shift-left", SelectLeft, Some(super::CONTEXT)),
        KeyBinding::new("shift-right", SelectRight, Some(super::CONTEXT)),
        KeyBinding::new("shift-up", SelectUp, Some(super::CONTEXT)),
        KeyBinding::new("shift-down", SelectDown, Some(super::CONTEXT)),
        KeyBinding::new("shift-home", SelectHome, Some(super::CONTEXT)),
        KeyBinding::new("shift-end", SelectEnd, Some(super::CONTEXT)),
        KeyBinding::new(&format!("{primary}-b"), ToggleBold, Some(super::CONTEXT)),
        KeyBinding::new(&format!("{primary}-i"), ToggleItalic, Some(super::CONTEXT)),
        KeyBinding::new(
            &format!("{primary_shift}-x"),
            ToggleStrike,
            Some(super::CONTEXT),
        ),
        KeyBinding::new(&format!("{primary}-e"), ToggleCode, Some(super::CONTEXT)),
        KeyBinding::new(
            &format!("{primary_shift}-h"),
            ToggleHighlight,
            Some(super::CONTEXT),
        ),
        KeyBinding::new(&format!("{primary}-z"), Undo, Some(super::CONTEXT)),
        KeyBinding::new(&format!("{primary_shift}-z"), Redo, Some(super::CONTEXT)),
        KeyBinding::new("tab", Indent, Some(super::CONTEXT)),
        KeyBinding::new("shift-tab", Outdent, Some(super::CONTEXT)),
        KeyBinding::new("enter", SplitBlock, Some(super::CONTEXT)),
        KeyBinding::new("shift-enter", InsertParagraph, Some(super::CONTEXT)),
        KeyBinding::new(
            &format!("{primary}-="),
            IncreaseTextSize,
            Some(super::CONTEXT),
        ),
        KeyBinding::new(
            &format!("{primary}--"),
            DecreaseTextSize,
            Some(super::CONTEXT),
        ),
        KeyBinding::new(&format!("{primary}-0"), ResetTextSize, Some(super::CONTEXT)),
        KeyBinding::new("escape", Dismiss, Some(super::CONTEXT)),
    ]);
}
