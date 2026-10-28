use std::sync::Arc;

use gpui::{
    AnyElement, App, Div, ElementId, FontWeight, InteractiveElement, Interactivity, IntoElement,
    KeyDownEvent, ParentElement, RenderOnce, SharedString, Stateful, StyleRefinement, Styled,
    Window, div, prelude::*, px,
};

use crate::input::TextInput;
use crate::theme::{Theme, sp};

use super::icon;

/// A common tag/label chip input shell: displays tokens as compact chips
/// inside the input container, followed by an inline [`TextInput`] to add more.
/// Clicking anywhere in the container focuses the input.
#[derive(IntoElement)]
pub struct TagInput {
    base: Stateful<Div>,
    input: gpui::Entity<TextInput>,
    tags: Vec<SharedString>,
    bordered: bool,
    focus_outline: bool,
    small: bool,
    accent_chips: bool,
    on_remove: Option<Arc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
}

impl TagInput {
    pub fn new(id: impl Into<ElementId>, input: gpui::Entity<TextInput>) -> Self {
        Self {
            base: div().id(id),
            input,
            tags: Vec::new(),
            bordered: true,
            focus_outline: true,
            small: false,
            accent_chips: false,
            on_remove: None,
        }
    }

    /// Whether to render a default border around the container.
    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    /// Whether to show an outline/border highlight when the input is focused.
    pub fn focus_outline(mut self, focus_outline: bool) -> Self {
        self.focus_outline = focus_outline;
        self
    }

    /// Whether to use the compact/small size styling for the container and chips.
    pub fn small(mut self, small: bool) -> Self {
        self.small = small;
        self
    }

    /// Whether chips should be styled with primary/accent outline and text.
    pub fn accent_chips(mut self, accent_chips: bool) -> Self {
        self.accent_chips = accent_chips;
        self
    }

    /// The list of tags/labels to render as chips inside the input container.
    pub fn tags(mut self, tags: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// Callback invoked with the tag index when a chip's remove button (×) is clicked.
    pub fn on_remove(mut self, on_remove: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_remove = Some(Arc::new(on_remove));
        self
    }
}

impl Styled for TagInput {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for TagInput {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl ParentElement for TagInput {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl RenderOnce for TagInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let focused = self.input.read(cx).is_visually_focused(window);
        let input_clone = self.input.clone();
        let on_remove = self.on_remove;

        let bordered = self.bordered;
        let show_focus_border = focused && self.focus_outline;
        let is_small = self.small;
        let accent_chips = self.accent_chips;

        let mut container = self
            .base
            .when(is_small, |s| {
                s.min_h(px(24.0))
                    .py(px(1.5))
                    .px(px(4.0))
                    .rounded(px(5.0))
                    .gap(px(4.0))
            })
            .when(!is_small, |s| {
                s.min_h(px(34.0))
                    .p(px(6.0))
                    .rounded(px(6.0))
                    .gap(px(6.0))
            })
            .bg(theme.inset)
            .when(bordered, |s| {
                s.border_1().border_color(if show_focus_border {
                    theme.accent
                } else {
                    theme.border
                })
            })
            .when(!bordered && show_focus_border, |s| {
                s.border_1().border_color(theme.accent)
            })
            .when(!bordered && !show_focus_border, |s| s.border_0())
            .flex()
            .flex_wrap()
            .items_center()
            .cursor_text()
            .on_click(move |_, window, cx| {
                let focus = input_clone.read(cx).focus();
                window.focus(&focus, cx);
            });

        for (i, tag) in self.tags.iter().enumerate() {
            let remove_cb = on_remove.clone();
            container = container.child(
                div()
                    .id(SharedString::from(format!("tag-chip-{}", i)))
                    .when(is_small, |s| {
                        s.h(px(18.0))
                            .px(px(5.0))
                            .rounded(px(3.5))
                            .gap(px(3.0))
                            .text_size(sp(10.0))
                    })
                    .when(!is_small, |s| {
                        s.h(px(22.0))
                            .px(px(7.0))
                            .rounded(px(4.0))
                            .gap(px(4.0))
                            .text_size(sp(11.0))
                    })
                    .when(accent_chips, |s| {
                        s.bg(theme.accent.opacity(0.12))
                            .border_1()
                            .border_color(theme.accent.opacity(0.40))
                            .text_color(theme.accent)
                    })
                    .when(!accent_chips, |s| {
                        s.bg(theme.overlay)
                            .border_1()
                            .border_color(theme.border)
                            .text_color(theme.text)
                    })
                    .flex()
                    .items_center()
                    .font_weight(FontWeight::MEDIUM)
                    .child(tag.clone())
                    .child(
                        div()
                            .id(SharedString::from(format!("tag-chip-remove-{}", i)))
                            .cursor_pointer()
                            .text_color(if accent_chips {
                                theme.accent.opacity(0.7)
                            } else {
                                theme.text_tertiary
                            })
                            .hover(|s| s.text_color(theme.danger))
                            .tab_index(0)
                            .focus_visible(|style| style.border_color(theme.accent))
                            .child(icon(
                                "icons/x.svg",
                                if is_small { 9.0 } else { 10.0 },
                                if accent_chips {
                                    theme.accent.opacity(0.7)
                                } else {
                                    theme.text_tertiary
                                },
                            ))
                            .on_click({
                                let remove_cb = remove_cb.clone();
                                move |_, window, cx| {
                                    if let Some(cb) = &remove_cb {
                                        cb(i, window, cx);
                                    }
                                }
                            })
                            .on_key_down({
                                let remove_cb = remove_cb.clone();
                                move |event: &KeyDownEvent, window, cx| {
                                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                        if let Some(cb) = &remove_cb {
                                            cb(i, window, cx);
                                        }
                                        cx.stop_propagation();
                                    }
                                }
                            }),
                    ),
            );
        }

        // Inline text input to type new tags
        container.child(
            div()
                .flex_1()
                .min_w(if is_small { px(60.0) } else { px(80.0) })
                .h(if is_small { px(18.0) } else { px(22.0) })
                .when(is_small, |s| s.text_size(sp(10.5)))
                .child(self.input),
        )
    }
}
