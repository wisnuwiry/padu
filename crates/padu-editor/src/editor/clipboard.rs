use gpui::{ClipboardItem, Context};

use crate::editor::Editor;
use crate::link::{Choice, Paste};
use crate::model::{BlockKind, Cursor, Form, Mark, MarkSpan, Selection, Text, is_url, serialize};

impl Editor {
    /// Select the entire document.
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.selection = Selection::all(&self.doc);
        self.history.interrupt();
        self.caret_moved();
        cx.notify();
    }

    /// Extract markdown source representation of the selected range.
    pub fn selected_source(&self) -> Option<String> {
        if self.selection.is_collapsed() {
            return None;
        }
        let (start, end) = self.selection.ordered();
        if start.block == end.block {
            let block = &self.doc.blocks[start.block];
            if let Some(text) = block.text_at(start.part) {
                let range = start.offset..end.offset.min(text.len());
                return Some(text.text.get(range)?.to_string());
            }
        }
        // Multi-block selection serialization
        let sub_doc = crate::model::Doc {
            blocks: self.doc.blocks[start.block..=end.block.min(self.doc.blocks.len() - 1)]
                .to_vec(),
        };
        Some(serialize(&sub_doc))
    }

    /// Copy selected text to the clipboard.
    pub fn copy(&mut self, cx: &mut Context<Self>) {
        if let Some(source) = self.selected_source() {
            cx.write_to_clipboard(ClipboardItem::new_string(source));
        }
    }

    /// Cut selected text to the clipboard.
    pub fn cut(&mut self, cx: &mut Context<Self>) {
        let Some(source) = self.selected_source() else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(source));
        self.delete_backward(cx);
    }

    /// Paste from the system clipboard into the editor.
    pub fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        let Some(text) = item.text() else {
            return;
        };
        let trimmed = text.trim();
        if is_url(trimmed) {
            self.paste_url(trimmed.to_string(), cx);
        } else {
            self.paste_literal(&text, cx);
        }
    }

    /// Paste plain text into the current selection.
    pub fn paste_literal(&mut self, text: &str, cx: &mut Context<Self>) {
        self.insert_text(text, cx);
    }

    /// Paste a URL, converting to a link or offering a rich card popup.
    pub fn paste_url(&mut self, url: String, cx: &mut Context<Self>) {
        if !self.selection.is_collapsed() {
            self.toggle_mark(Mark::Link(url), cx);
            return;
        }

        let at = self.selection.head;
        let alone = self
            .doc
            .blocks
            .get(at.block)
            .and_then(|b| b.text_at(at.part))
            .map(|t| t.is_empty())
            .unwrap_or(false);

        self.insert_text(&url, cx);

        if self.chrome.paste {
            self.paste = Some(Paste::open(at, url, alone));
            cx.notify();
        }
    }

    /// Confirm and apply a paste popup choice (Chip, Bookmark, Embed, Image).
    pub fn confirm_paste(&mut self, choice: Choice, cx: &mut Context<Self>) {
        let Some(pasted) = self.paste.take() else {
            return;
        };
        let ix = pasted.at.block;
        match choice {
            Choice::Dismiss => cx.notify(),
            Choice::Chip => {
                let end = Cursor {
                    offset: pasted.at.offset + pasted.url.len(),
                    ..pasted.at
                };
                if let Some(block) = self.doc.blocks.get_mut(ix)
                    && let Some(text) = block.text_at_mut(pasted.at.part)
                {
                    text.marks.push(MarkSpan {
                        range: pasted.at.offset..end.offset,
                        mark: Mark::Mention {
                            url: pasted.url,
                            form: Form::Chip,
                        },
                    });
                }
                cx.notify();
            }
            Choice::Bookmark => self.set_block_kind(
                BlockKind::Bookmark {
                    url: pasted.url,
                    form: Form::Auto,
                },
                cx,
            ),
            Choice::Embed => self.set_block_kind(
                BlockKind::Bookmark {
                    url: pasted.url,
                    form: Form::Embed,
                },
                cx,
            ),
            Choice::Image => self.set_block_kind(
                BlockKind::Image {
                    url: pasted.url,
                    alt: Text::default(),
                    width: None,
                },
                cx,
            ),
        }
    }
}
