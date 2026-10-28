use gpui::Context;

use crate::editor::Editor;
use crate::history::EditKind;
use crate::model::{Align, Block, BlockKind, Cursor, Part, Selection, Text};

impl Editor {
    /// Move block at `ix` up one position in document order.
    pub fn move_block_up(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix == 0 || ix >= self.doc.blocks.len() {
            return;
        }
        self.history.record(
            EditKind::Structure,
            &self.doc,
            self.selection,
            &self.anchors,
        );

        let block = self.doc.blocks.remove(ix);
        self.doc.blocks.insert(ix - 1, block);

        if self.selection.head.block == ix {
            self.selection.head.block = ix - 1;
            self.selection.anchor.block = ix - 1;
        }

        cx.emit(crate::editor::EditorEvent::Changed);
        cx.notify();
    }

    /// Move block at `ix` down one position in document order.
    pub fn move_block_down(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix + 1 >= self.doc.blocks.len() {
            return;
        }
        self.history.record(
            EditKind::Structure,
            &self.doc,
            self.selection,
            &self.anchors,
        );

        let block = self.doc.blocks.remove(ix);
        self.doc.blocks.insert(ix + 1, block);

        if self.selection.head.block == ix {
            self.selection.head.block = ix + 1;
            self.selection.anchor.block = ix + 1;
        }

        cx.emit(crate::editor::EditorEvent::Changed);
        cx.notify();
    }

    /// Duplicate the block at `ix`.
    pub fn duplicate_block(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix >= self.doc.blocks.len() {
            return;
        }
        self.history.record(
            EditKind::Structure,
            &self.doc,
            self.selection,
            &self.anchors,
        );

        let clone = self.doc.blocks[ix].clone();
        self.doc.blocks.insert(ix + 1, clone);

        self.selection = Selection::at(Cursor::new(ix + 1, Part::Body, 0));
        cx.emit(crate::editor::EditorEvent::Changed);
        cx.notify();
    }

    /// Remove the block at `ix`.
    pub fn remove_block(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix >= self.doc.blocks.len() {
            return;
        }
        self.history.record(
            EditKind::Structure,
            &self.doc,
            self.selection,
            &self.anchors,
        );

        self.doc.blocks.remove(ix);
        if self.doc.is_empty() {
            self.doc
                .blocks
                .push(Block::new(BlockKind::Paragraph(Text::default())));
        }

        let new_block = ix.min(self.doc.blocks.len() - 1);
        self.selection = Selection::at(Cursor::new(new_block, Part::Body, 0));

        cx.emit(crate::editor::EditorEvent::Changed);
        cx.notify();
    }

    /// Toggle task checkbox at `ix`.
    pub fn toggle_task(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(block) = self.doc.blocks.get_mut(ix) else {
            return;
        };
        if let BlockKind::Task { checked, .. } = &mut block.kind {
            *checked = !*checked;
            cx.emit(crate::editor::EditorEvent::Changed);
            cx.notify();
        }
    }

    /// Insert an empty row in table at `ix`.
    pub fn add_row(&mut self, ix: usize, row_idx: usize, cx: &mut Context<Self>) {
        let Some(block) = self.doc.blocks.get_mut(ix) else {
            return;
        };
        if let BlockKind::Table { header, rows, .. } = &mut block.kind {
            let cols = header.len();
            let new_row = vec![Text::default(); cols];
            let pos = row_idx.min(rows.len());
            rows.insert(pos, new_row);
            cx.emit(crate::editor::EditorEvent::Changed);
            cx.notify();
        }
    }

    /// Insert an empty column in table at `ix`.
    pub fn add_column(&mut self, ix: usize, col_idx: usize, cx: &mut Context<Self>) {
        let Some(block) = self.doc.blocks.get_mut(ix) else {
            return;
        };
        if let BlockKind::Table {
            align,
            header,
            rows,
        } = &mut block.kind
        {
            let col = col_idx.min(header.len());
            header.insert(col, Text::default());
            align.insert(col, Align::Left);
            for r in rows.iter_mut() {
                r.insert(col, Text::default());
            }
            cx.emit(crate::editor::EditorEvent::Changed);
            cx.notify();
        }
    }

    /// Remove a row from table at `ix`.
    pub fn remove_row(&mut self, ix: usize, row_idx: usize, cx: &mut Context<Self>) {
        let Some(block) = self.doc.blocks.get_mut(ix) else {
            return;
        };
        if let BlockKind::Table { rows, .. } = &mut block.kind
            && row_idx < rows.len()
        {
            rows.remove(row_idx);
            cx.emit(crate::editor::EditorEvent::Changed);
            cx.notify();
        }
    }

    /// Remove a column from table at `ix`.
    pub fn remove_column(&mut self, ix: usize, col_idx: usize, cx: &mut Context<Self>) {
        let Some(block) = self.doc.blocks.get_mut(ix) else {
            return;
        };
        if let BlockKind::Table {
            align,
            header,
            rows,
        } = &mut block.kind
            && header.len() > 1
            && col_idx < header.len()
        {
            header.remove(col_idx);
            if col_idx < align.len() {
                align.remove(col_idx);
            }
            for r in rows.iter_mut() {
                if col_idx < r.len() {
                    r.remove(col_idx);
                }
            }
            cx.emit(crate::editor::EditorEvent::Changed);
            cx.notify();
        }
    }
}
