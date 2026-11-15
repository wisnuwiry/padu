//! A semantic toggle switch backed by bezel styling and focus traversal.

use std::rc::Rc;

use bezel::theme::Theme;
use bezel::ui::focus::pressable;
use gpui::{
    AnyView, App, ElementId, Hsla, IntoElement, Refineable, RenderOnce, StyleRefinement, Styled,
    Window, div, prelude::*, px,
};

type ToggleCallback = dyn Fn(bool, &mut Window, &mut App);

/// A semantic toggle switch backed by bezel styling and focus traversal.
#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    checked: bool,
    enabled: bool,
    accent: Option<Hsla>,
    tooltip: Option<Rc<dyn Fn(&mut Window, &mut App) -> AnyView>>,
    refinement: StyleRefinement,
    on_toggle: Option<Rc<ToggleCallback>>,
}

#[allow(dead_code)]
impl Switch {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            enabled: true,
            accent: None,
            tooltip: None,
            refinement: StyleRefinement::default(),
            on_toggle: None,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn accent(mut self, accent: Hsla) -> Self {
        self.accent = Some(accent);
        self
    }

    pub fn tooltip(mut self, build: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self {
        self.tooltip = Some(Rc::new(build));
        self
    }

    pub fn on_toggle(mut self, on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(on_toggle));
        self
    }
}

impl Styled for Switch {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.refinement
    }
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window.use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle());
        let focus = focus.read(cx).clone();
        let bezel_theme = Theme::of(cx);
        let padu_theme = crate::theme::Theme::current(cx);
        let scheme = crate::theme::ColorScheme::current(cx);
        let accent = self.accent.unwrap_or(padu_theme.accent);
        let enabled = self.enabled && self.on_toggle.is_some();
        let next_checked = !self.checked;

        let track_bg = if self.checked {
            accent
        } else {
            bezel_theme.ink(0.15)
        };

        let knob_bg = if self.checked {
            if padu_theme.is_dark && matches!(scheme, crate::theme::ColorScheme::Neutral) {
                padu_theme.on_inverse
            } else {
                gpui::rgb(0xFFFFFF).into()
            }
        } else {
            bezel_theme.ink(0.7)
        };

        let mut frame = div()
            .flex_none()
            .w(px(32.0))
            .h(px(18.0))
            .rounded_full()
            .bg(track_bg)
            .border_1()
            .border_color(gpui::transparent_black())
            .relative()
            .child(
                div()
                    .absolute()
                    .top(px(1.0))
                    .left(px(if self.checked { 15.0 } else { 1.0 }))
                    .size(px(14.0))
                    .rounded_full()
                    .bg(knob_bg),
            )
            .id(self.id)
            .role(gpui::Role::Switch)
            .aria_toggled(if self.checked {
                gpui::Toggled::True
            } else {
                gpui::Toggled::False
            })
            .when_some(self.tooltip, |el, tooltip| {
                el.tooltip(move |window, cx| tooltip(window, cx))
            })
            .when(enabled, |el| el.cursor_pointer())
            .when(!self.enabled, |el| el.opacity(0.55));

        frame.style().refine(&self.refinement);

        let on_toggle = self.on_toggle.clone();
        pressable(bezel_theme, &focus, frame, enabled, move |_, window, cx| {
            if let Some(on_toggle) = &on_toggle {
                on_toggle(next_checked, window, cx);
            }
        })
    }
}
