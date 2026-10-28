//! Floating menu chrome: block handle, slash command menu, and link paste popup.

use gpui::Context;

use crate::editor::Editor;
use crate::model::BlockKind;

pub const SLASH_MENU: &str = "slash-menu";
pub const BLOCK_HANDLE: &str = "block-handle";
pub const BLOCK_MENU: &str = "block-menu";

impl Editor {
    /// Dismiss any currently active popups (slash menu, paste choice, etc.).
    pub fn dismiss_popovers(&mut self, cx: &mut Context<Self>) {
        if self.slash.is_some() || self.paste.is_some() {
            self.slash = None;
            self.paste = None;
            cx.notify();
        }
    }

    /// Confirm the current slash menu selection.
    pub fn confirm_slash_choice(&mut self, kind: BlockKind, cx: &mut Context<Self>) {
        self.set_block_kind(kind, cx);
        self.slash = None;
    }
}
