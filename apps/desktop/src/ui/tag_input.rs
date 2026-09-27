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
    on_remove: Option<Arc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
}

impl TagInput {
    pub fn new(id: impl Into<ElementId>, input: gpui::Entity<TextInput>) -> Self {
        Self {
            base: div().id(id),
            input,
            tags: Vec::new(),
            bordered: true,
            on_remove: None,
        }
    }

    /// Whether to render a default border around the container.
    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
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

        let mut container = self
            .base
            .min_h(px(34.0))
            .p(px(6.0))
            .rounded(px(6.0))
            .bg(theme.inset)
            .when(bordered, |s| {
                s.border_1()
                    .border_color(if focused { theme.accent } else { theme.border })
            })
            .when(!bordered && focused, |s| {
                s.border_1().border_color(theme.accent)
            })
            .when(!bordered && !focused, |s| s.border_0())
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
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
                    .h(px(22.0))
                    .px(px(7.0))
                    .rounded(px(4.0))
                    .bg(theme.overlay)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .text_size(sp(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(tag.clone())
                    .child(
                        div()
                            .id(SharedString::from(format!("tag-chip-remove-{}", i)))
                            .cursor_pointer()
                            .text_color(theme.text_tertiary)
                            .hover(|s| s.text_color(theme.danger))
                            .tab_index(0)
                            .focus_visible(|style| style.border_color(theme.accent))
                            .child(icon("icons/x.svg", 10.0, theme.text_tertiary))
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
        container.child(div().flex_1().min_w(px(80.0)).h(px(22.0)).child(self.input))
    }
}
