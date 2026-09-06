use super::*;

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

impl Padu {
    pub(super) fn render_appearance_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let selected_theme = self.state.theme;
        let selected_language = self.state.language;
        let weak = cx.entity().downgrade();
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

        div()
            .mt(px(15.0))
            .w_full()
            .flex()
            .flex_col()
            .rounded(px(13.0))
            .overflow_hidden()
            .bg(theme.raised)
            .child(
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
                    .child(theme_selector),
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
            .into_any_element()
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
        self.refresh_provider_detection(None);
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
