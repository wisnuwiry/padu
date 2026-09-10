//! Window-modal preview for embedded note references.

use gpui::{KeyBinding, actions};

use super::*;

actions!(padu_note_preview, [DismissNotePreview]);

const NOTE_PREVIEW_CONTEXT: &str = "NotePreview";

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new(
        "escape",
        DismissNotePreview,
        Some(NOTE_PREVIEW_CONTEXT),
    )]);
}

pub(super) struct NotePreviewState {
    note: padu_protocol::notes::EmbeddedNote,
    markdown: RefCell<MarkdownView>,
    selection: TranscriptSelection,
    scroll_handle: ScrollHandle,
    scrollbar: Rc<ScrollbarState>,
    focus: FocusHandle,
    close_focus: FocusHandle,
    previous_focus: Option<FocusHandle>,
    generation: u64,
}

impl Padu {
    pub(super) fn open_note_preview(
        &mut self,
        note: padu_protocol::notes::EmbeddedNote,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.note_preview_generation = self.note_preview_generation.wrapping_add(1);
        let generation = self.note_preview_generation;
        let focus = cx.focus_handle();
        self.note_preview = Some(NotePreviewState {
            note,
            markdown: RefCell::new(MarkdownView::new()),
            selection: TranscriptSelection::default(),
            scroll_handle: ScrollHandle::new(),
            scrollbar: ScrollbarState::new(),
            focus: focus.clone(),
            close_focus: cx.focus_handle(),
            previous_focus: window.focused(cx),
            generation,
        });

        let weak = cx.entity().downgrade();
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| {
                let mut should_focus = false;
                let _ = weak.update(cx, |this, _| {
                    should_focus = this
                        .note_preview
                        .as_ref()
                        .is_some_and(|preview| preview.generation == generation);
                });
                if should_focus {
                    window.focus(&focus, cx);
                }
            });
        });
        cx.notify();
    }

    pub(super) fn close_note_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(preview) = self.note_preview.take() else {
            return;
        };
        self.note_preview_generation = self.note_preview_generation.wrapping_add(1);
        if let Some(previous_focus) = preview.previous_focus {
            window.focus(&previous_focus, cx);
        } else {
            window.focus(&self.composer_focus(cx), cx);
        }
        cx.notify();
    }

    pub(super) fn render_note_preview(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let preview = self.note_preview.as_ref()?;
        let theme = Theme::current(cx);
        let note = &preview.note;
        let title = if note.title.is_empty() {
            tr!("notes.untitled")
        } else {
            note.title.clone()
        };
        let focus = preview.focus.clone();
        let close_focus = preview.close_focus.clone();
        let generation = preview.generation;
        let palette = MarkdownPalette::from_theme(&theme);
        let selection = preview.selection.clone();
        let markdown_context = MarkdownCtx::new(
            format!("note-modal-{generation}"),
            &palette,
            self.scaled_markdown_metrics(MarkdownMetrics::BODY),
            selection.clone(),
        );
        let document = {
            let mut markdown = preview.markdown.borrow_mut();
            markdown.set_text(&note.content, false);
            md::render::markdown(&mut markdown, &markdown_context).or_else(|| {
                (!note.content.is_empty()).then(|| {
                    md::render::plain_text(
                        note.content.clone(),
                        md::render::SANS_FAMILY,
                        FontWeight::NORMAL,
                        theme.text,
                        &markdown_context,
                    )
                })
            })
        };
        let selection_input = canvas(|_, _, _| (), {
            let selection = selection.clone();
            move |_, _, window, _| md::render::install_selection_input(window, &selection)
        })
        .absolute()
        .w(px(0.0))
        .h(px(0.0));

        let close = div()
            .id("note-preview-close")
            .track_focus(&close_focus)
            .tab_index(0)
            .absolute()
            .top(px(14.0))
            .right(px(14.0))
            .h(px(30.0))
            .min_w(px(58.0))
            .px(px(6.0))
            .gap(px(5.0))
            .rounded(px(8.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(theme.overlay_strong)
            .focus_visible(|style| style.border_1().border_color(theme.accent))
            .hover(|style| style.bg(theme.overlay))
            .child(icon("icons/x.svg", 13.0, theme.text))
            .child(crate::ui::kbd_badge("Esc", &theme))
            .on_click(cx.listener(|this, _, window, cx| {
                this.close_note_preview(window, cx);
                cx.stop_propagation();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    this.close_note_preview(window, cx);
                    cx.stop_propagation();
                }
            }));

        let content = div()
            .id(SharedString::from(format!(
                "note-preview-content-{generation}"
            )))
            .relative()
            .w(px(680.0))
            .max_w_full()
            .h(px(620.0))
            .max_h(px(620.0))
            .rounded(px(12.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.surface)
            .p(px(22.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .pr(px(36.0))
                    .text_size(sp(16.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child(title),
            )
            .child(
                div()
                    .id("note-preview-body")
                    .flex_1()
                    .min_h_0()
                    .max_h(px(540.0))
                    .relative()
                    .child(
                        div()
                            .id("note-preview-body-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&preview.scroll_handle)
                            .text_color(theme.text)
                            .whitespace_normal()
                            .child(md::render::frame_reset(selection.clone()))
                            .children(document)
                            .child(selection_input),
                    )
                    .child(scrollbar::vertical(
                        &preview.scroll_handle,
                        &preview.scrollbar,
                    )),
            )
            .child(close);

        let layer = div()
            .id(SharedString::from(format!(
                "note-preview-layer-{generation}"
            )))
            .absolute()
            .inset_0()
            .occlude()
            .track_focus(&focus)
            .key_context(NOTE_PREVIEW_CONTEXT)
            .on_action(cx.listener(|this, _: &DismissNotePreview, window, cx| {
                this.close_note_preview(window, cx);
            }))
            .tab_group()
            .tab_stop(false)
            .bg(gpui::hsla(0.0, 0.0, 0.0, 0.48))
            .p(px(36.0))
            .flex()
            .items_center()
            .justify_center()
            .child(content);

        Some(gpui::deferred(layer).with_priority(5).into_any_element())
    }
}
