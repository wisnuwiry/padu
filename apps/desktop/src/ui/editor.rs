//! Shared Notion-style block editor component and loader for GPUI, backed by bezel-editor.

use gpui::prelude::*;
use gpui::{
    App, Context, ElementId, Focusable, IntoElement, MouseButton, Pixels, RenderOnce, ScrollHandle,
    Window, div, px,
};
use std::rc::Rc;

#[allow(unused_imports)]
pub use bezel_editor::{Chrome, Editor, EditorEvent, Mode};

use crate::theme::Theme;
use crate::ui::scrollbar::{self, ScrollbarState};

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
}
