//! Mode switching between visual blocks and raw markdown source.

use gpui::Context;

use crate::editor::{Editor, EditorEvent, Mode};

impl Editor {
    /// Switch between Blocks and Source modes.
    pub fn toggle_mode(&mut self, cx: &mut Context<Self>) {
        let new_mode = match self.mode {
            Mode::Blocks => Mode::Source,
            Mode::Source => Mode::Blocks,
        };
        self.set_mode(new_mode, cx);
    }

    /// Set the presentation mode.
    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.mode != mode {
            self.mode = mode;
            self.slash = None;
            self.paste = None;
            cx.emit(EditorEvent::ModeChanged(mode));
            cx.notify();
        }
    }
}
