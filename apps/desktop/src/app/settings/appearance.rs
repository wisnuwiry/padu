use super::*;
use crate::ui::slider::slider;

/// Sizes offered by the font-size dropdowns. A hand-edited `app.json` may
/// hold values outside this list; they render as-is and simply select
/// nothing here.
const FONT_SIZES: [f32; 8] = [11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 18.0, 20.0];

fn font_size_label(size: f32) -> String {
    if size.fract() == 0.0 {
        format!("{size:.0} px")
    } else {
        format!("{size} px")
    }
}

fn render_background_section(padu: &Padu, theme: Theme, cx: &mut Context<Padu>) -> AnyElement {
    let settings = &padu.state.conversation_background;
    let image = padu.conversation_background_image();
    let weak = cx.entity().downgrade();
    let choose = div()
        .id("conversation-background-choose")
        .tab_index(0)
        .px(px(10.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(theme.border)
        .text_size(sp(12.0))
        .cursor_pointer()
        .focus_visible(|style| style.border_color(theme.accent))
        .hover(|style| style.bg(theme.overlay))
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(icon("icons/folder-open.svg", 13.0, theme.text_secondary))
        .child(tr!("settings.background_choose"))
        .on_click({
            let weak = weak.clone();
            move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.choose_conversation_background(cx));
            }
        })
        .on_key_down({
            let weak = weak.clone();
            move |event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    let _ = weak.update(cx, |this, cx| this.choose_conversation_background(cx));
                }
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
        div()
            .id("conversation-background-remove")
            .tab_index(0)
            .px(px(8.0))
            .py(px(6.0))
            .text_size(sp(12.0))
            .text_color(theme.text_secondary)
            .cursor_pointer()
            .focus_visible(|style| style.border_1().border_color(theme.accent))
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(icon("icons/trash.svg", 13.0, theme.danger))
            .child(tr!("settings.background_remove"))
            .on_click({
                let weak = weak.clone();
                move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| this.clear_conversation_background(cx));
                }
            })
            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    let _ = weak.update(cx, |this, cx| this.clear_conversation_background(cx));
                }
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

fn background_adjuster(
    id_prefix: &'static str,
    label: String,
    value: f32,
    min: f32,
    max: f32,
    theme: Theme,
    on_change: impl Fn(f32, &mut App) + 'static,
) -> AnyElement {
    let value = value.clamp(min, max);
    let value_label = format!("{value:.0}%");
    let on_change: Rc<dyn Fn(f32, &mut App)> = Rc::new(on_change);
    let slider_id = format!("{id_prefix}-slider");
    let slider_on_change = on_change.clone();

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
                .child(
                    slider(format!("{id_prefix}-control"), value)
                        .range(min..=max)
                        .step(1.0)
                        .colors(theme.accent, theme.border, theme.text)
                        .on_change({
                            let on_change = on_change.clone();
                            move |value, cx| on_change(value, cx)
                        }),
                ),
        )
        .into_any_element()
}

fn render_theme_section(
    theme: Theme,
    selected_theme: ThemePreference,
    weak: gpui::WeakEntity<Padu>,
) -> AnyElement {
    let theme_cards = ThemePreference::ALL
        .into_iter()
        .map(|preference| {
            let selected = preference == selected_theme;
            let weak = weak.clone();
            let preference_id = match preference {
                ThemePreference::System => "system",
                ThemePreference::Light => "light",
                ThemePreference::Dark => "dark",
            };
            let preview_path = match preference {
                ThemePreference::System => "themes/system.svg",
                ThemePreference::Light => "themes/light.svg",
                ThemePreference::Dark => "themes/dark.svg",
            };
            let preview = div()
                .w_full()
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
                );

            div()
                .id(SharedString::from(format!("theme-card-{preference_id}")))
                .tab_index(0)
                .flex_1()
                .max_w(px(140.0))
                .min_w(px(0.0))
                .p(px(7.0))
                .rounded(px(10.0))
                .border_1()
                .border_color(if selected { theme.accent } else { theme.border })
                .bg(if selected {
                    theme.accent.opacity(0.08)
                } else {
                    theme.surface
                })
                .cursor_pointer()
                .focus_visible(|style| style.border_1().border_color(theme.accent))
                .hover(|el| el.bg(theme.overlay))
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.set_theme_preference(preference, window, cx);
                    });
                })
                .child(preview)
                .child(
                    div()
                        .mt(px(7.0))
                        .text_size(sp(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(preference.label()),
                )
        })
        .collect::<Vec<_>>();
    let theme_selector = div()
        .flex()
        .w_full()
        .justify_end()
        .gap(px(8.0))
        .children(theme_cards);

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

fn render_sidebar_section(padu: &Padu, theme: Theme, cx: &mut Context<Padu>) -> AnyElement {
    let show_provider = padu.state.sidebar_show_provider;
    let toggle = toggle_switch(
        "sidebar-show-provider-toggle",
        show_provider,
        false,
        theme,
        cx,
        move |this, _, cx| this.set_sidebar_show_provider(!show_provider, cx),
    );

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
        let selected_theme = self.state.theme;
        let selected_language = self.state.language;
        let weak = cx.entity().downgrade();
        let theme_weak = weak.clone();

        let selected_ui_font_size = self.state.ui_font_size;
        let weak = cx.entity().downgrade();
        let ui_font_size_handle = self.menu_handle("ui-font-size-selector", cx);
        let ui_font_size_selector = dropdown_menu(
            MenuChip::new("ui-font-size-selector")
                .label(font_size_label(selected_ui_font_size))
                .outlined()
                .selected(ui_font_size_handle.is_open())
                .w(px(116.0))
                .justify_between(),
            "ui-font-size-selector-menu",
            &ui_font_size_handle,
            MenuAlign::BelowRight,
            move |_| {
                FONT_SIZES
                    .into_iter()
                    .map(|size| {
                        let weak = weak.clone();
                        MenuItem::new(font_size_label(size), move |window, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.set_ui_font_size(size, window, cx);
                            });
                        })
                        .selected(size == selected_ui_font_size)
                    })
                    .collect()
            },
        );

        let selected_code_font_size = self.state.code_font_size;
        let weak = cx.entity().downgrade();
        let code_font_size_handle = self.menu_handle("code-font-size-selector", cx);
        let code_font_size_selector = dropdown_menu(
            MenuChip::new("code-font-size-selector")
                .label(font_size_label(selected_code_font_size))
                .outlined()
                .selected(code_font_size_handle.is_open())
                .w(px(116.0))
                .justify_between(),
            "code-font-size-selector-menu",
            &code_font_size_handle,
            MenuAlign::BelowRight,
            move |_| {
                FONT_SIZES
                    .into_iter()
                    .map(|size| {
                        let weak = weak.clone();
                        MenuItem::new(font_size_label(size), move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.set_code_font_size(size, cx);
                            });
                        })
                        .selected(size == selected_code_font_size)
                    })
                    .collect()
            },
        );

        let weak = cx.entity().downgrade();
        let language_handle = self.menu_handle("language-selector", cx);
        let language_selector = dropdown_menu(
            MenuChip::new("language-selector")
                .label(selected_language.label())
                .outlined()
                .selected(language_handle.is_open())
                .w(px(150.0))
                .justify_between(),
            "language-selector-menu",
            &language_handle,
            MenuAlign::BelowRight,
            move |_| {
                crate::i18n::AppLanguage::ALL
                    .into_iter()
                    .map(|language| {
                        let weak = weak.clone();
                        MenuItem::new(language.label(), move |window, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.set_language(language, window, cx);
                            });
                        })
                        .selected(language == selected_language)
                    })
                    .collect()
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
            .child(render_theme_section(theme, selected_theme, theme_weak))
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
                    .child(language_selector),
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
                    .child(ui_font_size_selector),
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
                                    .child(tr!("settings.code_font_size")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("settings.code_font_size_description")),
                            ),
                    )
                    .child(code_font_size_selector),
            )
            .into_any_element();

        div()
            .w_full()
            .flex()
            .flex_col()
            .child(appearance_card)
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

    /// Drop every cached row height that a font size participates in. The
    /// virtualized lists remember measured heights, so a stale entry would
    /// misplace scroll anchors until the row happened to remeasure. The
    /// sidebar list keeps its uniform row height and needs no reset.
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

    fn set_language(
        &mut self,
        language: crate::i18n::AppLanguage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.language == language {
            return;
        }

        self.state.language = language;
        crate::i18n::set_language(language);

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
        window.refresh();
        cx.notify();
    }
}
