//! Padu's tooltip surface.
//!
//! GPUI already owns tooltip *behaviour* — hover timing, placement, dismissal —
//! through `InteractiveElement::tooltip`, which asks only for a view to render.
//! This is that view, and nothing more.

use gpui::{
    AnyView, App, AppContext, FontWeight, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, prelude::*, px,
};

use crate::theme::{Theme, sp};

/// A single-line hint.
pub struct Tooltip {
    label: SharedString,
    shortcut: Option<SharedString>,
}

impl Tooltip {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            shortcut: None,
        }
    }

    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Build the view GPUI's `.tooltip(..)` expects.
    pub fn build(self, _window: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| self).into()
    }

    /// Shorthand for the overwhelmingly common case:
    /// `.tooltip(Tooltip::text("Copy message"))`.
    pub fn text(
        label: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let label = label.into();
        move |window, cx| Tooltip::new(label.clone()).build(window, cx)
    }

    /// Shorthand for tooltips with a keyboard shortcut badge:
    /// `.tooltip(Tooltip::with_shortcut("New task", "⌘N"))`.
    pub fn with_shortcut(
        label: impl Into<SharedString>,
        shortcut: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let label = label.into();
        let shortcut = shortcut.into();
        move |window, cx| {
            Tooltip::new(label.clone())
                .shortcut(shortcut.clone())
                .build(window, cx)
        }
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        if self.label.trim().is_empty() {
            return div();
        }
        let theme = Theme::current(cx);
        // The outer wrapper is transparent and only offsets the card from the
        // cursor; the shadow needs a parent that does not clip it.
        div().pt(px(4.0)).pl(px(2.0)).child(
            div()
                .px(px(7.0))
                .py(px(4.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(theme.border_strong)
                .bg(theme.raised)
                .shadow_md()
                .flex()
                .items_center()
                .gap(px(8.0))
                .text_size(sp(12.5))
                .line_height(sp(15.0))
                .text_color(theme.text_secondary)
                .child(self.label.clone())
                .when_some(self.shortcut.clone(), |row, shortcut| {
                    row.child(
                        div()
                            .px(px(4.0))
                            .py(px(1.0))
                            .rounded(px(4.0))
                            .bg(theme.overlay_strong)
                            .text_size(sp(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_tertiary)
                            .child(shortcut),
                    )
                }),
        )
    }
}
