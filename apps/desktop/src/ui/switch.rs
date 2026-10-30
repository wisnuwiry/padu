//! A semantic toggle switch backed by bezel styling and focus traversal.

use std::rc::Rc;

use bezel::theme::Theme;
use bezel::ui::focus::pressable;
use bezel::ui::widgets::Controls;
use gpui::{
    AnyView, App, ElementId, IntoElement, Refineable, RenderOnce, StyleRefinement, Styled, Window,
    prelude::*,
};

type ToggleCallback = dyn Fn(bool, &mut Window, &mut App);

/// A semantic toggle switch backed by bezel styling and focus traversal.
#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    checked: bool,
    enabled: bool,
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
            tooltip: None,
            refinement: StyleRefinement::default(),
            on_toggle: None,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
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
        let enabled = self.enabled && self.on_toggle.is_some();
        let next_checked = !self.checked;

        let mut frame = bezel_theme
            .toggle(self.checked)
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
