//! Anchor tracking, activation, and comment hit-testing.

use gpui::Context;

use crate::anchor::{Anchor, AnchorId};
use crate::editor::{Editor, EditorEvent};
use crate::model::Cursor;

impl Editor {
    /// Add an anchor to the editor.
    pub fn add_anchor(&mut self, anchor: Anchor) {
        self.anchors.push(anchor);
    }

    /// Remove an anchor by its ID.
    pub fn remove_anchor(&mut self, id: AnchorId) {
        self.anchors.retain(|a| a.id != id);
    }

    /// Activate an anchor, emitting `AnchorActivated`.
    pub fn activate_anchor(&mut self, id: AnchorId, cx: &mut Context<Self>) {
        cx.emit(EditorEvent::AnchorActivated(id));
        cx.notify();
    }

    /// Find an anchor covering the given cursor.
    pub fn anchor_at(&self, cursor: Cursor) -> Option<&Anchor> {
        self.anchors.iter().find(|a| {
            let (start, end) = a.range.ordered();
            start <= cursor && cursor <= end
        })
    }
}
