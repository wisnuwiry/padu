//! Shared Notion-style block editor component and loader for GPUI, backed by bezel-editor.

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, ElementId, Focusable, IntoElement, MouseButton, Pixels, RenderOnce,
    ScrollHandle, SharedString, Window, div, px,
};
use std::rc::Rc;

#[allow(unused_imports)]
pub use bezel_editor::{Chrome, Editor, EditorEvent, Mode};
use bezel_markdown::Mark;

use crate::theme::Theme;
use crate::theme::sp;
use crate::ui::scrollbar::{self, ScrollbarState};
use crate::ui::tooltip::Tooltip;

pub trait EditorExt {
    fn content(&self) -> String;
    fn set_content(&mut self, text: impl AsRef<str>, cx: &mut Context<bezel_editor::Editor>);
}

impl EditorExt for bezel_editor::Editor {
    fn content(&self) -> String {
        self.source()
    }

    fn set_content(&mut self, text: impl AsRef<str>, cx: &mut Context<bezel_editor::Editor>) {
        let text = text.as_ref();
        let doc = self.document().clone();
        doc.update(cx, |doc, cx| {
            if doc.source() != text {
                doc.reload(text, cx);
            }
        });
    }
}

#[allow(dead_code)]
pub fn editor_theme(_theme: &Theme) {}

pub fn configure_editor(_editor: &gpui::Entity<bezel_editor::Editor>, _cx: &mut App) {}

/// A reusable Notion-style editor container and loader.
#[derive(IntoElement)]
pub struct RichEditor {
    id: ElementId,
    editor: gpui::Entity<bezel_editor::Editor>,
    scroll_handle: ScrollHandle,
    scrollbar: Option<Rc<ScrollbarState>>,
    padding: Option<Pixels>,
}

#[allow(dead_code)]
pub type SharedEditor = RichEditor;
#[allow(dead_code)]
pub type EditorLoader = RichEditor;

impl RichEditor {
    pub fn new(id: impl Into<ElementId>, editor: gpui::Entity<bezel_editor::Editor>) -> Self {
        Self {
            id: id.into(),
            editor,
            scroll_handle: ScrollHandle::new(),
            scrollbar: None,
            padding: None,
        }
    }

    /// Connects a scroll handle to track vertical scrolling.
    pub fn track_scroll(mut self, handle: &ScrollHandle) -> Self {
        self.scroll_handle = handle.clone();
        self
    }

    /// Attaches an overlay scrollbar.
    pub fn scrollbar(mut self, state: &Rc<ScrollbarState>) -> Self {
        self.scrollbar = Some(state.clone());
        self
    }

    /// Overrides the container padding (default is 14px).
    #[allow(dead_code)]
    pub fn padding(mut self, padding: Pixels) -> Self {
        self.padding = Some(padding);
        self
    }
}

impl RenderOnce for RichEditor {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let editor = self.editor.clone();
        let scrollbar_state = self.scrollbar.clone();
        let pad = self.padding.unwrap_or(px(14.0));

        div()
            .id(self.id)
            .flex_1()
            .min_h_0()
            .min_w_0()
            .relative()
            .rounded(px(8.0))
            .bg(theme.inset)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                editor.update(cx, |ed, cx| {
                    window.focus(&ed.focus_handle(cx), cx);
                });
            })
            .child(
                div()
                    .id("rich-editor-scroll-container")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p(pad)
                    .child(self.editor),
            )
            .when_some(scrollbar_state, |parent, state| {
                parent.child(scrollbar::vertical(&self.scroll_handle, &state))
            })
    }
}

/// Floating selection toolbar for rich text editing, with block turn-into dropdown and inline mark buttons.
pub struct EditorSelectionToolbar {
    id: SharedString,
    editor: gpui::Entity<bezel_editor::Editor>,
    turn_open: bool,
    on_toggle_turn: Rc<dyn Fn(&mut Window, &mut App)>,
    on_close_turn: Rc<dyn Fn(&mut Window, &mut App)>,
}

impl EditorSelectionToolbar {
    pub fn new(
        id: impl Into<SharedString>,
        editor: gpui::Entity<bezel_editor::Editor>,
        turn_open: bool,
        on_toggle_turn: impl Fn(&mut Window, &mut App) + 'static,
        on_close_turn: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            editor,
            turn_open,
            on_toggle_turn: Rc::new(on_toggle_turn),
            on_close_turn: Rc::new(on_close_turn),
        }
    }

    pub fn render(self, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        let editor_ref = self.editor.read(cx);
        let formatting = editor_ref.formatting();
        if formatting.mode == bezel_editor::Mode::Source {
            (self.on_close_turn)(window, cx);
            return None;
        }

        let bounds = editor_ref.selection_bounds()?;
        let viewport_h = window.viewport_size().height;
        if bounds.origin.y < px(40.0) || bounds.origin.y > viewport_h - px(40.0) {
            (self.on_close_turn)(window, cx);
            return None;
        }

        let bezel_theme = bezel::theme::Theme::of(cx).clone();
        let view = bezel::motion::Painter::from(self.editor.entity_id());
        let block_idx = editor_ref.selection().head.block;
        let block_label = formatting
            .block
            .clone()
            .unwrap_or_else(|| SharedString::from("Text"));
        let turn_open = self.turn_open;

        let on_toggle = self.on_toggle_turn.clone();
        let on_close_out = self.on_close_turn.clone();
        let editor_for_items = self.editor.clone();
        let on_close_item = self.on_close_turn.clone();

        let turn_trigger = div()
            .id(SharedString::from(format!("{}-turn-trigger", self.id)))
            .relative()
            .child(
                bezel::ui::popover::menu_row(
                    &bezel_theme,
                    turn_open,
                    Some(bezel::motion::Fade::new(
                        view,
                        format!("{}-turn-trigger", self.id),
                    )),
                )
                .id(SharedString::from(format!("{}-turn-trigger-btn", self.id)))
                .px(px(7.0))
                .py(px(4.0))
                .h(px(24.0))
                .rounded(px(4.0))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(sp(12.0))
                        .text_color(bezel_theme.text)
                        .child(block_label.clone()),
                )
                .child(
                    bezel::ui::icons::icon(bezel::ui::icons::glyph::ChevronDown)
                        .size(px(10.0))
                        .text_color(bezel_theme.text_faint),
                )
                .tooltip(Tooltip::text("Turn into"))
                .on_click(move |_, window, cx| {
                    (on_toggle)(window, cx);
                }),
            )
            .when(turn_open, |el| {
                let current_block_name = block_label.clone();
                let turn_items = bezel_editor::turns();
                let theme_ref = &bezel_theme;
                el.child(bezel::ui::popover::anchored_menu_below(
                    SharedString::from(format!("{}-turn-menu", self.id)),
                    bezel::ui::popover::popover_card(theme_ref)
                        .id(SharedString::from(format!("{}-turn-menu-card", self.id)))
                        .w(px(160.0))
                        .max_h(px(260.0))
                        .overflow_y_scroll()
                        .on_mouse_down_out(move |_, window, cx| {
                            (on_close_out)(window, cx);
                        })
                        .children(turn_items.into_iter().map(|(name, kind)| {
                            let is_current = current_block_name.as_ref() == name.as_ref();
                            let row_name = name.clone();
                            let editor = editor_for_items.clone();
                            let on_close = on_close_item.clone();
                            bezel::ui::popover::menu_row(
                                theme_ref,
                                is_current,
                                Some(bezel::motion::Fade::new(
                                    view,
                                    format!("{}-turn-{}", self.id, name),
                                )),
                            )
                            .id(SharedString::from(format!("{}-turn-{}", self.id, name)))
                            .px(px(8.0))
                            .py(px(5.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .text_size(sp(12.0))
                                    .text_color(if is_current {
                                        theme_ref.text
                                    } else {
                                        theme_ref.text.opacity(0.9)
                                    })
                                    .child(row_name),
                            )
                            .on_click(move |_, window, cx| {
                                let kind = kind.clone();
                                editor.update(cx, |ed, cx| {
                                    ed.set_block(block_idx, kind, cx);
                                    window.focus(&ed.focus_handle(cx), cx);
                                });
                                (on_close)(window, cx);
                            })
                        }))
                        .into_any_element(),
                    None,
                ))
            });

        let divider = div()
            .w(px(1.0))
            .h(px(14.0))
            .my_auto()
            .mx(px(2.0))
            .bg(bezel_theme.border_strong);

        struct MarkButton {
            key: &'static str,
            glyph: &'static [u8],
            label: &'static str,
            shortcut: (&'static str, &'static str),
            mark: Mark,
        }

        let marks = [
            MarkButton {
                key: "bold",
                glyph: bezel::ui::icons::glyph::Bold,
                label: "Bold",
                shortcut: ("⌘B", "Ctrl+B"),
                mark: Mark::Bold,
            },
            MarkButton {
                key: "italic",
                glyph: bezel::ui::icons::glyph::Italic,
                label: "Italic",
                shortcut: ("⌘I", "Ctrl+I"),
                mark: Mark::Italic,
            },
            MarkButton {
                key: "strike",
                glyph: bezel::ui::icons::glyph::Strikethrough,
                label: "Strikethrough",
                shortcut: ("⌘⇧X", "Ctrl+Shift+X"),
                mark: Mark::Strike,
            },
            MarkButton {
                key: "code",
                glyph: bezel::ui::icons::glyph::Code,
                label: "Code",
                shortcut: ("⌘E", "Ctrl+E"),
                mark: Mark::Code,
            },
        ];

        let editor_for_marks = self.editor.clone();
        let id_str = self.id.clone();
        let buttons = marks.map(|item| {
            let lit = formatting.marks.contains(&item.mark);
            let mark = item.mark.clone();
            let label = item.label;
            let shortcut = crate::platform::primary_shortcut(item.shortcut.0, item.shortcut.1);
            let editor = editor_for_marks.clone();

            bezel::ui::popover::menu_row(
                &bezel_theme,
                lit,
                Some(bezel::motion::Fade::new(
                    view,
                    format!("{}-bubble-{}", id_str, item.key),
                )),
            )
            .id(SharedString::from(format!(
                "{}-bubble-{}",
                id_str, item.key
            )))
            .px(px(6.0))
            .py(px(4.0))
            .h(px(24.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .child(
                bezel::ui::icons::icon(item.glyph)
                    .size(px(13.0))
                    .text_color(if lit {
                        bezel_theme.text
                    } else {
                        bezel_theme.text.opacity(0.85)
                    }),
            )
            .tooltip(Tooltip::with_shortcut(label, shortcut))
            .on_click(move |_, window, cx| {
                let mark = mark.clone();
                editor.update(cx, |ed, cx| {
                    ed.toggle_mark(mark, cx);
                    window.focus(&ed.focus_handle(cx), cx);
                });
            })
        });

        let at = gpui::point(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height + px(6.0),
        );

        Some(bezel::ui::popover::menu_at(
            self.id.clone(),
            at,
            div()
                .id(SharedString::from(format!("{}-card", self.id)))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(2.0))
                .p(px(3.0))
                .child(turn_trigger)
                .child(divider)
                .children(buttons)
                .into_any_element(),
            None,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn test_rich_editor_builder(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| bezel_editor::Editor::new("Test", cx));
            let scroll_handle = ScrollHandle::new();
            let scrollbar = ScrollbarState::new();

            let rich = RichEditor::new("test-editor", editor)
                .track_scroll(&scroll_handle)
                .scrollbar(&scrollbar)
                .padding(px(16.0));

            assert_eq!(rich.padding, Some(px(16.0)));
            assert!(rich.scrollbar.is_some());
        });
    }

    #[gpui::test]
    fn test_editor_selection_toolbar_builder(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| bezel_editor::Editor::new("Hello world", cx));
            let toolbar =
                EditorSelectionToolbar::new("test-toolbar", editor, false, |_, _| {}, |_, _| {});
            assert_eq!(toolbar.id.as_ref(), "test-toolbar");
            assert!(!toolbar.turn_open);
        });
    }
}
