//! Padu's tooltip surface.
//!
//! GPUI already owns tooltip *behaviour* — hover timing, placement, dismissal —
//! through `InteractiveElement::tooltip`, which asks only for a view to render.
//! This wraps Bezel's native `Tooltip` component so the entire app renders
//! consistent Bezel tooltips.

use gpui::{
    AnyElement, AnyView, App, AppContext, Empty, IntoElement, Render, SharedString, Window, div,
};

/// A single-line hint backed by Bezel's tooltip.
#[derive(Clone)]
pub struct Tooltip {
    label: SharedString,
    shortcut: Option<SharedString>,
}

#[allow(dead_code)]
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
    pub fn build(self, window: &mut Window, cx: &mut App) -> AnyView {
        if self.label.trim().is_empty() {
            return cx.new(|_| Empty).into();
        }
        if let Some(shortcut) = self.shortcut {
            bezel::ui::tooltip::Tooltip::with_keystroke(self.label, shortcut, window, cx)
        } else {
            bezel::ui::tooltip::Tooltip::text(self.label, window, cx)
        }
    }

    /// Shorthand for the overwhelmingly common case:
    /// `.tooltip(Tooltip::text("Copy message"))`.
    pub fn text(
        label: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + Clone + 'static {
        let label = label.into();
        move |window, cx| {
            if label.trim().is_empty() {
                cx.new(|_| Empty).into()
            } else {
                bezel::ui::tooltip::Tooltip::text(label.clone(), window, cx)
            }
        }
    }

    /// Shorthand for tooltips with a keyboard shortcut badge:
    /// `.tooltip(Tooltip::with_shortcut("New task", "⌘N"))`.
    pub fn with_shortcut(
        label: impl Into<SharedString>,
        shortcut: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + Clone + 'static {
        let label = label.into();
        let shortcut = shortcut.into();
        move |window, cx| {
            if label.trim().is_empty() {
                cx.new(|_| Empty).into()
            } else {
                bezel::ui::tooltip::Tooltip::with_keystroke(
                    label.clone(),
                    shortcut.clone(),
                    window,
                    cx,
                )
            }
        }
    }

    /// Tooltip bound directly to an action.
    pub fn for_action(
        text: impl Into<SharedString>,
        action: &dyn gpui::Action,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyView {
        bezel::ui::tooltip::Tooltip::for_action(text, action, window, cx)
    }

    /// Tooltip bound directly to an action within a specific context.
    pub fn for_action_in(
        text: impl Into<SharedString>,
        action: &dyn gpui::Action,
        context: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyView {
        bezel::ui::tooltip::Tooltip::for_action_in(text, action, context, window, cx)
    }

    /// The card a tooltip paints, for a hover drawn inside another view.
    pub fn card(
        text: impl Into<SharedString>,
        keystroke: Option<SharedString>,
        cx: &App,
    ) -> AnyElement {
        let text = text.into();
        if text.trim().is_empty() {
            return div().into_any_element();
        }
        bezel::ui::tooltip::Tooltip::card(text, keystroke, cx)
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        if self.label.trim().is_empty() {
            return div().into_any_element();
        }
        bezel::ui::tooltip::Tooltip::card(self.label.clone(), self.shortcut.clone(), cx)
    }
}
