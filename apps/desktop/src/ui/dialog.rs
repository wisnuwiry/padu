use gpui::prelude::*;
use gpui::{
    AnyElement, Context, ElementId, FocusHandle, FontWeight, Hsla, KeyDownEvent, MouseButton,
    Pixels, SharedString, Stateful, Window, div, px,
};

use crate::theme::{Theme, sp};
use crate::ui::icon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmVariant {
    Default,
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
        .child(card);

    gpui::deferred(layer).with_priority(5).into_any_element()
}

/// Standard modal card container.
pub fn dialog_card(id: impl Into<ElementId>, theme: &Theme, width: Pixels) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .tab_group()
        .w(width)
        .rounded(px(14.0))
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

/// Standardized 32px height cancel button with focus visibility and Esc badge.
pub fn dialog_cancel_button<V: 'static>(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    focus: &FocusHandle,
    theme: &Theme,
    cx: &mut Context<V>,
    on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> Stateful<gpui::Div> {
    let on_click = std::rc::Rc::new(on_click);
    let click_action = on_click.clone();
    let key_action = on_click;

    div()
        .id(id)
        .track_focus(focus)
        .tab_index(0)
        .h(px(32.0))
        .px(px(14.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(theme.border_strong)
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .cursor_pointer()
        .text_size(sp(13.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.text_secondary)
        .hover(|s| s.bg(theme.overlay).text_color(theme.text))
        .active(|s| s.bg(theme.overlay_strong))
        .focus_visible(|s| s.border_1().border_color(theme.accent))
        .child(label.into())
        .child(crate::ui::kbd_badge("Esc", theme))
        .on_click(cx.listener(move |view, _, window, cx| {
            click_action(view, window, cx);
        }))
        .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
            if !event.keystroke.modifiers.modified()
                && matches!(event.keystroke.key.as_str(), "enter" | "space" | "escape")
            {
                key_action(view, window, cx);
                cx.stop_propagation();
            }
        }))
}

/// Standardized 32px height confirm button supporting Default and Danger variants.
pub fn dialog_confirm_button<V: 'static>(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    focus: &FocusHandle,
    variant: ConfirmVariant,
    theme: &Theme,
    cx: &mut Context<V>,
    on_confirm: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> Stateful<gpui::Div> {
    let on_confirm = std::rc::Rc::new(on_confirm);
    let click_action = on_confirm.clone();
    let key_action = on_confirm;

    let (bg, text_color, badge_border) = match variant {
        ConfirmVariant::Danger => (theme.danger, gpui::white(), gpui::hsla(0.0, 0.0, 1.0, 0.25)),
        ConfirmVariant::Default => (
            theme.inverse,
            theme.on_inverse,
            gpui::hsla(0.0, 0.0, 1.0, 0.20),
        ),
    };

    div()
        .id(id)
        .track_focus(focus)
        .tab_index(0)
        .h(px(32.0))
        .px(px(14.0))
        .rounded(px(7.0))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .cursor_pointer()
        .text_size(sp(13.0))
        .font_weight(FontWeight::MEDIUM)
        .bg(bg)
        .text_color(text_color)
        .hover(|s| s.opacity(0.9))
        .focus_visible(|s| s.border_1().border_color(theme.accent))
        .child(label.into())
        .child(crate::ui::kbd_badge_icon(
            "icons/corner-down-left.svg",
            bg,
            text_color,
            badge_border,
        ))
        .on_click(cx.listener(move |view, _, window, cx| {
            click_action(view, window, cx);
        }))
        .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
            if !event.keystroke.modifiers.modified()
                && matches!(event.keystroke.key.as_str(), "enter" | "space")
            {
                key_action(view, window, cx);
                cx.stop_propagation();
            }
        }))
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
    cancel_focus: &FocusHandle,
    confirm_focus: &FocusHandle,
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
        cancel_focus,
        theme,
        cx,
        move |view, window, cx| on_cancel(view, window, cx),
    );

    let confirm_button = dialog_confirm_button(
        "confirm-dialog-confirm",
        confirm_label,
        confirm_focus,
        variant,
        theme,
        cx,
        move |view, window, cx| on_confirm(view, window, cx),
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
