//! Shared Notion-style block editor component and loader for GPUI.
//!
//! Provides a standardized, reusable wrapper around [`padu_editor::Editor`]
//! that configures themes, canvas click behaviors (Notion-style click-to-focus
//! at the end of the document), markdown/math/mermaid block preview rendering,
//! and native scrollbar tracking.

use gpui::prelude::*;
use gpui::{
    App, ElementId, IntoElement, MouseButton, Pixels, RenderOnce, ScrollHandle, Window, div, px,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::md;
use crate::md::render::{
    Ctx as MarkdownCtx, MarkdownView, Metrics as MarkdownMetrics, Palette as MarkdownPalette,
    TranscriptSelection,
};
use crate::theme::Theme;
use crate::ui::scrollbar::{self, ScrollbarState};

/// Converts the application [`Theme`] into a [`padu_editor::EditorTheme`].
pub fn editor_theme(theme: &Theme) -> padu_editor::EditorTheme {
    padu_editor::EditorTheme {
        is_dark: theme.is_dark,
        surface: theme.surface,
        inset: theme.inset,
        overlay: theme.overlay,
        border: theme.border,
        text: theme.text,
        text_secondary: theme.text_secondary,
        text_ghost: theme.text_ghost,
        accent: theme.accent,
        code_wash: theme.code_wash,
    }
}

/// Creates a self-contained custom preview renderer for code, LaTeX/math, and Mermaid blocks.
///
/// Owns its own block preview cache so it is completely decoupled from any specific
/// workspace, note, or document.
pub fn create_markdown_block_renderer() -> padu_editor::CustomBlockRenderer {
    let cache: Rc<RefCell<HashMap<usize, (u64, MarkdownView)>>> =
        Rc::new(RefCell::new(HashMap::new()));

    Rc::new(move |block_ix, block, _preview_mode, _window, cx| {
        let (lang, code_text) = match &block.kind {
            padu_editor::BlockKind::Code { language, code } => {
                (language.as_deref().unwrap_or(""), &code.text)
            }
            _ => return None,
        };

        let lang_lower = lang.trim().to_ascii_lowercase();
        let md = if matches!(lang_lower.as_str(), "latex" | "tex" | "math") {
            format!("$$\n{}\n$$\n", code_text)
        } else if lang_lower.is_empty() {
            format!("```\n{}\n```\n", code_text)
        } else {
            format!("```{}\n{}\n```\n", lang, code_text)
        };

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        md.hash(&mut hasher);
        let hash = hasher.finish();

        let mut cache_ref = cache.borrow_mut();
        let entry = cache_ref
            .entry(block_ix)
            .or_insert_with(|| (0, MarkdownView::new()));
        if entry.0 != hash {
            entry.0 = hash;
            entry.1.set_text(&md, false);
        }

        let theme = Theme::current(cx);
        let palette = MarkdownPalette::from_theme(&theme);
        let markdown_ctx = MarkdownCtx::new(
            format!("editor-block-preview-{}", block_ix),
            &palette,
            MarkdownMetrics::BODY,
            TranscriptSelection::default(),
        );

        let rendered = md::render::markdown(&entry.1, &markdown_ctx)?;
        Some(
            div()
                .w_full()
                .min_w_0()
                .overflow_hidden()
                .child(rendered)
                .into_any_element(),
        )
    })
}

/// Ensures the given editor is configured with the current application theme and block preview renderer.
///
/// This call is idempotent: if the editor already has the current theme and a preview renderer,
/// it performs zero allocations, zero state dirties, and zero notifications.
pub fn configure_editor(editor: &gpui::Entity<padu_editor::Editor>, cx: &mut App) {
    let theme = Theme::current(cx);
    let target_theme = editor_theme(&theme);

    let needs_theme = editor.read(cx).theme() != target_theme;
    let needs_renderer = !editor.read(cx).has_custom_preview_renderer();

    if needs_theme || needs_renderer {
        editor.update(cx, |ed, cx| {
            if needs_theme {
                ed.set_theme(target_theme, cx);
            }
            if needs_renderer {
                ed.set_custom_preview_renderer(Some(create_markdown_block_renderer()), cx);
            }
        });
    }
}

/// A reusable Notion-style editor container and loader.
///
/// Features:
/// - Wraps an `Entity<padu_editor::Editor>` in a responsive, full-height Notion-like canvas
/// - Notion behavior: clicking anywhere in empty space or margins focuses the editor and places cursor at the end
/// - Clean vertical scrollbar integration
/// - Idempotent theme and markdown preview configuration
#[derive(IntoElement)]
pub struct RichEditor {
    id: ElementId,
    editor: gpui::Entity<padu_editor::Editor>,
    scroll_handle: ScrollHandle,
    scrollbar: Option<Rc<ScrollbarState>>,
    padding: Option<Pixels>,
}

#[allow(dead_code)]
pub type SharedEditor = RichEditor;
#[allow(dead_code)]
pub type EditorLoader = RichEditor;

impl RichEditor {
    pub fn new(id: impl Into<ElementId>, editor: gpui::Entity<padu_editor::Editor>) -> Self {
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
        // Idempotently configure theme and preview renderer if not already configured
        configure_editor(&self.editor, cx);

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
            // Notion behavior: clicking anywhere on the editor pane or blank space below
            // positions the caret at the end of the document
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                editor.update(cx, |ed, cx| {
                    ed.focus_end(window, cx);
                });
            })
            .child(
                div()
                    .id("rich-editor-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p(pad)
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .w_full()
                            .min_h_full()
                            .flex()
                            .flex_col()
                            .child(self.editor),
                    ),
            )
            .when_some(scrollbar_state, |this, sb| {
                this.child(scrollbar::vertical(&self.scroll_handle, &sb))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn test_editor_theme_conversion() {
        let theme = Theme::dark();
        let ed_theme = editor_theme(&theme);
        assert_eq!(ed_theme.is_dark, theme.is_dark);
        assert_eq!(ed_theme.surface, theme.surface);
        assert_eq!(ed_theme.inset, theme.inset);
        assert_eq!(ed_theme.border, theme.border);
        assert_eq!(ed_theme.accent, theme.accent);
    }

    #[gpui::test]
    fn test_configure_editor_idempotency(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| padu_editor::Editor::new("Initial block", cx));
            assert!(!editor.read(cx).has_custom_preview_renderer());

            // First configuration installs theme and renderer
            configure_editor(&editor, cx);
            assert!(editor.read(cx).has_custom_preview_renderer());

            let current_theme = editor.read(cx).theme();
            assert_eq!(current_theme, editor_theme(&Theme::current(cx)));

            // Second configuration is a no-op that maintains the existing renderer
            configure_editor(&editor, cx);
            assert!(editor.read(cx).has_custom_preview_renderer());
        });
    }

    #[gpui::test]
    fn test_rich_editor_builder(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| padu_editor::Editor::new("Test", cx));
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
