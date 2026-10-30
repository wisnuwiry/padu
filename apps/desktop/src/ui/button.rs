//! A semantic button with bezel styling, stable focus, one activation callback,
//! and optional trailing keyboard shortcut badge.

use std::rc::Rc;

use bezel::theme::{ControlSize, Sizing, Theme};
use bezel::ui::focus::pressable;
use gpui::{
    App, ElementId, IntoElement, Refineable, RenderOnce, SharedString, StyleRefinement, Styled,
    Window, div, prelude::*, px,
};

pub use bezel::ui::widgets::{ButtonRole, ButtonStyle};

use super::{icon, kbd_badge_icon, kbd_badge_styled};

#[allow(dead_code)]
#[derive(Clone)]
pub enum ButtonKbd {
    Label(SharedString),
    Icon(&'static str),
}

type Activation = dyn Fn(&(), &mut Window, &mut App);

/// A semantic button backed by bezel styling and focus traversal.
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    icon: Option<&'static str>,
    label_hidden: bool,
    style: ButtonStyle,
    size: ControlSize,
    role: Option<ButtonRole>,
    kbd: Option<ButtonKbd>,
    enabled: bool,
    refinement: StyleRefinement,
    activate: Option<Rc<Activation>>,
}

#[allow(dead_code)]
impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            label_hidden: false,
            style: ButtonStyle::Ghost,
            size: ControlSize::Regular,
            role: None,
            kbd: None,
            enabled: true,
            refinement: StyleRefinement::default(),
            activate: None,
        }
    }

    pub fn button_style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn role(mut self, role: ButtonRole) -> Self {
        self.role = Some(role);
        self
    }

    pub fn icon(mut self, icon_path: &'static str) -> Self {
        self.icon = Some(icon_path);
        self
    }

    pub fn label_hidden(mut self) -> Self {
        self.label_hidden = true;
        self
    }

    pub fn kbd(mut self, label: impl Into<SharedString>) -> Self {
        self.kbd = Some(ButtonKbd::Label(label.into()));
        self
    }

    pub fn kbd_icon(mut self, icon_path: &'static str) -> Self {
        self.kbd = Some(ButtonKbd::Icon(icon_path));
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn on_press(mut self, activate: impl Fn(&(), &mut Window, &mut App) + 'static) -> Self {
        self.activate = Some(Rc::new(activate));
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.refinement
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window.use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle());
        let focus = focus.read(cx).clone();
        let bezel_theme = Theme::of(cx);
        let enabled = self.enabled && self.activate.is_some();
        let destructive =
            self.role == Some(ButtonRole::Destructive) || self.style == ButtonStyle::Destructive;

        let frame = div()
            .control_size(self.size)
            .py(px((self.size.pad_y() - 1.0).max(0.0)))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .border_1()
            .border_color(gpui::hsla(0.0, 0.0, 0.0, 0.0))
            .when(self.label_hidden, |el| {
                el.px(px(0.0)).w(px(self.size.height()))
            });

        let (mut frame, tint) = if self.style == ButtonStyle::Ghost {
            let rest = if destructive {
                bezel_theme.danger_strong
            } else {
                bezel_theme.text_muted
            };
            (frame.text_color(rest), rest)
        } else {
            let (fill, text_color) = if destructive {
                (bezel_theme.danger_strong, gpui::white())
            } else {
                (bezel_theme.text, bezel_theme.on_solid)
            };
            (
                frame
                    .bg(fill)
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(text_color)
                    .when(enabled, |btn| btn.hover(|s| s.opacity(0.9))),
                text_color,
            )
        };

        if enabled && self.style == ButtonStyle::Ghost {
            frame = frame.hover(|style| style.bg(bezel_theme.element_hover));
        }

        let tint = self.refinement.text.color.unwrap_or(tint);
        frame.style().refine(&self.refinement);

        let kbd_badge = self.kbd.map(|kbd| {
            let (bg, text_color, border_color) = if self.style == ButtonStyle::Ghost {
                (
                    bezel_theme.element_hover,
                    bezel_theme.text_muted,
                    bezel_theme.border,
                )
            } else if destructive {
                (
                    gpui::hsla(0.0, 0.0, 1.0, 0.18),
                    gpui::white(),
                    gpui::hsla(0.0, 0.0, 1.0, 0.28),
                )
            } else {
                (
                    gpui::hsla(0.0, 0.0, 1.0, 0.18),
                    bezel_theme.on_solid,
                    gpui::hsla(0.0, 0.0, 1.0, 0.28),
                )
            };
            match kbd {
                ButtonKbd::Label(kbd_label) => {
                    kbd_badge_styled(kbd_label, bg, text_color, border_color)
                }
                ButtonKbd::Icon(icon_path) => {
                    kbd_badge_icon(icon_path, bg, text_color, border_color)
                }
            }
        });

        let frame = frame
            .when_some(self.icon, |el, icon_path| {
                el.child(icon(icon_path, 14.0, tint))
            })
            .when(!self.label_hidden, |el| el.child(self.label.clone()))
            .children(kbd_badge)
            .when(enabled, |el| el.cursor_pointer())
            .when(!enabled, |el| el.opacity(0.5))
            .id(self.id)
            .role(gpui::Role::Button)
            .aria_label(self.label);

        pressable(
            bezel_theme,
            &focus,
            frame,
            enabled,
            move |event, window, cx| {
                if let Some(activate) = &self.activate {
                    activate(event, window, cx);
                }
            },
        )
    }
}
