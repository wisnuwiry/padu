//! Typing operations and text mutations.

use gpui::Context;

use crate::anchor::Delta;
use crate::editor::{Editor, EditorEvent};
use crate::history::EditKind;
use crate::link::Paste;
use crate::model::{
    Block, BlockKind, Cursor, Mark, MarkSpan, Part, Selection, Splice, Text, is_url,
};
use crate::slash::Slash;

impl Editor {
    /// Insert text at the current cursor/selection.
    pub fn insert_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.history
            .record(EditKind::Insert, &self.doc, self.selection, &self.anchors);

        let (start, end) = self.selection.ordered();
        if let Some(block) = self.doc.blocks.get_mut(start.block)
            && let Some(target_text) = block.text_at_mut(start.part)
        {
            let range = start.offset..end.offset.min(target_text.len());
            target_text.text.replace_range(range.clone(), text);

            let new_offset = start.offset + text.len();
            let caret = Cursor::new(start.block, start.part, new_offset);

            let delta = Delta::Spliced(Splice {
                removed: self.selection,
                caret,
                blocks: 0,
            });
            for a in &mut self.anchors {
                a.map(&delta);
            }

            self.selection = Selection::at(caret);

            // Trigger slash menu if '/' is typed at the beginning of an empty block
            if self.chrome.slash && text == "/" && start.offset == 0 {
                self.slash = Some(Slash::open(start));
            } else if let Some(slash) = &mut self.slash {
                if let Some(query) = slash.query(caret, &target_text.text) {
                    slash.refilter(&query);
                } else {
                    self.slash = None;
                }
            }

            // Trigger link paste popup if URL is pasted
            if self.chrome.paste && is_url(text) {
                let alone = target_text.len() == text.len();
                self.paste = Some(Paste::open(caret, text.to_string(), alone));
            }

            self.history.landed(EditKind::Insert, self.selection);
            self.caret_moved();
        }

        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    /// Delete character or selection backward (Backspace).
    pub fn delete_backward(&mut self, cx: &mut Context<Self>) {
        self.history
            .record(EditKind::Delete, &self.doc, self.selection, &self.anchors);

        if self.selection.is_collapsed() {
            let caret = self.selection.head;
            if caret.offset > 0 {
                let prev = caret.left(&self.doc);
                self.selection = Selection::new(prev, caret);
            } else if caret.block > 0 {
                let prev_block_ix = caret.block - 1;
                let prev_len = self.doc.blocks[prev_block_ix]
                    .text_at(Part::Body)
                    .map(|t| t.len())
                    .unwrap_or(0);

                if let Some(removed_text) =
                    self.doc.blocks[caret.block].text_at(Part::Body).cloned()
                {
                    if let Some(prev_text) = self.doc.blocks[prev_block_ix].text_at_mut(Part::Body)
                    {
                        prev_text.text.push_str(&removed_text.text);
                    }
                    self.doc.blocks.remove(caret.block);
                    self.selection =
                        Selection::at(Cursor::new(prev_block_ix, Part::Body, prev_len));
                    self.caret_moved();
                    cx.emit(EditorEvent::Changed);
                    cx.notify();
                    return;
                }
            }
        }

        let (start, end) = self.selection.ordered();
        if let Some(block) = self.doc.blocks.get_mut(start.block)
            && let Some(text) = block.text_at_mut(start.part)
        {
            let range = start.offset..end.offset.min(text.len());
            text.text.replace_range(range, "");
            self.selection = Selection::at(start);
            self.history.landed(EditKind::Delete, self.selection);
            self.caret_moved();
        }

        self.slash = None;
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    /// Delete character or selection forward (Delete).
    pub fn delete_forward(&mut self, cx: &mut Context<Self>) {
        if self.selection.is_collapsed() {
            let caret = self.selection.head;
            let next = caret.right(&self.doc);
            if next != caret {
                self.selection = Selection::new(caret, next);
            }
        }
        self.delete_backward(cx);
    }

    /// Split block at the current cursor position (Enter).
    pub fn split_block(&mut self, cx: &mut Context<Self>) {
        if let Some(slash) = &self.slash
            && let Some(kind) = slash.choice()
        {
            self.set_block_kind(kind, cx);
            self.slash = None;
            return;
        }

        self.history.record(
            EditKind::Structure,
            &self.doc,
            self.selection,
            &self.anchors,
        );

        let caret = self.selection.head;
        if let Some(block) = self.doc.blocks.get_mut(caret.block)
            && let Some(text) = block.text_at_mut(caret.part)
        {
            let tail = if caret.offset < text.len() {
                text.text.split_off(caret.offset)
            } else {
                String::new()
            };
            let new_block = Block::at(BlockKind::Paragraph(Text::plain(tail)), block.indent);
            self.doc.blocks.insert(caret.block + 1, new_block);
            self.selection = Selection::at(Cursor::new(caret.block + 1, Part::Body, 0));
            self.history.landed(EditKind::Structure, self.selection);
            self.caret_moved();
        }

        self.slash = None;
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    /// Increase indent level of current block (Tab).
    pub fn indent(&mut self, cx: &mut Context<Self>) {
        let block_ix = self.selection.head.block;
        if let Some(block) = self.doc.blocks.get_mut(block_ix) {
            block.indent = block.indent.saturating_add(1);
            cx.emit(EditorEvent::Changed);
            cx.notify();
        }
    }

    /// Decrease indent level of current block (Shift-Tab).
    pub fn outdent(&mut self, cx: &mut Context<Self>) {
        let block_ix = self.selection.head.block;
        if let Some(block) = self.doc.blocks.get_mut(block_ix) {
            block.indent = block.indent.saturating_sub(1);
            cx.emit(EditorEvent::Changed);
            cx.notify();
        }
    }

    /// Toggle an inline styling mark (Bold, Italic, Code, Strike, Link).
    pub fn toggle_mark(&mut self, mark: Mark, cx: &mut Context<Self>) {
        if self.selection.is_collapsed() {
            return;
        }
        let (start, end) = self.selection.ordered();
        if start.block == end.block
            && let Some(block) = self.doc.blocks.get_mut(start.block)
            && let Some(text) = block.text_at_mut(start.part)
        {
            let range = start.offset..end.offset.min(text.len());
            let existing_pos = text
                .marks
                .iter()
                .position(|m| m.mark == mark && m.range == range);
            if let Some(pos) = existing_pos {
                text.marks.remove(pos);
            } else {
                text.marks.push(MarkSpan { range, mark });
            }
            cx.emit(EditorEvent::Changed);
            cx.notify();
        }
    }

    /// Move cursor left one character or boundary.
    pub fn move_left(&mut self, cx: &mut Context<Self>) {
        let prev = self.selection.head.left(&self.doc);
        self.set_selection(Selection::at(prev), cx);
    }

    /// Move cursor right one character or boundary.
    pub fn move_right(&mut self, cx: &mut Context<Self>) {
        let next = self.selection.head.right(&self.doc);
        self.set_selection(Selection::at(next), cx);
    }

    /// Move cursor up one block.
    pub fn move_up(&mut self, cx: &mut Context<Self>) {
        let head = self.selection.head;
        if head.block > 0 {
            let prev_block = head.block - 1;
            let target = Cursor::new(prev_block, head.part, head.offset).clamp(&self.doc);
            self.set_selection(Selection::at(target), cx);
        }
    }

    /// Move cursor down one block.
    pub fn move_down(&mut self, cx: &mut Context<Self>) {
        let head = self.selection.head;
        if head.block + 1 < self.doc.blocks.len() {
            let next_block = head.block + 1;
            let target = Cursor::new(next_block, head.part, head.offset).clamp(&self.doc);
            self.set_selection(Selection::at(target), cx);
        }
    }

    /// Move cursor to the beginning of the current block.
    pub fn move_home(&mut self, cx: &mut Context<Self>) {
        let head = self.selection.head;
        let target = Cursor::new(head.block, head.part, 0);
        self.set_selection(Selection::at(target), cx);
    }

    /// Move cursor to the end of the current block.
    pub fn move_end(&mut self, cx: &mut Context<Self>) {
        let head = self.selection.head;
        let len = self.doc.blocks[head.block]
            .text_at(head.part)
            .map(|t| t.len())
            .unwrap_or(0);
        let target = Cursor::new(head.block, head.part, len);
        self.set_selection(Selection::at(target), cx);
    }

    /// Extend selection left.
    pub fn select_left(&mut self, cx: &mut Context<Self>) {
        self.selection.head = self.selection.head.left(&self.doc);
        self.caret_moved();
        cx.notify();
    }

    /// Extend selection right.
    pub fn select_right(&mut self, cx: &mut Context<Self>) {
        self.selection.head = self.selection.head.right(&self.doc);
        self.caret_moved();
        cx.notify();
    }

    /// Extend selection up.
    pub fn select_up(&mut self, cx: &mut Context<Self>) {
        let head = self.selection.head;
        if head.block > 0 {
            self.selection.head =
                Cursor::new(head.block - 1, head.part, head.offset).clamp(&self.doc);
            self.caret_moved();
            cx.notify();
        }
    }

    /// Extend selection down.
    pub fn select_down(&mut self, cx: &mut Context<Self>) {
        let head = self.selection.head;
        if head.block + 1 < self.doc.blocks.len() {
            self.selection.head =
                Cursor::new(head.block + 1, head.part, head.offset).clamp(&self.doc);
            self.caret_moved();
            cx.notify();
        }
    }

    /// Extend selection to the start of the block.
    pub fn select_home(&mut self, cx: &mut Context<Self>) {
        let head = self.selection.head;
        self.selection.head = Cursor::new(head.block, head.part, 0);
        self.caret_moved();
        cx.notify();
    }

    /// Extend selection to the end of the block.
    pub fn select_end(&mut self, cx: &mut Context<Self>) {
        let head = self.selection.head;
        let len = self.doc.blocks[head.block]
            .text_at(head.part)
            .map(|t| t.len())
            .unwrap_or(0);
        self.selection.head = Cursor::new(head.block, head.part, len);
        self.caret_moved();
        cx.notify();
    }
}
