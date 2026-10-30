use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, ElementId, FontWeight, Hsla, KeyDownEvent, MouseButton, Pixels,
    SharedString, Stateful, Window, div, px,
};

use crate::theme::{Theme, sp};
use crate::ui::icon;
use bezel::ui::widgets::{Button, ButtonRole, ButtonStyle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmVariant {
    Danger,
}

/// Standardized backdrop scrim color across all modal surfaces.
pub fn dialog_scrim_color(theme: &Theme) -> Hsla {
    if theme.is_dark {
        gpui::hsla(0.0, 0.0, 0.0, 0.40)
    } else {
        gpui::hsla(0.0, 0.0, 0.0, 0.20)
    }
}

/// Standardized full-screen dialog backdrop with outside-click dismissal
/// and elevated deferred priority so it paints above standard window content.
pub fn dialog_backdrop<V: 'static>(
    id: impl Into<ElementId>,
    theme: &Theme,
    cx: &mut Context<V>,
    on_dismiss: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
    card: impl IntoElement,
) -> AnyElement {
    let id = id.into();
    let motion_id = ElementId::Name(format!("{}-in", id).into());
    let layer = div()
        .id(id)
        .absolute()
        .inset_0()
        .occlude()
        .bg(dialog_scrim_color(theme))
        .p(px(24.0))
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |view, _, window, cx| {
                on_dismiss(view, window, cx);
            }),
        )
        .child(bezel::motion::dialog_in(
            motion_id,
            div().occlude().child(card),
        ));

    gpui::deferred(layer).with_priority(5).into_any_element()
}

/// Standard modal card container with keyboard traversal support.
pub fn dialog_card(id: impl Into<ElementId>, theme: &Theme, width: Pixels) -> Stateful<gpui::Div> {
    bezel::ui::focus::traversal(div())
        .id(id)
        .tab_group()
        .w(width)
        .rounded(px(bezel::theme::Theme::panel_radius()))
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border_strong)
        .shadow_xl()
        .flex()
        .flex_col()
        .p(px(20.0))
        .gap(px(14.0))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// Standardized cancel button backed by bezel's Button component.
pub fn dialog_cancel_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&(), &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id, label)
        .button_style(ButtonStyle::Ghost)
        .role(ButtonRole::Cancel)
        .on_press(on_click)
}

/// Standardized confirmation button backed by bezel's Button component.
pub fn dialog_confirm_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    variant: ConfirmVariant,
    on_confirm: impl Fn(&(), &mut Window, &mut App) + 'static,
) -> Button {
    let (style, role) = match variant {
        ConfirmVariant::Danger => (ButtonStyle::Destructive, ButtonRole::Destructive),
    };
    Button::new(id, label)
        .button_style(style)
        .role(role)
        .on_press(on_confirm)
}

/// Renders the standard confirmation card containing header, message, and Cancel/Confirm actions.
#[allow(clippy::too_many_arguments)]
pub fn render_confirm_dialog_card<V: 'static>(
    id: impl Into<ElementId>,
    title: SharedString,
    message: SharedString,
    confirm_label: SharedString,
    cancel_label: SharedString,
    variant: ConfirmVariant,
    icon_name: Option<&'static str>,
    theme: &Theme,
    cx: &mut Context<V>,
    on_confirm: impl Fn(&mut V, &mut Window, &mut Context<V>) + Copy + 'static,
    on_cancel: impl Fn(&mut V, &mut Window, &mut Context<V>) + Copy + 'static,
) -> Stateful<gpui::Div> {
    let is_danger = variant == ConfirmVariant::Danger;
    let icon_tint = if is_danger {
        theme.danger
    } else {
        theme.text_secondary
    };
    let icon_bg = if is_danger {
        theme.danger.opacity(0.12)
    } else {
        theme.overlay
    };

    let cancel_button = dialog_cancel_button(
        "confirm-dialog-cancel",
        cancel_label,
        cx.listener(move |view, _, window, cx| on_cancel(view, window, cx)),
    );

    let confirm_button = dialog_confirm_button(
        "confirm-dialog-confirm",
        confirm_label,
        variant,
        cx.listener(move |view, _, window, cx| on_confirm(view, window, cx)),
    );

    let card = dialog_card(id, theme, px(420.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .when_some(icon_name, |header, name| {
                    header.child(
                        div()
                            .size(px(32.0))
                            .rounded(px(8.0))
                            .bg(icon_bg)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon(name, 16.0, icon_tint)),
                    )
                })
                .child(
                    div()
                        .text_size(sp(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text)
                        .child(title),
                ),
        )
        .child(
            div()
                .text_size(sp(13.5))
                .line_height(sp(20.0))
                .text_color(theme.text_secondary)
                .child(message),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap(px(8.0))
                .pt(px(6.0))
                .child(cancel_button)
                .child(confirm_button),
        )
        .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
            if !event.keystroke.modifiers.modified() {
                match event.keystroke.key.as_str() {
                    "enter" => {
                        on_confirm(view, window, cx);
                        cx.stop_propagation();
                    }
                    "escape" => {
                        on_cancel(view, window, cx);
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }
        }));

    card
}
