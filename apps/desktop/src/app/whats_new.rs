//! Window-modal "what's new" release notes, shown after an app update.
//!
//! A full-width button sits directly above the sidebar footer while the
//! recorded [`PersistedState::last_seen_version`] differs from the running
//! version. Only pressing Done in the dialog marks the current version seen,
//! so the button stays visible when the dialog is closed via the close
//! button, backdrop, or Escape. Fresh installs stamp the version silently at
//! startup and never show the button.
//!
//! Only the current version's notes are bundled (extracted at build time
//! from `CHANGELOG.md` by `apps/desktop/build.rs`), so frames read an
//! in-memory `&str` — no filesystem or network I/O on the UI thread.

use gpui::{KeyBinding, actions};

use super::*;
use crate::ui::ActivationExt;

actions!(padu_whats_new, [OpenWhatsNew, DismissWhatsNew]);

const WHATS_NEW_CONTEXT: &str = "WhatsNew";

/// Running version; compared against the persisted last-seen version.
pub(super) const WHATS_NEW_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Current version's notes only, written by the build script.
const WHATS_NEW_NOTES: &str = include_str!(concat!(env!("OUT_DIR"), "/whats-new.md"));

const WHATS_NEW_CHANGELOG_URL: &str = "https://padu.dev/changelog";

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new(
        "escape",
        DismissWhatsNew,
        Some(WHATS_NEW_CONTEXT),
    )]);
}

pub(super) struct WhatsNewState {
    markdown: RefCell<MarkdownView>,
    selection: TranscriptSelection,
    scroll_handle: ScrollHandle,
    scrollbar: Rc<ScrollbarState>,
    focus: FocusHandle,
    close_focus: FocusHandle,
    done_focus: FocusHandle,
    previous_focus: Option<FocusHandle>,
}

impl Padu {
    /// Whether the full-width sidebar button should be shown: an update was
    /// recorded as unseen. A missing record is a fresh install — never show.
    pub(super) fn should_show_whats_new(&self) -> bool {
        self.state.shows_whats_new_for(WHATS_NEW_VERSION)
    }

    pub fn open_whats_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.whats_new.is_some() {
            if let Some(state) = self.whats_new.as_ref() {
                let focus = state.done_focus.clone();
                window.focus(&focus, cx);
            }
            return;
        }
        let done_focus = cx.focus_handle();
        let dialog_focus = done_focus.clone();
        self.whats_new = Some(WhatsNewState {
            markdown: RefCell::new(MarkdownView::new()),
            selection: TranscriptSelection::default(),
            scroll_handle: ScrollHandle::new(),
            scrollbar: ScrollbarState::new(),
            focus: cx.focus_handle(),
            close_focus: cx.focus_handle(),
            done_focus,
            previous_focus: window.focused(cx),
        });

        let weak = cx.entity().downgrade();
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| {
                let mut should_focus = false;
                let _ = weak.update(cx, |this, _| {
                    should_focus = this.whats_new.is_some();
                });
                if should_focus {
                    window.focus(&dialog_focus, cx);
                }
            });
        });
        cx.notify();
    }

    /// Close the dialog without marking the version seen. Used by every
    /// dismiss path except Done (close button, backdrop, Escape), so the
    /// sidebar button stays visible until the user explicitly presses Done.
    pub fn close_whats_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.whats_new.take() else {
            return;
        };
        if let Some(previous_focus) = state.previous_focus {
            window.focus(&previous_focus, cx);
        }
        cx.notify();
    }

    /// Close the dialog via Done and stamp the current version seen.
    /// Idempotent: stamping happens only here, so every other close path
    /// funnels through [`Self::close_whats_new`] instead.
    pub fn dismiss_whats_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.whats_new.is_none() {
            return;
        }
        // One-shot user action: mark seen synchronously so the sidebar button
        // disappears on the same frame the dialog closes.
        self.state.last_seen_version = Some(WHATS_NEW_VERSION.to_string());
        self.save();
        self.close_whats_new(window, cx);
    }

    /// Full-width button rendered directly above the sidebar footer row.
    pub(super) fn render_whats_new_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        div()
            .w_full()
            .flex_none()
            .px(px(10.0))
            .pt(px(6.0))
            .child(
                div()
                    .id("whats-new-button")
                    .tab_index(0)
                    .focus_visible(|style| style.border_1().border_color(theme.accent))
                    .w_full()
                    .h(px(30.0))
                    .rounded(px(8.0))
                    .bg(theme.accent.opacity(0.12))
                    .border_1()
                    .border_color(theme.accent.opacity(0.35))
                    .flex()
                    .items_center()
                    .gap(px(7.0))
                    .px(px(9.0))
                    .cursor_pointer()
                    .hover(|element| element.bg(theme.accent.opacity(0.18)))
                    .active(|element| element.bg(theme.accent.opacity(0.24)))
                    .tooltip(Tooltip::text(tr!("whats_new.tooltip")))
                    .child(icon("icons/sparkle.svg", 13.0, theme.accent))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(sp(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(tr!("whats_new.button", version = WHATS_NEW_VERSION)),
                    )
                    .child(icon("icons/chevron-right.svg", 12.0, theme.text_tertiary))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_whats_new(window, cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            this.open_whats_new(window, cx);
                            cx.stop_propagation();
                        }
                    })),
            )
            .into_any_element()
    }

    pub(super) fn render_whats_new_modal(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.whats_new.as_ref()?;
        let theme = Theme::current(cx);
        let close_focus = state.close_focus.clone();
        let done_focus = state.done_focus.clone();
        let selection = state.selection.clone();
        let palette = MarkdownPalette::from_theme(&theme);
        let markdown_context = MarkdownCtx::new(
            format!("whats-new-md-{WHATS_NEW_VERSION}"),
            &palette,
            self.scaled_markdown_metrics(MarkdownMetrics::COMPACT),
            selection.clone(),
        );
        // `set_text` is a cheap no-op when the text is unchanged, so seeding
        // here keeps the parse cached across frames without extra state.
        let document = {
            let mut markdown = state.markdown.borrow_mut();
            markdown.set_text(WHATS_NEW_NOTES, false);
            md::render::markdown(&markdown, &markdown_context).or_else(|| {
                (!WHATS_NEW_NOTES.trim().is_empty()).then(|| {
                    md::render::plain_text(
                        WHATS_NEW_NOTES.to_string(),
                        md::render::SANS_FAMILY,
                        FontWeight::NORMAL,
                        theme.text_secondary,
                        &markdown_context,
                    )
                })
            })
        };
        let empty_notes = WHATS_NEW_NOTES.trim().is_empty();
        let selection_input = canvas(|_, _, _| (), {
            let selection = selection.clone();
            move |_, _, window, _| md::render::install_selection_input(window, &selection)
        })
        .absolute()
        .w(px(0.0))
        .h(px(0.0));

        let close = div()
            .id("whats-new-close")
            .track_focus(&close_focus)
            .tab_index(0)
            .h(px(28.0))
            .min_w(px(28.0))
            .px(px(6.0))
            .gap(px(6.0))
            .rounded(px(7.0))
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .cursor_pointer()
            .hover(|style| style.bg(theme.overlay))
            .active(|style| style.bg(theme.overlay_strong))
            .focus_visible(|style| style.border_1().border_color(theme.accent))
            .tooltip(Tooltip::text(tr!("whats_new.dismiss")))
            .child(icon("icons/x.svg", 13.0, theme.text_secondary))
            .child(crate::ui::kbd_badge("Esc", &theme))
            .on_activation(cx, |this, window, cx| {
                this.close_whats_new(window, cx);
            });

        let header = div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .size(px(40.0))
                    .rounded(px(10.0))
                    .bg(theme.accent.opacity(0.12))
                    .border_1()
                    .border_color(theme.accent.opacity(0.25))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_none()
                    .child(icon("icons/sparkle.svg", 18.0, theme.accent)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(sp(16.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child(tr!("whats_new.title")),
                            )
                            .child(
                                div()
                                    .px(px(7.0))
                                    .py(px(2.0))
                                    .rounded_full()
                                    .bg(theme.accent.opacity(0.12))
                                    .text_size(sp(11.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.accent)
                                    .child(format!("v{WHATS_NEW_VERSION}")),
                            ),
                    )
                    .child(
                        div()
                            .text_size(sp(12.5))
                            .text_color(theme.text_tertiary)
                            .child(tr!("whats_new.subtitle")),
                    ),
            )
            .child(close);

        let body = div()
            .id("whats-new-body")
            .flex_1()
            .min_h_0()
            .min_h(px(320.0))
            .max_h(px(560.0))
            .relative()
            .rounded(px(12.0))
            .bg(theme.raised)
            .border_1()
            .border_color(theme.border)
            .child(
                div()
                    .id("whats-new-body-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&state.scroll_handle)
                    .px(px(18.0))
                    .py(px(14.0))
                    .text_color(theme.text_secondary)
                    .whitespace_normal()
                    .child(md::render::frame_reset(selection.clone()))
                    .children(document)
                    .when(empty_notes, |element| {
                        element.child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .gap(px(8.0))
                                .py(px(20.0))
                                .text_center()
                                .child(icon("icons/sparkle.svg", 20.0, theme.text_tertiary))
                                .child(
                                    div()
                                        .max_w(px(340.0))
                                        .text_size(sp(13.0))
                                        .line_height(sp(20.0))
                                        .text_color(theme.text_secondary)
                                        .child(tr!("whats_new.empty")),
                                ),
                        )
                    })
                    .child(selection_input),
            )
            .child(scrollbar::vertical(&state.scroll_handle, &state.scrollbar));

        let footer = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .border_t_1()
            .border_color(theme.border)
            .pt(px(14.0))
            .child(
                div()
                    .id("whats-new-view-full")
                    .tab_index(0)
                    .cursor_pointer()
                    .focus_visible(|style| style.border_1().border_color(theme.accent))
                    .h(px(32.0))
                    .px(px(12.0))
                    .rounded(px(7.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .text_size(sp(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_secondary)
                    .hover(|style| style.bg(theme.overlay).text_color(theme.text))
                    .active(|style| style.bg(theme.overlay_strong))
                    .child(tr!("whats_new.view_full_changelog"))
                    .child(icon("icons/arrow-up-right.svg", 11.0, theme.text_tertiary))
                    .on_activation(cx, |_, _, cx| {
                        cx.open_url(WHATS_NEW_CHANGELOG_URL);
                    }),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("whats-new-done")
                    .track_focus(&done_focus)
                    .tab_index(0)
                    .cursor_pointer()
                    .focus_visible(|style| style.border_1().border_color(theme.accent))
                    .h(px(32.0))
                    .px(px(14.0))
                    .rounded(px(7.0))
                    .bg(theme.inverse)
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(6.0))
                    .text_size(sp(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.on_inverse)
                    .hover(|style| style.opacity(0.9))
                    .child(tr!("whats_new.done"))
                    .child(crate::ui::kbd_badge_icon(
                        "icons/corner-down-left.svg",
                        theme.inverse,
                        theme.on_inverse,
                        gpui::hsla(0.0, 0.0, 1.0, 0.25),
                    ))
                    .on_activation(cx, |this, window, cx| {
                        this.dismiss_whats_new(window, cx);
                    }),
            );

        let card = div()
            .id("whats-new-card")
            .key_context(WHATS_NEW_CONTEXT)
            .track_focus(&state.focus)
            .tab_group()
            .on_action(cx.listener(|this, _: &DismissWhatsNew, window, cx| {
                this.close_whats_new(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenWhatsNew, window, cx| {
                this.open_whats_new(window, cx);
            }))
            .occlude()
            .w(px(560.0))
            .max_w(px(560.0))
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border_strong)
            .rounded(px(16.0))
            .shadow_xl()
            .p(px(24.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .child(header)
            .child(body)
            .child(footer);

        Some(crate::ui::dialog::dialog_backdrop(
            "whats-new-backdrop",
            &theme,
            cx,
            |this, window, cx| this.close_whats_new(window, cx),
            card,
        ))
    }
}
