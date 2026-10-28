//! Platform IME and system text input handler integration.

use std::ops::Range;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};

use crate::editor::Editor;
use crate::model::{Cursor, Selection, is_url};

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let block_ix = self.selection.head.block;
        let part = self.selection.head.part;
        let text = self.doc.blocks.get(block_ix)?.text_at(part)?;
        let start = range.start.min(text.len());
        let end = range.end.min(text.len());
        *adjusted = Some(start..end);
        Some(text.text.get(start..end)?.to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let (start, end) = self.selection.ordered();
        if start.block == end.block && start.part == end.part {
            Some(UTF16Selection {
                range: start.offset..end.offset,
                reversed: self.selection.head < self.selection.anchor,
            })
        } else {
            let offset = self.selection.head.offset;
            Some(UTF16Selection {
                range: offset..offset,
                reversed: false,
            })
        }
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone()
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if range.is_none() && self.marked.is_none() && is_url(text.trim()) {
            self.paste_url(text.trim().to_string(), cx);
            return;
        }

        if let Some(r) = range {
            let head = self.selection.head;
            self.selection = Selection::new(
                Cursor::new(head.block, head.part, r.start),
                Cursor::new(head.block, head.part, r.end),
            );
        }
        self.marked = None;
        self.insert_text(text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        marked: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(r) = range {
            let head = self.selection.head;
            self.selection = Selection::new(
                Cursor::new(head.block, head.part, r.start),
                Cursor::new(head.block, head.part, r.end),
            );
        }
        let base = self.selection.ordered().0.offset;
        self.insert_text(text, cx);
        self.marked = marked.map(|m| (base + m.start)..(base + m.end));
        self.caret_moved();
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range: Range<usize>,
        _bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
