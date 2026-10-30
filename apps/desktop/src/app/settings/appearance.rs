use std::cell::Cell;

use super::*;
use crate::ui::Switch;
use crate::ui::button::{Button, ButtonRole, ButtonStyle};
use bezel::ui::combobox::{Combobox, ComboboxEvent};
use bezel::ui::widgets::{Controls, SliderDrag, slider_fraction};
use gpui::{Bounds, DragMoveEvent, ElementId, MouseButton, MouseDownEvent, SharedString, canvas};

fn font_size_label(size: f32) -> String {
    if size.fract() == 0.0 {
        format!("{size:.0} px")
    } else {
        format!("{size} px")
    }
}

fn render_background_section(padu: &Padu, theme: Theme, cx: &mut Context<Padu>) -> AnyElement {
    let bezel_theme = bezel::theme::Theme::of(cx);
    let settings = &padu.state.conversation_background;
    let image = padu.conversation_background_image();
    let weak = cx.entity().downgrade();
    let choose = Button::new(
        "conversation-background-choose",
        tr!("settings.background_choose"),
    )
    .button_style(ButtonStyle::Ghost)
    .icon("icons/folder-open.svg")
    .on_press({
        let weak = weak.clone();
        move |_, _, cx| {
            let _ = weak.update(cx, |this, cx| this.choose_conversation_background(cx));
        }
    });
    let opacity = (settings.opacity * 100.0).round() as i32;
    let height = settings.height_percent.round() as i32;
    let preview = div()
        .relative()
        .w_full()
        .aspect_ratio(188.0 / 142.0)
        .min_h(px(240.0))
        .overflow_hidden()
        .rounded(px(8.0))
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border)
        .children(
            crate::app::conversation_background::render_conversation_background(
                image.clone(),
                settings,
                &theme,
                Some(px(8.0)),
            ),
        )
        .child(
            div()
                .relative()
                .h_full()
                .p(px(16.0))
                .flex()
                .flex_col()
                .justify_end()
                .gap(px(3.0))
                .child(render_preview_message(
                    MessageRole::User,
                    tr!("settings.background_preview_user"),
                    &theme,
                ))
                .child(render_preview_message(
                    MessageRole::Assistant,
                    tr!("settings.background_preview_assistant"),
                    &theme,
                )),
        );
    let remove = settings.image_path.as_ref().map(|_| {
        let weak = cx.entity().downgrade();
        Button::new(
            "conversation-background-remove",
            tr!("settings.background_remove"),
        )
        .button_style(ButtonStyle::Ghost)
        .role(ButtonRole::Destructive)
        .icon("icons/trash.svg")
        .on_press(move |_, _, cx| {
            let _ = weak.update(cx, |this, cx| this.clear_conversation_background(cx));
        })
        .into_any_element()
    });
    div()
        .mt(px(15.0))
        .w_full()
        .rounded(px(13.0))
        .overflow_hidden()
        .bg(theme.raised)
        .px(px(20.0))
        .py(px(16.0))
        .flex()
        .flex_col()
        .items_start()
        .gap(px(12.0))
        .child(
            div()
                .w_full()
                .child(
                    div()
                        .text_size(sp(13.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(tr!("settings.background")),
                )
                .child(
                    div()
                        .mt(px(5.0))
                        .text_size(sp(12.5))
                        .line_height(sp(18.0))
                        .text_color(theme.text_secondary)
                        .child(tr!("settings.background_description")),
                ),
        )
        .child(
            div()
                .w_full()
                .flex()
                .items_start()
                .gap(px(16.0))
                .child(
                    div()
                        .w(px(190.0))
                        .flex_none()
                        .flex()
                        .flex_col()
                        .gap(px(12.0))
                        .child(choose)
                        .children(remove)
                        .child(div().w_full().h(px(1.0)).my(px(2.0)).bg(theme.border))
                        .child(background_adjuster(
                            "background-opacity",
                            tr!("settings.background_opacity"),
                            opacity as f32,
                            0.0,
                            100.0,
                            theme,
                            bezel_theme,
                            {
                                let weak = cx.entity().downgrade();
                                move |value, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.set_conversation_background_opacity(value / 100.0, cx)
                                    });
                                }
                            },
                        ))
                        .child(background_adjuster(
                            "background-height",
                            tr!("settings.background_height"),
                            height as f32,
                            20.0,
                            100.0,
                            theme,
                            bezel_theme,
                            {
                                let weak = cx.entity().downgrade();
                                move |value, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.set_conversation_background_height(value, cx)
                                    });
                                }
                            },
                        )),
                )
                .child(div().flex_1().min_w_0().child(preview)),
        )
        .into_any_element()
}

fn font_size_slider(
    id_prefix: &'static str,
    value: f32,
    min: f32,
    max: f32,
    theme: Theme,
    bezel_theme: &bezel::theme::Theme,
    on_change: impl Fn(f32, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let value = value.clamp(min, max);
    let fraction = if max > min {
        (value - min) / (max - min)
    } else {
        0.0
    };
    let on_change: Rc<dyn Fn(f32, &mut Window, &mut App)> = Rc::new(on_change);
    let slider_id = format!("{id_prefix}-slider");
    let control_id = format!("{id_prefix}-control");
    let slider_on_change = on_change.clone();
    let bounds_cell = Rc::new(Cell::new(None));

    let bounds_ref = bounds_cell.clone();
    let bounds_recorder = canvas(
        move |bounds: Bounds<Pixels>, _, _| bounds_ref.set(Some(bounds)),
        |_, _, _, _| (),
    )
    .absolute()
    .inset_0();

    let drag_id: ElementId = control_id.clone().into();
    let drag_move_on_change = on_change.clone();
    let mouse_down_on_change = on_change.clone();

    let slider_control = bezel_theme
        .slider(fraction)
        .child(bounds_recorder)
        .id(control_id)
        .on_drag(SliderDrag(drag_id.clone()), |_, _, _, cx| {
            cx.new(|_| gpui::Empty)
        })
        .on_drag_move(move |event: &DragMoveEvent<SliderDrag>, window, cx| {
            if let Some(fraction) = slider_fraction(event, drag_id.clone(), cx) {
                let next = (min + fraction * (max - min)).round().clamp(min, max);
                drag_move_on_change(next, window, cx);
            }
        })
        .on_mouse_down(
            MouseButton::Left,
            move |event: &MouseDownEvent, window, cx| {
                if let Some(bounds) = bounds_cell.get() {
                    if bounds.size.width > px(0.0) {
                        let fraction = ((event.position.x - bounds.left()) / bounds.size.width)
                            .clamp(0.0, 1.0);
                        let next = (min + fraction * (max - min)).round().clamp(min, max);
                        mouse_down_on_change(next, window, cx);
                    }
                }
            },
        );

    div()
        .w(px(180.0))
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .text_size(sp(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text)
                .child(font_size_label(value)),
        )
        .child(
            div()
                .id(slider_id)
                .w_full()
                .tab_index(0)
                .cursor_pointer()
                .rounded(px(4.0))
                .focus_visible(|style| style.bg(theme.overlay))
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let next = match event.keystroke.key.as_str() {
                        "left" | "down" => Some((value - 1.0).max(min)),
                        "right" | "up" => Some((value + 1.0).min(max)),
                        "home" => Some(min),
                        "end" => Some(max),
                        _ => None,
                    };
                    if let Some(next) = next {
                        slider_on_change(next, window, cx);
                        cx.stop_propagation();
                    }
                })
                .child(slider_control),
        )
        .into_any_element()
}

fn background_adjuster(
    id_prefix: &'static str,
    label: String,
    value: f32,
    min: f32,
    max: f32,
    theme: Theme,
    bezel_theme: &bezel::theme::Theme,
    on_change: impl Fn(f32, &mut App) + 'static,
) -> AnyElement {
    let value = value.clamp(min, max);
    let value_label = format!("{value:.0}%");
    let fraction = if max > min {
        (value - min) / (max - min)
    } else {
        0.0
    };
    let on_change: Rc<dyn Fn(f32, &mut App)> = Rc::new(on_change);
    let slider_id = format!("{id_prefix}-slider");
    let control_id = format!("{id_prefix}-control");
    let slider_on_change = on_change.clone();
    let bounds_cell = Rc::new(Cell::new(None));

    let bounds_ref = bounds_cell.clone();
    let bounds_recorder = canvas(
        move |bounds: Bounds<Pixels>, _, _| bounds_ref.set(Some(bounds)),
        |_, _, _, _| (),
    )
    .absolute()
    .inset_0();

    let drag_id: ElementId = control_id.clone().into();
    let drag_move_on_change = on_change.clone();
    let mouse_down_on_change = on_change.clone();

    let slider_control = bezel_theme
        .slider(fraction)
        .child(bounds_recorder)
        .id(control_id)
        .on_drag(SliderDrag(drag_id.clone()), |_, _, _, cx| {
            cx.new(|_| gpui::Empty)
        })
        .on_drag_move(move |event: &DragMoveEvent<SliderDrag>, _, cx| {
            if let Some(fraction) = slider_fraction(event, drag_id.clone(), cx) {
                let next = (min + fraction * (max - min)).round().clamp(min, max);
                drag_move_on_change(next, cx);
            }
        })
        .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, cx| {
            if let Some(bounds) = bounds_cell.get() {
                if bounds.size.width > px(0.0) {
                    let fraction =
                        ((event.position.x - bounds.left()) / bounds.size.width).clamp(0.0, 1.0);
                    let next = (min + fraction * (max - min)).round().clamp(min, max);
                    mouse_down_on_change(next, cx);
                }
            }
        });

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(sp(12.0))
                        .text_color(theme.text_secondary)
                        .child(label),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(sp(12.0))
                        .text_color(theme.text)
                        .child(value_label),
                ),
        )
        .child(
            div()
                .id(slider_id)
                .w_full()
                .tab_index(0)
                .cursor_pointer()
                .rounded(px(4.0))
                .focus_visible(|style| style.bg(theme.overlay))
                .on_key_down(move |event: &KeyDownEvent, _, cx| {
                    let next = match event.keystroke.key.as_str() {
                        "left" => Some((value - 1.0).max(min)),
                        "right" => Some((value + 1.0).min(max)),
                        "home" => Some(min),
                        "end" => Some(max),
                        _ => None,
                    };
                    if let Some(next) = next {
                        slider_on_change(next, cx);
                        cx.stop_propagation();
                    }
                })
                .child(slider_control),
        )
        .into_any_element()
}

fn render_theme_section(
    theme: Theme,
    selected_theme: ThemePreference,
    weak: gpui::WeakEntity<Padu>,
    cx: &mut Context<Padu>,
) -> AnyElement {
    let bezel_theme = bezel::theme::Theme::of(cx);
    let selected_ix = match selected_theme {
        ThemePreference::System => 0,
        ThemePreference::Light => 1,
        ThemePreference::Dark => 2,
    };
    let tiles = ThemePreference::ALL.into_iter().map(|pref| {
        let preview_path = match pref {
            ThemePreference::System => "themes/system.svg",
            ThemePreference::Light => "themes/light.svg",
            ThemePreference::Dark => "themes/dark.svg",
        };
        let preview = div()
            .w(px(130.0))
            .aspect_ratio(188.0 / 142.0)
            .rounded(px(6.0))
            .overflow_hidden()
            .border_1()
            .border_color(theme.border)
            .bg(theme.surface)
            .flex()
            .items_center()
            .justify_center()
            .child(
                img(preview_path)
                    .size_full()
                    .rounded(px(6.0))
                    .object_fit(ObjectFit::Contain),
            )
            .into_any_element();
        (preview, SharedString::from(pref.label()))
    });

    let weak_picker = weak.clone();
    let theme_selector = bezel_theme.picture_picker(
        "theme-preference-picker",
        tiles,
        selected_ix,
        move |ix, window, cx| {
            let pref = match ix {
                0 => ThemePreference::System,
                1 => ThemePreference::Light,
                _ => ThemePreference::Dark,
            };
            let _ = weak_picker.update(cx, |this, cx| {
                this.set_theme_preference(pref, window, cx);
            });
        },
    );

    div()
        .w_full()
        .min_h(px(60.0))
        .px(px(20.0))
        .py(px(16.0))
        .flex()
        .flex_col()
        .items_start()
        .gap(px(12.0))
        .child(
            div()
                .w_full()
                .child(
                    div()
                        .text_size(sp(13.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(tr!("settings.theme")),
                )
                .child(
                    div()
                        .mt(px(5.0))
                        .text_size(sp(12.5))
                        .line_height(sp(18.0))
                        .text_color(theme.text_secondary)
                        .child(tr!("settings.theme_description")),
                ),
        )
        .child(theme_selector)
        .into_any_element()
}

/// One label/description row of a settings card, with its control on the end.
fn code_setting_row(title: String, description: String, control: AnyElement, theme: Theme) -> Div {
    div()
        .w_full()
        .min_h(px(60.0))
        .px(px(20.0))
        .py(px(12.0))
        .flex()
        .items_center()
        .gap(px(24.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(sp(13.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(title),
                )
                .child(
                    div()
                        .mt(px(5.0))
                        .text_size(sp(12.5))
                        .line_height(sp(18.0))
                        .text_color(theme.text_secondary)
                        .child(description),
                ),
        )
        .child(control)
}

/// Code surfaces: text size and soft wrapping, shared by the file editor,
/// diffs, and code blocks.
fn render_code_section(padu: &Padu, theme: Theme, cx: &mut Context<Padu>) -> AnyElement {
    let bezel_theme = bezel::theme::Theme::of(cx);
    let selected_code_font_size = padu.state.code_font_size;
    let weak_code_font = cx.entity().downgrade();
    let code_font_size_slider = font_size_slider(
        "code-font-size",
        selected_code_font_size,
        11.0,
        20.0,
        theme,
        bezel_theme,
        move |size, _, cx| {
            let _ = weak_code_font.update(cx, |this, cx| {
                this.set_code_font_size(size, cx);
            });
        },
    );

    let wrap = padu.state.code_word_wrap;
    let weak = cx.entity().downgrade();
    let wrap_toggle = Switch::new("code-word-wrap-toggle", wrap).on_toggle(move |wrap, _, cx| {
        let _ = weak.update(cx, |this, cx| this.set_code_word_wrap(wrap, cx));
    });

    div()
        .mt(px(15.0))
        .w_full()
        .rounded(px(13.0))
        .overflow_hidden()
        .bg(theme.raised)
        .child(
            div()
                .w_full()
                .px(px(20.0))
                .py(px(14.0))
                .child(
                    div()
                        .text_size(sp(13.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(tr!("settings.code")),
                )
                .child(
                    div()
                        .mt(px(5.0))
                        .text_size(sp(12.5))
                        .line_height(sp(18.0))
                        .text_color(theme.text_secondary)
                        .child(tr!("settings.code_description")),
                ),
        )
        .child(div().mx(px(20.0)).h(px(1.0)).bg(theme.border))
        .child(code_setting_row(
            tr!("settings.code_font_size"),
            tr!("settings.code_font_size_description"),
            code_font_size_slider,
            theme,
        ))
        .child(div().mx(px(20.0)).h(px(1.0)).bg(theme.border))
        .child(code_setting_row(
            tr!("settings.code_word_wrap"),
            tr!("settings.code_word_wrap_description"),
            wrap_toggle.into_any_element(),
            theme,
        ))
        .into_any_element()
}

fn render_sidebar_section(padu: &Padu, theme: Theme, cx: &mut Context<Padu>) -> AnyElement {
    let show_provider = padu.state.sidebar_show_provider;
    let weak = cx.entity().downgrade();
    let toggle =
        Switch::new("sidebar-show-provider-toggle", show_provider).on_toggle(move |show, _, cx| {
            let _ = weak.update(cx, |this, cx| this.set_sidebar_show_provider(show, cx));
        });

    div()
        .mt(px(15.0))
        .w_full()
        .rounded(px(13.0))
        .overflow_hidden()
        .bg(theme.raised)
        .child(
            div()
                .w_full()
                .px(px(20.0))
                .py(px(14.0))
                .child(
                    div()
                        .text_size(sp(13.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(tr!("settings.sidebar")),
                )
                .child(
                    div()
                        .mt(px(5.0))
                        .text_size(sp(12.5))
                        .line_height(sp(18.0))
                        .text_color(theme.text_secondary)
                        .child(tr!("settings.sidebar_description")),
                ),
        )
        .child(div().mx(px(20.0)).h(px(1.0)).bg(theme.border))
        .child(
            div()
                .w_full()
                .min_h(px(60.0))
                .px(px(20.0))
                .py(px(12.0))
                .flex()
                .items_center()
                .gap(px(24.0))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(sp(13.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(tr!("settings.sidebar_show_provider")),
                        )
                        .child(
                            div()
                                .mt(px(5.0))
                                .text_size(sp(12.5))
                                .line_height(sp(18.0))
                                .text_color(theme.text_secondary)
                                .child(tr!("settings.sidebar_show_provider_description")),
                        ),
                )
                .child(toggle),
        )
        .into_any_element()
}

impl Padu {
    pub(super) fn render_appearance_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let bezel_theme = bezel::theme::Theme::of(cx);
        let selected_theme = self.state.theme;
        let weak = cx.entity().downgrade();
        let theme_weak = weak.clone();

        let selected_ui_font_size = self.state.ui_font_size;
        let weak_ui_font = cx.entity().downgrade();
        let ui_font_size_slider = font_size_slider(
            "ui-font-size",
            selected_ui_font_size,
            11.0,
            20.0,
            theme,
            bezel_theme,
            move |size, window, cx| {
                let _ = weak_ui_font.update(cx, |this, cx| {
                    this.set_ui_font_size(size, window, cx);
                });
            },
        );

        let appearance_card = div()
            .mt(px(15.0))
            .w_full()
            .flex()
            .flex_col()
            .rounded(px(13.0))
            .overflow_hidden()
            .bg(theme.raised)
            .child(render_theme_section(theme, selected_theme, theme_weak, cx))
            .child(div().mx(px(20.0)).h(px(1.0)).bg(theme.border))
            .child(
                div()
                    .w_full()
                    .min_h(px(60.0))
                    .px(px(20.0))
                    .py(px(12.0))
                    .flex()
                    .items_center()
                    .gap(px(24.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(sp(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(tr!("language.title")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("language.description")),
                            ),
                    )
                    .child(
                        div()
                            .w(px(180.0))
                            .flex_none()
                            .child(self.language_combobox.clone()),
                    ),
            )
            .child(div().mx(px(20.0)).h(px(1.0)).bg(theme.border))
            .child(
                div()
                    .w_full()
                    .min_h(px(60.0))
                    .px(px(20.0))
                    .py(px(12.0))
                    .flex()
                    .items_center()
                    .gap(px(24.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(sp(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(tr!("settings.ui_font_size")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("settings.ui_font_size_description")),
                            ),
                    )
                    .child(ui_font_size_slider),
            )
            .into_any_element();

        div()
            .w_full()
            .flex()
            .flex_col()
            .child(appearance_card)
            .child(render_code_section(self, theme, cx))
            .child(render_sidebar_section(self, theme, cx))
            .child(render_background_section(self, theme, cx))
            .into_any_element()
    }

    fn set_sidebar_show_provider(&mut self, show: bool, cx: &mut Context<Self>) {
        if self.state.sidebar_show_provider == show {
            return;
        }
        self.state.sidebar_show_provider = show;
        self.save();
        cx.notify();
    }

    fn set_ui_font_size(&mut self, size: f32, window: &mut Window, cx: &mut Context<Self>) {
        let size = padu_client::persistence::sanitized_ui_font_size(size);
        if self.state.ui_font_size == size {
            return;
        }
        self.state.ui_font_size = size;
        // Chrome is authored in `sp` rems; the rem size is the setting.
        window.set_rem_size(px(size));
        self.remeasure_font_sized_surfaces();
        self.save();
        window.refresh();
        cx.notify();
    }

    fn set_code_font_size(&mut self, size: f32, cx: &mut Context<Self>) {
        let size = padu_client::persistence::sanitized_code_font_size(size);
        if self.state.code_font_size == size {
            return;
        }
        self.state.code_font_size = size;
        self.remeasure_font_sized_surfaces();
        self.save();
        cx.notify();
    }

    fn set_code_word_wrap(&mut self, wrap: bool, cx: &mut Context<Self>) {
        if self.state.code_word_wrap == wrap {
            return;
        }
        self.state.code_word_wrap = wrap;
        // Open editors pick the setting up on their next render — see
        // `render_file_editor_body` — so nothing has to be rebuilt here and no
        // caret, selection, or scroll position is lost.
        //
        // Wrap changes how tall every code row is, exactly like a font size
        // change does, so the same height caches have to be dropped.
        self.remeasure_font_sized_surfaces();
        self.save();
        cx.notify();
    }

    /// Drop every cached row height that a code setting — font size or word
    /// wrap — participates in. The virtualized lists remember measured
    /// heights, so a stale entry would misplace scroll anchors until the row
    /// happened to remeasure. The sidebar list keeps its uniform row height
    /// and needs no reset.
    fn remeasure_font_sized_surfaces(&self) {
        self.reset_transcript_rows(self.transcript_row_count());
        let line_count = self
            .right_panel_diff_snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.lines.len());
        self.right_panel_diff_list_state.reset(line_count);
        self.right_panel_diff_tree_list_state
            .reset(self.right_panel_diff_tree_rows.borrow().len());
        self.skills_list_state
            .reset(self.skills_rows.borrow().len());
    }

    fn set_theme_preference(
        &mut self,
        preference: ThemePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.theme == preference {
            return;
        }
        self.state.theme = preference;
        crate::theme::apply_theme_preference(preference, window, cx);
        self.save();
        cx.notify();
    }

    pub(crate) fn create_language_combobox(
        language: padu_protocol::i18n::AppLanguage,
        cx: &mut Context<Self>,
    ) -> Entity<Combobox> {
        let items = padu_protocol::i18n::AppLanguage::ALL
            .into_iter()
            .map(|lang| SharedString::from(lang.label()))
            .collect::<Vec<_>>();
        let selected_index = padu_protocol::i18n::AppLanguage::ALL
            .into_iter()
            .position(|lang| lang == language);
        let combobox = cx.new(|cx| {
            let mut combo = Combobox::new(items, tr!("language.title"), cx);
            if let Some(index) = selected_index {
                combo = combo.with_selection(index);
            }
            combo
        });

        cx.subscribe(
            &combobox,
            |this: &mut Self, _, event: &ComboboxEvent, cx| match event {
                ComboboxEvent::Selected(index) => {
                    if let Some(&language) = padu_protocol::i18n::AppLanguage::ALL.get(*index) {
                        this.set_language(language, None, cx);
                    }
                }
            },
        )
        .detach();

        combobox
    }

    pub(crate) fn set_language(
        &mut self,
        language: crate::i18n::AppLanguage,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if self.state.language == language {
            return;
        }

        self.state.language = language;
        crate::i18n::set_language(language);
        self.language_combobox = Self::create_language_combobox(language, cx);

        self.composer.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.do_anything"), cx)
        });
        self.model_search.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.search_models"), cx)
        });
        self.branch_search.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.search_branches"), cx)
        });
        self.branch_create_input.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.new_branch_name"), cx)
        });
        self.settings_search.update(cx, |input, cx| {
            input.set_placeholder(tr!("settings.search"), cx)
        });
        self.keybindings_search.update(cx, |input, cx| {
            input.set_placeholder(tr!("keybindings.search"), cx)
        });
        self.skills_search.update(cx, |input, cx| {
            input.set_placeholder(tr!("skills.search"), cx)
        });
        self.provider_path_input.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.detected_automatically"), cx)
        });
        self.usage_project_filter.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.filter_projects"), cx)
        });
        self.refresh_command_palette_localized_text(cx);
        self.refresh_file_search_localized_text(cx);
        self.refresh_transcript_search_localized_text(cx);
        for browser in self.right_panel_browsers.values() {
            browser.update(cx, |browser, cx| browser.refresh_localized_text(cx));
        }
        for terminal in self.right_panel_terminals.values() {
            terminal.update(cx, |terminal, cx| terminal.refresh_localized_text(cx));
        }
        for probe in &mut self.probes {
            probe.models = crate::model_catalog::fallback_models(probe.provider);
        }
        self.refresh_provider_detection(None, cx);
        self.invalidate_composer_sources(cx);

        let updater_available = cx
            .try_global::<crate::updater::UpdaterState>()
            .and_then(|updater| updater.0.as_ref())
            .is_some();
        crate::set_app_menus(cx, updater_available);
        self.save();
        if let Some(window) = window {
            window.refresh();
        }
        cx.notify();
    }
}
