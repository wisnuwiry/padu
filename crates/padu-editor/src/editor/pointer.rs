use gpui::Context;

use crate::editor::Editor;
use crate::model::{Cursor, Part, Selection};

impl Editor {
    /// Handle mouse down event at a specific block and character offset.
    pub fn on_mouse_down(
        &mut self,
        block: usize,
        part: Part,
        offset: usize,
        cx: &mut Context<Self>,
    ) {
        let cursor = Cursor::new(block, part, offset).clamp(&self.doc);
        self.selection = Selection::at(cursor);
        self.history.interrupt();
        self.caret_moved();
        self.dismiss_popovers(cx);
        cx.notify();
    }

    /// Handle drag selection to a new cursor endpoint.
    pub fn on_mouse_drag(
        &mut self,
        block: usize,
        part: Part,
        offset: usize,
        cx: &mut Context<Self>,
    ) {
        let head = Cursor::new(block, part, offset).clamp(&self.doc);
        self.selection.head = head;
        self.caret_moved();
        cx.notify();
    }
}
