//! The editing surface for GPUI over the markdown document model.

use gpui::prelude::*;
use gpui::*;
use std::ops::Range;

use crate::anchor::{Anchor, AnchorId};
use crate::history::History;
use crate::layout::Layout;
use crate::link::Paste;
use crate::model::{Block, BlockKind, Cursor, Doc, Mark, Part, Selection, Text, parse, serialize};
use crate::slash::{Slash, items, label};
use crate::text_size::{self, adjust_text_size, reset_text_size};

mod anchors;
mod blocks;
mod caret;
mod clipboard;
pub mod image;
mod input;
pub mod keys;
pub mod menu;
mod mode;
mod pointer;
mod typing;

pub use caret::BLINK_INTERVAL;
pub use image::{ImageStore, Source, set_image_store};
pub use keys::init;
pub use menu::{BLOCK_HANDLE, BLOCK_MENU, SLASH_MENU};

pub const CONTEXT: &str = "PaduEditor";
pub const HIGHLIGHT_MARK: &str = "highlight";

/// Events emitted by the editor to notify parent views.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EditorEvent {
    Changed,
    AnchorActivated(AnchorId),
    ModeChanged(Mode),
}

/// The document editing presentation mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Blocks,
    Source,
}

/// Feature flags for floating UI affordances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chrome {
    pub handle: bool,
    pub slash: bool,
    pub language: bool,
    pub paste: bool,
}

impl Default for Chrome {
    fn default() -> Self {
        Self {
            handle: true,
            slash: true,
            language: true,
            paste: true,
        }
    }
}

/// Formatting state of the current selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Formatting {
    pub mode: Mode,
    pub marks: Vec<Mark>,
    pub block: Option<SharedString>,
    pub fenceable: bool,
}

/// Vocabulary of blocks that can be created.
pub fn turns() -> Vec<(SharedString, BlockKind)> {
    items()
}

/// The stateful markdown block editor view.
pub struct Editor {
    focus_handle: FocusHandle,
    doc: Doc,
    selection: Selection,
    mode: Mode,
    chrome: Chrome,
    history: History,
    anchors: Vec<Anchor>,
    slash: Option<Slash>,
    paste: Option<Paste>,
    text_size_base: Option<f32>,
    marked: Option<Range<usize>>,
    caret_visible: bool,
    caret_epoch: usize,
    blink_task: Option<Task<()>>,
}

impl EventEmitter<EditorEvent> for Editor {}

impl Editor {
    pub fn new(content: impl AsRef<str>, cx: &mut Context<Self>) -> Self {
        let mut doc = parse(content.as_ref());
        if doc.is_empty() {
            doc.blocks
                .push(Block::new(BlockKind::Paragraph(Text::default())));
        }
        let selection = Selection::at(Cursor::default().clamp(&doc));

        let mut editor = Self {
            focus_handle: cx.focus_handle(),
            doc,
            selection,
            mode: Mode::default(),
            chrome: Chrome::default(),
            history: History::default(),
            anchors: Vec::new(),
            slash: None,
            paste: None,
            text_size_base: None,
            marked: None,
            caret_visible: true,
            caret_epoch: 0,
            blink_task: None,
        };
        editor.start_blink(cx);
        editor
    }

    /// Returns the focus handle for this editor.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn content(&self) -> String {
        serialize(&self.doc)
    }

    pub fn set_content(&mut self, content: impl AsRef<str>, cx: &mut Context<Self>) {
        let mut doc = parse(content.as_ref());
        if doc.is_empty() {
            doc.blocks
                .push(Block::new(BlockKind::Paragraph(Text::default())));
        }
        self.doc = doc;
        self.selection = self.selection.clamp(&self.doc);
        self.slash = None;
        self.paste = None;
        self.caret_moved();
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    pub fn doc_mut(&mut self) -> &mut Doc {
        &mut self.doc
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn set_selection(&mut self, selection: Selection, cx: &mut Context<Self>) {
        self.selection = selection.clamp(&self.doc);
        self.history.interrupt();
        self.caret_moved();
        cx.notify();
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn chrome(&self) -> Chrome {
        self.chrome
    }

    pub fn set_chrome(&mut self, chrome: Chrome, cx: &mut Context<Self>) {
        self.chrome = chrome;
        cx.notify();
    }

    pub fn anchors(&self) -> &[Anchor] {
        &self.anchors
    }

    pub fn set_block_kind(&mut self, kind: BlockKind, cx: &mut Context<Self>) {
        self.history.record(
            crate::history::EditKind::Structure,
            &self.doc,
            self.selection,
            &self.anchors,
        );

        let block_ix = self.selection.head.block;
        if let Some(block) = self.doc.blocks.get_mut(block_ix) {
            block.kind = kind;
            self.selection = self.selection.clamp(&self.doc);
        }

        self.slash = None;
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    pub fn formatting(&self) -> Formatting {
        let block_label = self
            .doc
            .blocks
            .get(self.selection.head.block)
            .and_then(|b| label(&b.kind));

        let marks = if let Some(b) = self.doc.blocks.get(self.selection.head.block) {
            if let Some(t) = b.text_at(self.selection.head.part) {
                let (start, end) = self.selection.ordered();
                if self.selection.is_collapsed() {
                    t.marks
                        .iter()
                        .filter(|m| {
                            m.range.contains(&self.selection.head.offset)
                                || (self.selection.head.offset > 0
                                    && m.range.end == self.selection.head.offset)
                        })
                        .map(|m| m.mark.clone())
                        .collect()
                } else {
                    t.marks
                        .iter()
                        .filter(|m| m.range.start < end.offset && m.range.end > start.offset)
                        .map(|m| m.mark.clone())
                        .collect()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        Formatting {
            mode: self.mode,
            marks,
            block: block_label,
            fenceable: !self.selection.is_collapsed(),
        }
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(step) = self.history.undo(&self.doc, self.selection, &self.anchors) {
            self.doc = step.doc;
            self.selection = step.selection;
            self.anchors = step.anchors;
            self.slash = None;
            self.paste = None;
            self.caret_moved();
            cx.emit(EditorEvent::Changed);
            cx.notify();
        }
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(step) = self.history.redo(&self.doc, self.selection, &self.anchors) {
            self.doc = step.doc;
            self.selection = step.selection;
            self.anchors = step.anchors;
            self.slash = None;
            self.paste = None;
            self.caret_moved();
            cx.emit(EditorEvent::Changed);
            cx.notify();
        }
    }

    pub fn slash_menu(&self) -> Option<&Slash> {
        self.slash.as_ref()
    }

    pub fn paste_menu(&self) -> Option<&Paste> {
        self.paste.as_ref()
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::of(cx);
        let text_size = text_size::resolve(self.text_size_base, cx);
        let is_focused = self.focus_handle.is_focused(window);
        let show_caret = is_focused && self.caret_visible;

        let input_handler = canvas(|_, _, _| (), {
            let focus = self.focus_handle.clone();
            let editor = cx.entity().clone();
            move |bounds, _, window, cx| {
                window.handle_input(&focus, ElementInputHandler::new(bounds, editor.clone()), cx);
            }
        })
        .absolute()
        .size_full();

        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &keys::Backspace, _, cx| this.delete_backward(cx)))
            .on_action(cx.listener(|this, _: &keys::Delete, _, cx| this.delete_forward(cx)))
            .on_action(cx.listener(|this, _: &keys::Left, _, cx| this.move_left(cx)))
            .on_action(cx.listener(|this, _: &keys::Right, _, cx| this.move_right(cx)))
            .on_action(cx.listener(|this, _: &keys::Up, _, cx| this.move_up(cx)))
            .on_action(cx.listener(|this, _: &keys::Down, _, cx| this.move_down(cx)))
            .on_action(cx.listener(|this, _: &keys::Home, _, cx| this.move_home(cx)))
            .on_action(cx.listener(|this, _: &keys::End, _, cx| this.move_end(cx)))
            .on_action(cx.listener(|this, _: &keys::SelectLeft, _, cx| this.select_left(cx)))
            .on_action(cx.listener(|this, _: &keys::SelectRight, _, cx| this.select_right(cx)))
            .on_action(cx.listener(|this, _: &keys::SelectUp, _, cx| this.select_up(cx)))
            .on_action(cx.listener(|this, _: &keys::SelectDown, _, cx| this.select_down(cx)))
            .on_action(cx.listener(|this, _: &keys::SelectHome, _, cx| this.select_home(cx)))
            .on_action(cx.listener(|this, _: &keys::SelectEnd, _, cx| this.select_end(cx)))
            .on_action(
                cx.listener(|this, _: &keys::ToggleBold, _, cx| this.toggle_mark(Mark::Bold, cx)),
            )
            .on_action(
                cx.listener(|this, _: &keys::ToggleItalic, _, cx| {
                    this.toggle_mark(Mark::Italic, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &keys::ToggleStrike, _, cx| {
                    this.toggle_mark(Mark::Strike, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &keys::ToggleCode, _, cx| this.toggle_mark(Mark::Code, cx)),
            )
            .on_action(cx.listener(|this, _: &keys::SplitBlock, _, cx| this.split_block(cx)))
            .on_action(cx.listener(|this, _: &keys::Indent, _, cx| this.indent(cx)))
            .on_action(cx.listener(|this, _: &keys::Outdent, _, cx| this.outdent(cx)))
            .on_action(cx.listener(|this, _: &keys::Undo, _, cx| this.undo(cx)))
            .on_action(cx.listener(|this, _: &keys::Redo, _, cx| this.redo(cx)))
            .on_action(cx.listener(|this, _: &keys::Cut, _, cx| this.cut(cx)))
            .on_action(cx.listener(|this, _: &keys::Copy, _, cx| this.copy(cx)))
            .on_action(cx.listener(|this, _: &keys::Paste, _, cx| this.paste(cx)))
            .on_action(cx.listener(|this, _: &keys::SelectAll, _, cx| this.select_all(cx)))
            .on_action(cx.listener(|this, _: &keys::Dismiss, _, cx| this.dismiss_popovers(cx)))
            .on_action(
                cx.listener(|_, _: &keys::IncreaseTextSize, _, cx| adjust_text_size(cx, 1.0)),
            )
            .on_action(
                cx.listener(|_, _: &keys::DecreaseTextSize, _, cx| adjust_text_size(cx, -1.0)),
            )
            .on_action(cx.listener(|_, _: &keys::ResetTextSize, _, cx| reset_text_size(cx)))
            .relative()
            .w_full()
            .min_h_full()
            .px(px(layout.text_inset))
            .text_size(px(text_size))
            .child(input_handler)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    window.focus(&this.focus_handle, cx);
                    let last_block = this.doc.blocks.len().saturating_sub(1);
                    let last_len = this
                        .doc
                        .blocks
                        .get(last_block)
                        .and_then(|b| b.text_at(Part::Body))
                        .map(|t| t.len())
                        .unwrap_or(0);
                    this.on_mouse_down(last_block, Part::Body, last_len, cx);
                }),
            )
            .children(self.doc.blocks.iter().enumerate().map(|(ix, block)| {
                let is_caret_block = self.selection.head.block == ix;
                let selection = self.selection;

                let render_text_with_caret = |t: &str, part: Part| -> AnyElement {
                    let is_active_part = is_caret_block && selection.head.part == part;
                    let (sel_start, sel_end) = selection.ordered();
                    let has_selection = !selection.is_collapsed()
                        && is_focused
                        && sel_start.block <= ix
                        && sel_end.block >= ix;

                    let sel_range = if has_selection {
                        let start = if sel_start.block < ix || sel_start.part < part {
                            0
                        } else if sel_start.block == ix && sel_start.part == part {
                            sel_start.offset.min(t.len())
                        } else {
                            t.len()
                        };
                        let end = if sel_end.block > ix || sel_end.part > part {
                            t.len()
                        } else if sel_end.block == ix && sel_end.part == part {
                            sel_end.offset.min(t.len())
                        } else {
                            0
                        };
                        if start < end { Some(start..end) } else { None }
                    } else {
                        None
                    };

                    let caret_offset = if is_active_part && show_caret && selection.is_collapsed() {
                        Some(selection.head.offset.min(t.len()))
                    } else {
                        None
                    };

                    let caret_el = || {
                        div()
                            .w(px(2.0))
                            .h(px(text_size * 1.2))
                            .my(px(-2.0))
                            .bg(hsla(0.58, 0.9, 0.6, 1.0))
                    };

                    if let Some(range) = sel_range {
                        let before = &t[..range.start];
                        let selected = &t[range.clone()];
                        let after = &t[range.end..];
                        div()
                            .child(before.to_string())
                            .child(
                                div()
                                    .bg(hsla(0.58, 0.8, 0.5, 0.3))
                                    .rounded(px(2.0))
                                    .child(selected.to_string()),
                            )
                            .child(after.to_string())
                            .into_any_element()
                    } else if let Some(offset) = caret_offset {
                        let safe_offset = if t.is_char_boundary(offset) {
                            offset
                        } else {
                            let mut o = offset;
                            while o > 0 && !t.is_char_boundary(o) {
                                o -= 1;
                            }
                            o
                        };
                        let before = &t[..safe_offset];
                        let after = &t[safe_offset..];
                        div()
                            .child(before.to_string())
                            .child(caret_el())
                            .child(after.to_string())
                            .into_any_element()
                    } else {
                        div().child(t.to_string()).into_any_element()
                    }
                };

                div()
                    .id(("block", ix))
                    .w_full()
                    .py(px(2.0))
                    .when(block.indent > 0, |d| d.pl(px(16.0 * block.indent as f32)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            window.focus(&this.focus_handle, cx);
                            this.on_mouse_down(ix, Part::Body, 0, cx);
                        }),
                    )
                    .child(match &block.kind {
                        BlockKind::Heading { level, text } => {
                            let size = match level {
                                1 => px(text_size * 1.6),
                                2 => px(text_size * 1.3),
                                _ => px(text_size * 1.1),
                            };
                            div()
                                .text_size(size)
                                .font_weight(FontWeight::BOLD)
                                .child(render_text_with_caret(&text.text, Part::Body))
                                .into_any_element()
                        }
                        BlockKind::Bullet(text) => div()
                            .flex()
                            .items_start()
                            .child("• ")
                            .child(render_text_with_caret(&text.text, Part::Body))
                            .into_any_element(),
                        BlockKind::Ordered { number, text } => div()
                            .flex()
                            .items_start()
                            .child(format!("{number}. "))
                            .child(render_text_with_caret(&text.text, Part::Body))
                            .into_any_element(),
                        BlockKind::Task { checked, text } => div()
                            .flex()
                            .items_start()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .cursor_pointer()
                                    .child(if *checked { "[x] " } else { "[ ] " })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.toggle_task(ix, cx);
                                        }),
                                    ),
                            )
                            .child(render_text_with_caret(&text.text, Part::Body))
                            .into_any_element(),
                        BlockKind::Quote { kind, text } => {
                            let prefix =
                                kind.map_or("".to_string(), |k| format!("{} ", k.marker()));
                            div()
                                .pl(px(8.0))
                                .child(format!("{prefix}"))
                                .child(render_text_with_caret(&text.text, Part::Body))
                                .into_any_element()
                        }
                        BlockKind::Code { language, code } => div()
                            .p(px(6.0))
                            .rounded(px(4.0))
                            .child(language.clone().unwrap_or_default())
                            .child(render_text_with_caret(&code.text, Part::Code))
                            .into_any_element(),
                        BlockKind::Rule => div().h(px(1.0)).w_full().into_any_element(),
                        BlockKind::Paragraph(text) => {
                            if text.is_empty() && is_caret_block {
                                if show_caret {
                                    div()
                                        .flex()
                                        .items_center()
                                        .child(
                                            div()
                                                .w(px(2.0))
                                                .h(px(text_size * 1.2))
                                                .bg(hsla(0.58, 0.9, 0.6, 1.0)),
                                        )
                                        .child(
                                            div()
                                                .pl(px(2.0))
                                                .opacity(0.4)
                                                .child("Type / for commands"),
                                        )
                                        .into_any_element()
                                } else {
                                    div()
                                        .opacity(0.5)
                                        .child("Type / for commands")
                                        .into_any_element()
                                }
                            } else {
                                render_text_with_caret(&text.text, Part::Body)
                            }
                        }
                        _ => {
                            let text = block
                                .text_at(Part::Body)
                                .map(|t| t.text.as_str())
                                .unwrap_or("");
                            render_text_with_caret(text, Part::Body)
                        }
                    })
            }))
    }
}
