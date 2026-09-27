use gpui::*;

use super::super::*;
use crate::input::TextInput;
use crate::md;
use crate::md::render::TranscriptSelection;
use crate::theme::Theme;
use crate::ui::icon;
use crate::ui::tooltip::Tooltip;

impl Padu {
    pub(crate) fn apply_markdown_format(
        input: &Entity<TextInput>,
        prefix: &str,
        suffix: &str,
        default_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        input.update(cx, |this, cx| {
            let range = this.selected_range();
            let content = this.content().to_string();
            if range.is_empty() {
                let insert = format!("{prefix}{default_text}{suffix}");
                this.replace_range(range.clone(), &insert, cx);
                let start = range.start + prefix.len();
                let end = start + default_text.len();
                this.select_range(start..end, cx);
            } else {
                let selected = &content[range.clone()];
                let insert = format!("{prefix}{selected}{suffix}");
                let start = range.start;
                let end = start + insert.len();
                this.replace_range(range, &insert, cx);
                this.select_range(start..end, cx);
            }
        });
        let focus = input.read(cx).focus();
        window.focus(&focus, cx);
        cx.notify();
    }

    pub(crate) fn markdown_toolbar_btn(
        id: impl Into<ElementId>,
        label: impl IntoElement,
        tooltip_str: impl Into<SharedString>,
        theme: &Theme,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Window, &mut Context<Self>) + 'static + Clone,
    ) -> impl IntoElement {
        let tooltip_text = tooltip_str.into();
        let on_click_action = on_click.clone();
        let on_key_action = on_click;
        div()
            .id(id)
            .h(px(22.0))
            .px(px(6.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .text_size(sp(11.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.text_secondary)
            .hover(|s| s.bg(theme.overlay_strong).text_color(theme.text))
            .tab_index(0)
            .focus_visible(|s| s.border_color(theme.accent))
            .tooltip(Tooltip::text(tooltip_text))
            .child(label)
            .on_click(cx.listener(move |_this, _, window, cx| {
                on_click_action(window, cx);
            }))
            .on_key_down(cx.listener(move |_this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    on_key_action(window, cx);
                    cx.stop_propagation();
                }
            }))
    }

    pub(crate) fn render_board_markdown_editor(
        &self,
        field_id: &'static str,
        input: &Entity<TextInput>,
        is_preview: bool,
        min_height: Pixels,
        max_preview_height: Pixels,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_toggle_preview: impl Fn(&mut Self, bool, &mut Context<Self>) + 'static + Clone,
    ) -> Div {
        let on_edit_click = on_toggle_preview.clone();
        let on_edit_key = on_toggle_preview.clone();
        let on_prev_click = on_toggle_preview.clone();
        let on_prev_key = on_toggle_preview;

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(sp(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_secondary)
                    .child(tr!("board.task_description")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .p(px(2.0))
                    .rounded(px(6.0))
                    .bg(theme.inset)
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id(SharedString::from(format!("{field_id}-tab-edit")))
                            .h(px(22.0))
                            .px(px(8.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .text_size(sp(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .when(!is_preview, |tab| {
                                tab.bg(theme.surface)
                                    .text_color(theme.text)
                                    .border_1()
                                    .border_color(theme.border)
                            })
                            .when(is_preview, |tab| {
                                tab.text_color(theme.text_tertiary)
                                    .hover(|s| s.text_color(theme.text))
                            })
                            .tab_index(0)
                            .focus_visible(|s| s.border_color(theme.accent))
                            .child(icon(
                                "icons/pencil.svg",
                                11.0,
                                if !is_preview {
                                    theme.text
                                } else {
                                    theme.text_tertiary
                                },
                            ))
                            .child(tr!("board.edit"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                on_edit_click(this, false, cx);
                            }))
                            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    on_edit_key(this, false, cx);
                                    cx.stop_propagation();
                                }
                            })),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("{field_id}-tab-preview")))
                            .h(px(22.0))
                            .px(px(8.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .text_size(sp(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .when(is_preview, |tab| {
                                tab.bg(theme.surface)
                                    .text_color(theme.text)
                                    .border_1()
                                    .border_color(theme.border)
                            })
                            .when(!is_preview, |tab| {
                                tab.text_color(theme.text_tertiary)
                                    .hover(|s| s.text_color(theme.text))
                            })
                            .tab_index(0)
                            .focus_visible(|s| s.border_color(theme.accent))
                            .child(icon(
                                "icons/eye.svg",
                                11.0,
                                if is_preview {
                                    theme.text
                                } else {
                                    theme.text_tertiary
                                },
                            ))
                            .child(tr!("board.preview"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                on_prev_click(this, true, cx);
                            }))
                            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    on_prev_key(this, true, cx);
                                    cx.stop_propagation();
                                }
                            })),
                    ),
            );

        let mut editor_container = div().flex().flex_col().gap(px(6.0)).child(header);

        if is_preview {
            let content = input.read(cx).content().to_string();
            let (display_text, is_empty) = if content.trim().is_empty() {
                (String::from("_No description provided._"), true)
            } else {
                (content, false)
            };
            let mut cache = self.board_markdown_preview.borrow_mut();
            cache.set_text(&display_text, false);
            let palette = MarkdownPalette::from_theme(theme);
            let ctx = MarkdownCtx::new(
                format!("{field_id}-preview"),
                &palette,
                self.scaled_markdown_metrics(MarkdownMetrics::COMPACT),
                TranscriptSelection::default(),
            );
            let rendered = md::render::markdown(&cache, &ctx).unwrap_or_else(|| {
                div()
                    .text_size(sp(12.0))
                    .text_color(theme.text_tertiary)
                    .child(display_text)
                    .into_any_element()
            });
            drop(cache);

            editor_container = editor_container.child(
                div()
                    .id(SharedString::from(format!("{field_id}-preview-pane")))
                    .min_h(min_height)
                    .max_h(max_preview_height)
                    .overflow_y_scroll()
                    .rounded(px(6.0))
                    .bg(theme.inset)
                    .border_1()
                    .border_color(theme.border)
                    .p(px(10.0))
                    .when(is_empty, |s| s.text_color(theme.text_tertiary))
                    .child(rendered),
            );
        } else {
            let input_bold = input.clone();
            let input_italic = input.clone();
            let input_heading = input.clone();
            let input_code = input.clone();
            let input_codeblock = input.clone();
            let input_list = input.clone();
            let input_checklist = input.clone();
            let input_link = input.clone();

            let toolbar = div()
                .flex()
                .items_center()
                .gap(px(3.0))
                .p(px(2.0))
                .rounded(px(6.0))
                .bg(theme.surface)
                .border_1()
                .border_color(theme.border)
                .child(Self::markdown_toolbar_btn(
                    SharedString::from(format!("{field_id}-btn-bold")),
                    div().font_weight(FontWeight::BOLD).child("B"),
                    "Bold (**text**)",
                    theme,
                    cx,
                    move |window, cx| {
                        Self::apply_markdown_format(
                            &input_bold,
                            "**",
                            "**",
                            "bold text",
                            window,
                            cx,
                        );
                    },
                ))
                .child(Self::markdown_toolbar_btn(
                    SharedString::from(format!("{field_id}-btn-italic")),
                    div().font_weight(FontWeight::BOLD).child("I"),
                    "Italic (*text*)",
                    theme,
                    cx,
                    move |window, cx| {
                        Self::apply_markdown_format(
                            &input_italic,
                            "*",
                            "*",
                            "italic text",
                            window,
                            cx,
                        );
                    },
                ))
                .child(Self::markdown_toolbar_btn(
                    SharedString::from(format!("{field_id}-btn-heading")),
                    div().font_weight(FontWeight::BOLD).child("H"),
                    "Heading (### title)",
                    theme,
                    cx,
                    move |window, cx| {
                        Self::apply_markdown_format(
                            &input_heading,
                            "### ",
                            "",
                            "Heading",
                            window,
                            cx,
                        );
                    },
                ))
                .child(Self::markdown_toolbar_btn(
                    SharedString::from(format!("{field_id}-btn-code")),
                    div().font_weight(FontWeight::MEDIUM).child("` `"),
                    "Inline code (`code`)",
                    theme,
                    cx,
                    move |window, cx| {
                        Self::apply_markdown_format(&input_code, "`", "`", "code", window, cx);
                    },
                ))
                .child(Self::markdown_toolbar_btn(
                    SharedString::from(format!("{field_id}-btn-codeblock")),
                    div().font_weight(FontWeight::MEDIUM).child("```"),
                    "Code block (```)",
                    theme,
                    cx,
                    move |window, cx| {
                        Self::apply_markdown_format(
                            &input_codeblock,
                            "```\n",
                            "\n```",
                            "code",
                            window,
                            cx,
                        );
                    },
                ))
                .child(Self::markdown_toolbar_btn(
                    SharedString::from(format!("{field_id}-btn-list")),
                    icon("icons/list.svg", 12.0, theme.text_secondary),
                    "Bullet list (- item)",
                    theme,
                    cx,
                    move |window, cx| {
                        Self::apply_markdown_format(&input_list, "- ", "", "item", window, cx);
                    },
                ))
                .child(Self::markdown_toolbar_btn(
                    SharedString::from(format!("{field_id}-btn-checklist")),
                    icon("icons/list-checks.svg", 12.0, theme.text_secondary),
                    "Task list (- [ ] task)",
                    theme,
                    cx,
                    move |window, cx| {
                        Self::apply_markdown_format(
                            &input_checklist,
                            "- [ ] ",
                            "",
                            "task",
                            window,
                            cx,
                        );
                    },
                ))
                .child(Self::markdown_toolbar_btn(
                    SharedString::from(format!("{field_id}-btn-link")),
                    icon("icons/external-link.svg", 12.0, theme.text_secondary),
                    "Link ([text](url))",
                    theme,
                    cx,
                    move |window, cx| {
                        Self::apply_markdown_format(
                            &input_link,
                            "[",
                            "](url)",
                            "link text",
                            window,
                            cx,
                        );
                    },
                ));

            let is_focused = input.read(cx).is_visually_focused(window);
            let input_focus = input.clone();

            editor_container = editor_container.child(toolbar).child(
                div()
                    .id(SharedString::from(format!("{field_id}-edit-box")))
                    .min_h(min_height)
                    .rounded(px(6.0))
                    .bg(theme.inset)
                    .border_1()
                    .border_color(if is_focused {
                        theme.accent
                    } else {
                        theme.border
                    })
                    .p(px(8.0))
                    .cursor_text()
                    .on_click(cx.listener(move |_, _, window, cx| {
                        let focus = input_focus.read(cx).focus();
                        window.focus(&focus, cx);
                    }))
                    .child(input.clone()),
            );
        }

        editor_container
    }
}
