//! Semantic slider control styled with Padu's active color scheme.

use gpui::{Div, Hsla, div, prelude::*, px};

/// Builds a slider track and knob styled with the active color scheme accent.
pub fn slider(fraction: f32, bezel_theme: &bezel::theme::Theme, accent: Hsla) -> Div {
    let fraction = fraction.clamp(0.0, 1.0);
    div()
        .w_full()
        .h(px(16.0))
        .border_1()
        .border_color(gpui::transparent_black())
        .rounded(px(4.0))
        .flex()
        .items_center()
        .relative()
        .cursor_pointer()
        .child(
            div()
                .w_full()
                .h(px(4.0))
                .rounded_full()
                .bg(bezel_theme.ink(0.12))
                .child(
                    div()
                        .h_full()
                        .w(gpui::relative(fraction))
                        .rounded_full()
                        .bg(accent),
                ),
        )
        .child(
            div()
                .absolute()
                .left(gpui::relative(fraction))
                .child(div().size(px(14.0)).ml(px(-7.0)).rounded_full().bg(accent)),
        )
}
