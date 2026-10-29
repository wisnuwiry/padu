//! The editing surface for GPUI over the markdown document model.

use gpui::prelude::*;
use gpui::*;
use std::collections::HashMap;
use std::ops::{DerefMut, Range};
use std::rc::Rc;

use crate::anchor::{Anchor, AnchorId};
use crate::history::History;
use crate::layout::Layout;
use crate::link::Paste;
use crate::model::{
    Block, BlockKind, Cursor, Doc, Mark, Part, QuoteKind, Selection, Text, parse, serialize,
};
use crate::slash::{Slash, items, label};
use crate::text_size::{self, adjust_text_size, reset_text_size};
use crate::theme::EditorTheme;

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
pub const MONO_FONT: &str = "JetBrains Mono";

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

/// The presentation mode of a code or previewable block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BlockPreviewMode {
    #[default]
    Code,
    Preview,
    Split,
}

/// A custom renderer function that can render a preview or component for a block.
pub type CustomBlockRenderer = Rc<
    dyn Fn(
            usize,            // block_ix
            &Block,           // block
            BlockPreviewMode, // preview_mode
            &mut Window,
            &mut App,
        ) -> Option<AnyElement>
        + 'static,
>;

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
    theme: EditorTheme,
    block_preview_modes: HashMap<usize, BlockPreviewMode>,
    custom_preview_renderer: Option<CustomBlockRenderer>,
    copied_block_ix: Option<usize>,
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
            theme: EditorTheme::default(),
            block_preview_modes: HashMap::new(),
            custom_preview_renderer: None,
            copied_block_ix: None,
        };
        editor.start_blink(cx);
        editor
    }

    pub fn set_theme(&mut self, theme: EditorTheme, cx: &mut Context<Self>) {
        if self.theme != theme {
            self.theme = theme;
            cx.notify();
        }
    }

    pub fn theme(&self) -> EditorTheme {
        self.theme
    }

    pub fn set_custom_preview_renderer(
        &mut self,
        renderer: Option<CustomBlockRenderer>,
        cx: &mut Context<Self>,
    ) {
        self.custom_preview_renderer = renderer;
        cx.notify();
    }

    pub fn has_custom_preview_renderer(&self) -> bool {
        self.custom_preview_renderer.is_some()
    }

    /// Focus the editor and place the caret at the end of the document.
    ///
    /// If the document ends with a non-text block (Code, Rule, Table), appends
    /// an empty paragraph so the user can immediately type, matching Notion behavior.
    pub fn focus_end(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
        if self.doc.blocks.is_empty() {
            self.doc
                .blocks
                .push(Block::new(BlockKind::Paragraph(Text::default())));
        }
        let last_ix = self.doc.blocks.len().saturating_sub(1);
        let should_append_paragraph = matches!(
            self.doc.blocks.get(last_ix).map(|b| &b.kind),
            Some(BlockKind::Code { .. } | BlockKind::Rule | BlockKind::Table { .. })
        );
        let target_block = if should_append_paragraph {
            self.doc
                .blocks
                .push(Block::new(BlockKind::Paragraph(Text::default())));
            cx.emit(EditorEvent::Changed);
            self.doc.blocks.len() - 1
        } else {
            last_ix
        };

        let part = match self.doc.blocks.get(target_block).map(|b| &b.kind) {
            Some(BlockKind::Code { .. }) => Part::Code,
            _ => Part::Body,
        };
        let last_len = self
            .doc
            .blocks
            .get(target_block)
            .and_then(|b| b.text_at(part))
            .map(|t| t.len())
            .unwrap_or(0);
        self.on_mouse_down(target_block, part, last_len, cx);
    }

    pub fn block_preview_mode(&self, block_ix: usize) -> BlockPreviewMode {
        if let Some(&mode) = self.block_preview_modes.get(&block_ix) {
            return mode;
        }
        if let Some(block) = self.doc.blocks.get(block_ix) {
            if let BlockKind::Code {
                language: Some(lang),
                ..
            } = &block.kind
            {
                let l = lang.trim().to_ascii_lowercase();
                if matches!(l.as_str(), "mermaid" | "latex" | "tex" | "math") {
                    return BlockPreviewMode::Preview;
                }
            }
        }
        BlockPreviewMode::Code
    }

    pub fn set_block_preview_mode(
        &mut self,
        block_ix: usize,
        mode: BlockPreviewMode,
        cx: &mut Context<Self>,
    ) {
        self.block_preview_modes.insert(block_ix, mode);
        cx.notify();
    }

    pub fn copy_block_code(&mut self, block_ix: usize, cx: &mut Context<Self>) {
        if let Some(block) = self.doc.blocks.get(block_ix) {
            if let BlockKind::Code { code, .. } = &block.kind {
                cx.write_to_clipboard(ClipboardItem::new_string(code.text.clone()));
                self.copied_block_ix = Some(block_ix);
                cx.notify();
                cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(2))
                        .await;
                    let _ = this.update(cx, |this, cx| {
                        if this.copied_block_ix == Some(block_ix) {
                            this.copied_block_ix = None;
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
        }
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
        let theme = self.theme;

        let input_handler = canvas(|_, _, _| (), {
            let focus = self.focus_handle.clone();
            let editor = cx.entity().clone();
            move |bounds, _, window, cx| {
                window.handle_input(&focus, ElementInputHandler::new(bounds, editor.clone()), cx);
            }
        })
        .absolute()
        .size_full();

        let custom_renderer = self.custom_preview_renderer.clone();
        let selection = self.selection;
        let mut block_elements = Vec::with_capacity(self.doc.blocks.len());

        for (ix, block) in self.doc.blocks.iter().enumerate() {
            let is_caret_block = selection.head.block == ix;

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
                        .bg(theme.accent)
                };

                if let Some(range) = sel_range {
                    let before = &t[..range.start];
                    let selected = &t[range.clone()];
                    let after = &t[range.end..];
                    div()
                        .flex()
                        .items_center()
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
                        .flex()
                        .items_center()
                        .child(before.to_string())
                        .child(caret_el())
                        .child(after.to_string())
                        .into_any_element()
                } else {
                    div().child(t.to_string()).into_any_element()
                }
            };

            let block_el: AnyElement = match &block.kind {
                BlockKind::Heading { level, text } => {
                    let (size, weight, mt, mb, placeholder) = match level {
                        1 => (
                            text_size * 1.75,
                            FontWeight::BOLD,
                            px(16.0),
                            px(4.0),
                            "Heading 1",
                        ),
                        2 => (
                            text_size * 1.4,
                            FontWeight::BOLD,
                            px(12.0),
                            px(3.0),
                            "Heading 2",
                        ),
                        _ => (
                            text_size * 1.18,
                            FontWeight::SEMIBOLD,
                            px(8.0),
                            px(2.0),
                            "Heading 3",
                        ),
                    };
                    let content = if text.is_empty() && is_caret_block {
                        if show_caret {
                            div()
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .w(px(2.0))
                                        .h(px(size * 1.1))
                                        .my(px(-2.0))
                                        .bg(theme.accent),
                                )
                                .child(
                                    div()
                                        .pl(px(2.0))
                                        .text_color(theme.text_ghost)
                                        .child(placeholder),
                                )
                                .into_any_element()
                        } else {
                            div()
                                .text_color(theme.text_ghost)
                                .child(placeholder)
                                .into_any_element()
                        }
                    } else {
                        render_text_with_caret(&text.text, Part::Body)
                    };
                    div()
                        .w_full()
                        .mt(mt)
                        .mb(mb)
                        .text_size(px(size))
                        .font_weight(weight)
                        .text_color(theme.text)
                        .child(content)
                        .into_any_element()
                }
                BlockKind::Bullet(text) => div()
                    .flex()
                    .items_start()
                    .py(px(2.0))
                    .child(
                        div()
                            .w(px(18.0))
                            .flex_none()
                            .text_color(theme.text_secondary)
                            .child("•"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(theme.text)
                            .child(render_text_with_caret(&text.text, Part::Body)),
                    )
                    .into_any_element(),
                BlockKind::Ordered { number, text } => div()
                    .flex()
                    .items_start()
                    .py(px(2.0))
                    .child(
                        div()
                            .min_w(px(22.0))
                            .flex_none()
                            .text_color(theme.text_secondary)
                            .child(format!("{number}. ")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(theme.text)
                            .child(render_text_with_caret(&text.text, Part::Body)),
                    )
                    .into_any_element(),
                BlockKind::Task { checked, text } => {
                    let checked = *checked;
                    div()
                        .flex()
                        .items_start()
                        .gap(px(8.0))
                        .py(px(2.0))
                        .child(
                            div()
                                .flex_none()
                                .mt(px(2.0))
                                .size(px(16.0))
                                .rounded(px(4.0))
                                .border_1()
                                .cursor_pointer()
                                .when(checked, |s| {
                                    s.bg(theme.accent)
                                        .border_color(theme.accent)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(theme.surface)
                                                .font_weight(FontWeight::BOLD)
                                                .child("✓"),
                                        )
                                })
                                .when(!checked, |s| {
                                    s.bg(theme.inset)
                                        .border_color(theme.border)
                                        .hover(|s| s.border_color(theme.accent))
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.toggle_task(ix, cx);
                                        cx.stop_propagation();
                                    }),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .when(checked, |s| s.line_through().text_color(theme.text_ghost))
                                .when(!checked, |s| s.text_color(theme.text))
                                .child(render_text_with_caret(&text.text, Part::Body)),
                        )
                        .into_any_element()
                }
                BlockKind::Quote { kind, text } => match kind {
                    None => div()
                        .pl(px(14.0))
                        .border_l_3()
                        .border_color(theme.accent)
                        .my(px(4.0))
                        .italic()
                        .text_color(theme.text_secondary)
                        .child(render_text_with_caret(&text.text, Part::Body))
                        .into_any_element(),
                    Some(qk) => {
                        let (icon, bg_color, border_color) = match qk {
                            QuoteKind::Note => {
                                ("ℹ", hsla(0.58, 0.6, 0.5, 0.08), hsla(0.58, 0.6, 0.5, 0.25))
                            }
                            QuoteKind::Tip => (
                                "💡",
                                hsla(0.35, 0.6, 0.45, 0.08),
                                hsla(0.35, 0.6, 0.45, 0.25),
                            ),
                            QuoteKind::Important => {
                                ("⚠️", hsla(0.75, 0.6, 0.5, 0.08), hsla(0.75, 0.6, 0.5, 0.25))
                            }
                            QuoteKind::Warning => {
                                ("⚡", hsla(0.1, 0.7, 0.5, 0.08), hsla(0.1, 0.7, 0.5, 0.25))
                            }
                            QuoteKind::Caution => {
                                ("🛑", hsla(0.0, 0.7, 0.5, 0.08), hsla(0.0, 0.7, 0.5, 0.25))
                            }
                        };
                        div()
                            .w_full()
                            .my(px(6.0))
                            .p(px(12.0))
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(border_color)
                            .bg(bg_color)
                            .flex()
                            .items_start()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .flex_none()
                                    .size(px(22.0))
                                    .rounded(px(4.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(13.0))
                                    .child(icon),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_color(theme.text)
                                    .child(render_text_with_caret(&text.text, Part::Body)),
                            )
                            .into_any_element()
                    }
                },
                BlockKind::Code { language, code } => {
                    let mode = self.block_preview_mode(ix);
                    let lang_str = language.as_deref().unwrap_or("").trim();
                    let is_mermaid = lang_str.eq_ignore_ascii_case("mermaid");
                    let is_math = matches!(
                        lang_str.to_ascii_lowercase().as_str(),
                        "latex" | "tex" | "math"
                    );
                    let display_name: SharedString = if is_mermaid {
                        "Mermaid Diagram".into()
                    } else if is_math {
                        "Math (LaTeX)".into()
                    } else if lang_str.is_empty() {
                        "Code".into()
                    } else {
                        lang_str.to_string().into()
                    };

                    let icon_str = if is_mermaid {
                        "📊"
                    } else if is_math {
                        "∑"
                    } else {
                        "</>"
                    };

                    let is_copied = self.copied_block_ix == Some(ix);

                    let header = div()
                        .w_full()
                        .h(px(32.0))
                        .px(px(10.0))
                        .flex()
                        .items_center()
                        .justify_between()
                        .border_b_1()
                        .border_color(theme.border)
                        .bg(theme.overlay)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(theme.accent)
                                        .font_weight(FontWeight::BOLD)
                                        .child(icon_str),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text_secondary)
                                        .child(display_name),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(2.0))
                                        .p(px(2.0))
                                        .rounded(px(5.0))
                                        .bg(theme.inset)
                                        .child(
                                            div()
                                                .px(px(6.0))
                                                .py(px(2.0))
                                                .rounded(px(4.0))
                                                .text_size(px(11.0))
                                                .cursor_pointer()
                                                .when(mode == BlockPreviewMode::Code, |s| {
                                                    s.bg(theme.overlay)
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(theme.text)
                                                })
                                                .when(mode != BlockPreviewMode::Code, |s| {
                                                    s.text_color(theme.text_ghost)
                                                        .hover(|s| s.text_color(theme.text))
                                                })
                                                .child("Code")
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.set_block_preview_mode(
                                                            ix,
                                                            BlockPreviewMode::Code,
                                                            cx,
                                                        );
                                                        cx.stop_propagation();
                                                    }),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .px(px(6.0))
                                                .py(px(2.0))
                                                .rounded(px(4.0))
                                                .text_size(px(11.0))
                                                .cursor_pointer()
                                                .when(mode == BlockPreviewMode::Preview, |s| {
                                                    s.bg(theme.overlay)
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(theme.text)
                                                })
                                                .when(mode != BlockPreviewMode::Preview, |s| {
                                                    s.text_color(theme.text_ghost)
                                                        .hover(|s| s.text_color(theme.text))
                                                })
                                                .child("Preview")
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.set_block_preview_mode(
                                                            ix,
                                                            BlockPreviewMode::Preview,
                                                            cx,
                                                        );
                                                        cx.stop_propagation();
                                                    }),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .px(px(6.0))
                                                .py(px(2.0))
                                                .rounded(px(4.0))
                                                .text_size(px(11.0))
                                                .cursor_pointer()
                                                .when(mode == BlockPreviewMode::Split, |s| {
                                                    s.bg(theme.overlay)
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(theme.text)
                                                })
                                                .when(mode != BlockPreviewMode::Split, |s| {
                                                    s.text_color(theme.text_ghost)
                                                        .hover(|s| s.text_color(theme.text))
                                                })
                                                .child("Split")
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.set_block_preview_mode(
                                                            ix,
                                                            BlockPreviewMode::Split,
                                                            cx,
                                                        );
                                                        cx.stop_propagation();
                                                    }),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .px(px(6.0))
                                        .py(px(2.0))
                                        .rounded(px(4.0))
                                        .text_size(px(11.0))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.inset))
                                        .text_color(if is_copied {
                                            theme.accent
                                        } else {
                                            theme.text_ghost
                                        })
                                        .child(if is_copied { "✓ Copied" } else { "Copy" })
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.copy_block_code(ix, cx);
                                                cx.stop_propagation();
                                            }),
                                        ),
                                ),
                        );

                    let code_str = &code.text;
                    let lines: Vec<&str> = code_str.split('\n').collect();
                    let line_count = lines.len().max(1);
                    let gutter_width = 24.0 + (line_count.to_string().len() as f32 * 8.0);
                    let code_line_height = text_size * 1.35;

                    let is_active_block = is_caret_block && selection.head.part == Part::Code;
                    let (sel_start, sel_end) = selection.ordered();
                    let has_sel = !selection.is_collapsed()
                        && is_focused
                        && sel_start.block <= ix
                        && sel_end.block >= ix;

                    let mut offset_acc = 0;
                    let mut line_elements = Vec::new();

                    for (line_num, line_str) in lines.iter().enumerate() {
                        let line_start = offset_acc;
                        let line_end = line_start + line_str.len();
                        offset_acc = line_end + 1;

                        let line_sel = if has_sel {
                            let s = if sel_start.block < ix
                                || (sel_start.block == ix && sel_start.part < Part::Code)
                            {
                                0
                            } else if sel_start.block == ix && sel_start.part == Part::Code {
                                sel_start.offset.clamp(line_start, line_end) - line_start
                            } else {
                                line_str.len()
                            };
                            let e = if sel_end.block > ix
                                || (sel_end.block == ix && sel_end.part > Part::Code)
                            {
                                line_str.len()
                            } else if sel_end.block == ix && sel_end.part == Part::Code {
                                sel_end.offset.clamp(line_start, line_end) - line_start
                            } else {
                                0
                            };
                            if s < e { Some(s..e) } else { None }
                        } else {
                            None
                        };

                        let line_caret = if is_active_block
                            && show_caret
                            && selection.is_collapsed()
                            && selection.head.offset >= line_start
                            && (selection.head.offset <= line_end
                                || (line_num + 1 == line_count
                                    && selection.head.offset >= line_end))
                        {
                            Some(
                                selection
                                    .head
                                    .offset
                                    .saturating_sub(line_start)
                                    .min(line_str.len()),
                            )
                        } else {
                            None
                        };

                        let content_el = if let Some(range) = line_sel {
                            let before = &line_str[..range.start];
                            let selected = &line_str[range.clone()];
                            let after = &line_str[range.end..];
                            div()
                                .flex()
                                .items_center()
                                .child(before.to_string())
                                .child(
                                    div()
                                        .bg(hsla(0.58, 0.8, 0.5, 0.3))
                                        .rounded(px(2.0))
                                        .child(selected.to_string()),
                                )
                                .child(after.to_string())
                                .into_any_element()
                        } else if let Some(col) = line_caret {
                            let safe_col = if line_str.is_char_boundary(col) {
                                col
                            } else {
                                let mut c = col;
                                while c > 0 && !line_str.is_char_boundary(c) {
                                    c -= 1;
                                }
                                c
                            };
                            let before = &line_str[..safe_col];
                            let after = &line_str[safe_col..];
                            div()
                                .flex()
                                .items_center()
                                .child(before.to_string())
                                .child(
                                    div()
                                        .w(px(2.0))
                                        .h(px(text_size * 1.2))
                                        .my(px(-2.0))
                                        .bg(theme.accent),
                                )
                                .child(after.to_string())
                                .into_any_element()
                        } else if line_str.is_empty() {
                            div().h(px(code_line_height)).into_any_element()
                        } else {
                            div().child(line_str.to_string()).into_any_element()
                        };

                        line_elements.push(
                            div()
                                .id(("code-line", line_num))
                                .w_full()
                                .min_h(px(code_line_height))
                                .flex()
                                .items_start()
                                .child(
                                    div()
                                        .w(px(gutter_width))
                                        .flex_none()
                                        .text_align(gpui::TextAlign::Right)
                                        .pr(px(10.0))
                                        .text_color(theme.text_ghost)
                                        .child(format!("{}", line_num + 1)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_color(theme.text)
                                        .child(content_el),
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, window, cx| {
                                        window.focus(&this.focus_handle, cx);
                                        this.on_mouse_down(ix, Part::Code, line_start, cx);
                                        cx.stop_propagation();
                                    }),
                                ),
                        );
                    }

                    let code_editor_el = div()
                        .w_full()
                        .py(px(8.0))
                        .font_family(MONO_FONT)
                        .text_size(px(text_size * 0.88))
                        .line_height(px(code_line_height))
                        .children(line_elements);

                    let preview_el = if let Some(custom) = &custom_renderer {
                        if let Some(rendered) = custom(ix, block, mode, window, cx.deref_mut()) {
                            div().w_full().p(px(8.0)).child(rendered).into_any_element()
                        } else {
                            div()
                                .w_full()
                                .p(px(12.0))
                                .font_family(MONO_FONT)
                                .text_size(px(text_size * 0.88))
                                .text_color(theme.text_secondary)
                                .child(code.text.clone())
                                .into_any_element()
                        }
                    } else {
                        div()
                            .w_full()
                            .p(px(12.0))
                            .font_family(MONO_FONT)
                            .text_size(px(text_size * 0.88))
                            .text_color(theme.text_secondary)
                            .child(code.text.clone())
                            .into_any_element()
                    };

                    let body: AnyElement = match mode {
                        BlockPreviewMode::Code => code_editor_el.into_any_element(),
                        BlockPreviewMode::Preview => preview_el,
                        BlockPreviewMode::Split => div()
                            .w_full()
                            .flex()
                            .items_stretch()
                            .child(div().flex_1().min_w_0().child(code_editor_el))
                            .child(div().w(px(1.0)).bg(theme.border))
                            .child(div().flex_1().min_w_0().child(preview_el))
                            .into_any_element(),
                    };

                    div()
                        .w_full()
                        .my(px(6.0))
                        .rounded(px(8.0))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.code_wash)
                        .overflow_hidden()
                        .child(header)
                        .child(body)
                        .into_any_element()
                }
                BlockKind::Rule => div()
                    .w_full()
                    .my(px(12.0))
                    .h(px(1.0))
                    .bg(theme.border)
                    .into_any_element(),
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
                                        .my(px(-2.0))
                                        .bg(theme.accent),
                                )
                                .child(
                                    div()
                                        .pl(px(2.0))
                                        .text_color(theme.text_ghost)
                                        .child("Type '/' for commands"),
                                )
                                .into_any_element()
                        } else {
                            div()
                                .text_color(theme.text_ghost)
                                .child("Type '/' for commands")
                                .into_any_element()
                        }
                    } else {
                        div()
                            .text_color(theme.text)
                            .child(render_text_with_caret(&text.text, Part::Body))
                            .into_any_element()
                    }
                }
                _ => {
                    let text = block
                        .text_at(Part::Body)
                        .map(|t| t.text.as_str())
                        .unwrap_or("");
                    div()
                        .text_color(theme.text)
                        .child(render_text_with_caret(text, Part::Body))
                        .into_any_element()
                }
            };

            let default_part = match block.kind {
                BlockKind::Code { .. } => Part::Code,
                _ => Part::Body,
            };

            block_elements.push(
                div()
                    .id(("block", ix))
                    .w_full()
                    .py(px(2.0))
                    .when(block.indent > 0, |d| d.pl(px(16.0 * block.indent as f32)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            window.focus(&this.focus_handle, cx);
                            this.on_mouse_down(ix, default_part, 0, cx);
                            cx.stop_propagation();
                        }),
                    )
                    .child(block_el),
            );
        }

        let slash_popover = if let Some(slash) = &self.slash {
            let selected_ix = slash.selected;
            let all_items = items();
            let rows: Vec<_> = slash
                .rows
                .iter()
                .copied()
                .filter_map(|idx| {
                    let (label, kind) = all_items.get(idx)?;
                    Some((idx, label.clone(), kind.clone()))
                })
                .collect();

            let target_top = 36.0 + (slash.at.block as f32 * 28.0).min(400.0);

            Some(
                div()
                    .id(SLASH_MENU)
                    .absolute()
                    .top(px(target_top))
                    .left(px(layout.text_inset + 8.0))
                    .w(px(260.0))
                    .max_h(px(280.0))
                    .overflow_y_scroll()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .shadow_lg()
                    .p(px(4.0))
                    .children(rows.into_iter().enumerate().map(
                        |(list_ix, (item_ix, label, kind))| {
                            let is_sel = list_ix == selected_ix;
                            let kind_clone = kind.clone();
                            let icon_str = match &kind {
                                BlockKind::Paragraph(_) => "¶",
                                BlockKind::Heading { level: 1, .. } => "H1",
                                BlockKind::Heading { level: 2, .. } => "H2",
                                BlockKind::Heading { level: 3, .. } => "H3",
                                BlockKind::Bullet(_) => "•",
                                BlockKind::Ordered { .. } => "1.",
                                BlockKind::Task { .. } => "☑",
                                BlockKind::Quote { kind: None, .. } => "“",
                                BlockKind::Quote {
                                    kind: Some(QuoteKind::Note),
                                    ..
                                } => "ℹ",
                                BlockKind::Quote {
                                    kind: Some(QuoteKind::Tip),
                                    ..
                                } => "💡",
                                BlockKind::Quote {
                                    kind: Some(QuoteKind::Important),
                                    ..
                                } => "⚠️",
                                BlockKind::Quote {
                                    kind: Some(QuoteKind::Warning),
                                    ..
                                } => "⚡",
                                BlockKind::Quote {
                                    kind: Some(QuoteKind::Caution),
                                    ..
                                } => "🛑",
                                BlockKind::Code {
                                    language: Some(lang),
                                    ..
                                } if lang == "mermaid" => "📊",
                                BlockKind::Code {
                                    language: Some(lang),
                                    ..
                                } if matches!(lang.as_str(), "latex" | "tex" | "math") => "∑",
                                BlockKind::Code { .. } => "</>",
                                BlockKind::Table { .. } => "⊞",
                                BlockKind::Rule => "―",
                                _ => "▪",
                            };
                            div()
                                .id(("slash-item", item_ix))
                                .px(px(8.0))
                                .py(px(5.0))
                                .rounded(px(6.0))
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .cursor_pointer()
                                .when(is_sel, |s| s.bg(theme.overlay))
                                .hover(|s| s.bg(theme.overlay))
                                .child(
                                    div()
                                        .size(px(24.0))
                                        .rounded(px(4.0))
                                        .border_1()
                                        .border_color(theme.border)
                                        .bg(theme.inset)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_size(px(11.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.accent)
                                        .child(icon_str),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(theme.text)
                                        .child(label.to_string()),
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.confirm_slash_choice(kind_clone.clone(), cx);
                                        cx.stop_propagation();
                                    }),
                                )
                        },
                    )),
            )
        } else {
            None
        };

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
                    this.focus_end(window, cx);
                }),
            )
            .children(block_elements)
            .when_some(slash_popover, |div, slash| div.child(slash))
    }
}
