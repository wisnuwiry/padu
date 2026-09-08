use super::*;

impl Padu {
    pub(super) fn render_providers_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let checking = self.provider_detection_remaining > 0;
        let checked_label = self
            .provider_detection_checked_at
            .filter(|_| !checking)
            .map(|checked_at| detection_checked_label(checked_at.elapsed()));

        let refresh = div()
            .id("refresh-providers")
            .tab_index(0)
            .focus_visible(|style| style.border_color(theme.accent))
            .h(px(28.0))
            .px(px(11.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .gap(px(6.0))
            .cursor_pointer()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .opacity(if checking { 0.6 } else { 1.0 })
            .hover(|element| element.bg(theme.overlay))
            .child(icon("icons/rotate-cw.svg", 11.0, theme.text_tertiary))
            .child(if checking {
                tr!("common.checking")
            } else {
                tr!("common.refresh")
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.refresh_provider_detection(None);
                cx.notify();
            }));

        let mut rows = div().mt(px(4.0)).flex().flex_col();
        let provider_count = ProviderKind::ALL.len();
        for (index, kind) in ProviderKind::ALL.into_iter().enumerate() {
            let probe = self.provider_probe(kind);
            let installed = probe.is_some_and(|probe| probe.installed);
            let binary_path = probe
                .filter(|probe| probe.installed)
                .and_then(|probe| probe.path.as_deref())
                .map(|path| abbreviate_home_path(path, self.home_directory.as_deref()));
            let model_count = probe.map(|probe| probe.models.len()).unwrap_or(0);
            let version = self
                .provider_versions
                .get(&kind)
                .and_then(|version| version.clone());
            let disabled = self.state.disabled_providers.contains(&kind);

            let dot_color = if !installed {
                theme.text_ghost
            } else if disabled {
                theme.warning
            } else {
                theme.success
            };

            let detail: AnyElement = if installed {
                let mut parts = Vec::new();
                if let Some(path) = binary_path {
                    parts.push(path);
                }
                if disabled {
                    parts.push(tr!("providers.disabled_for_new_tasks"));
                } else if model_count > 0 {
                    parts.push(if model_count == 1 {
                        tr!("providers.model_count_one", count = model_count)
                    } else {
                        tr!("providers.model_count_many", count = model_count)
                    });
                }
                div()
                    .truncate()
                    .child(SharedString::from(parts.join("  ·  ")))
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .items_baseline()
                    .child(SharedString::from(tr!(
                        "providers.not_detected_as",
                        command = kind.command()
                    )))
                    .into_any_element()
            };

            let toggle_on = !disabled;
            let toggle = toggle_switch(
                SharedString::from(format!("provider-enabled-{}", kind.id())),
                toggle_on,
                false,
                theme,
                cx,
                move |this, _, cx| this.set_provider_enabled(kind, disabled, cx),
            );

            let expanded = self.expanded_provider_settings == Some(kind);
            let expand_button = icon_button(
                SharedString::from(format!("provider-expand-{}", kind.id())),
                if expanded {
                    "icons/chevron-down.svg"
                } else {
                    "icons/chevron-right.svg"
                },
                theme,
            )
            .tab_index(0)
            .focus_visible(|style| style.border_1().border_color(theme.accent))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.toggle_provider_expanded(kind, window, cx);
            }));

            let header = div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .relative()
                        .w(px(30.0))
                        .h(px(30.0))
                        .flex_none()
                        .rounded(px(7.0))
                        .bg(theme.overlay)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(
                            provider_icon(kind),
                            16.0,
                            provider_color(&theme, kind).opacity(if installed { 1.0 } else { 0.5 }),
                        ))
                        .child(
                            div()
                                .absolute()
                                .bottom(px(-2.0))
                                .right(px(-2.0))
                                .w(px(10.0))
                                .h(px(10.0))
                                .rounded_full()
                                .border_2()
                                .border_color(theme.raised)
                                .bg(dot_color),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .flex()
                                .items_baseline()
                                .gap(px(7.0))
                                .child(
                                    div()
                                        .text_size(sp(12.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(if installed {
                                            theme.text
                                        } else {
                                            theme.text_secondary
                                        })
                                        .child(kind.display_name()),
                                )
                                .when_some(version, |element, version| {
                                    element.child(
                                        div()
                                            .font_family(crate::md::render::MONO_FAMILY)
                                            .text_size(sp(12.5))
                                            .text_color(theme.text_tertiary)
                                            .child(SharedString::from(format!("v{version}"))),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .mt(px(3.0))
                                .text_size(sp(12.5))
                                .text_color(theme.text_tertiary)
                                .child(detail),
                        ),
                )
                .child(expand_button)
                .when(installed, |element| element.child(toggle));

            rows = rows.child(
                div()
                    .py(px(11.0))
                    .flex()
                    .flex_col()
                    .when(index + 1 != provider_count, |element| {
                        element.border_b_1().border_color(theme.border)
                    })
                    .child(header)
                    .when(expanded, |element| {
                        element.child(self.render_provider_expanded_settings(kind, theme, cx))
                    }),
            );
        }

        div()
            .mt(px(15.0))
            .w_full()
            .px(px(20.0))
            .py(px(14.0))
            .rounded(px(13.0))
            .bg(theme.raised)
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(20.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(sp(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(tr!("providers.coding_agents")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("providers.description")),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap(px(6.0))
                            .child(refresh)
                            .when_some(checked_label, |element, label| {
                                element.child(
                                    div()
                                        .text_size(sp(12.5))
                                        .text_color(theme.text_ghost)
                                        .child(SharedString::from(label)),
                                )
                            }),
                    ),
            )
            .child(rows)
            .into_any_element()
    }

    pub(crate) fn install_agy_acp(&mut self, cx: &mut Context<Self>) {
        if self.agy_installing {
            return;
        }

        self.agy_installing = true;
        self.agy_install_percent = 0;
        cx.notify();

        let daemon = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    daemon.request_with_timeout(
                        uuid::Uuid::nil(),
                        uuid::Uuid::nil(),
                        padu_client::Command::InstallAgyAcp,
                        std::time::Duration::from_secs(10 * 60),
                    )
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.agy_installing = false;
                match result {
                    Ok(padu_client::ResponsePayload::ProviderInstalled { path, .. }) => {
                        let display_path = path.display().to_string();
                        this.state
                            .provider_binary_overrides
                            .insert(ProviderKind::Agy, display_path.clone());
                        this.save();

                        // The initial provider scan may still be in flight. Update the
                        // visible probe immediately instead of leaving the row stale
                        // until that scan happens to finish.
                        let probe = ProviderProbe {
                            provider: ProviderKind::Agy,
                            installed: true,
                            path: Some(path),
                            models: crate::model_catalog::fallback_models(ProviderKind::Agy),
                            agent_presets: crate::model_catalog::fallback_agent_presets(
                                ProviderKind::Agy,
                            ),
                        };
                        if let Some(existing) = this
                            .probes
                            .iter_mut()
                            .find(|existing| existing.provider == ProviderKind::Agy)
                        {
                            *existing = probe;
                        } else {
                            this.probes.push(probe);
                        }
                        this.show_success_toast(tr!(
                            "providers.agy_installed_at",
                            path = display_path
                        ));
                        this.refresh_provider_detection(Some(ProviderKind::Agy));
                    }
                    Ok(response) => {
                        this.show_toast(format!(
                            "Antigravity ACP installation returned an unexpected response: {response:?}"
                        ));
                    }
                    Err(error) => {
                        this.show_toast(tr!(
                            "providers.agy_install_failed",
                            error = error
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn authenticate_agy(&mut self, cx: &mut Context<Self>) {
        if self.agy_installing {
            return;
        }
        self.agy_installing = true;
        cx.notify();
        let daemon = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    daemon.request_with_timeout(
                        uuid::Uuid::nil(),
                        uuid::Uuid::nil(),
                        padu_client::Command::AuthenticateAgy,
                        std::time::Duration::from_secs(10 * 60),
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.agy_installing = false;
                match result {
                    Ok(padu_client::ResponsePayload::Ack) => {
                        this.agy_authenticated = true;
                        this.show_success_toast(tr!("providers.agy_signed_in"));
                    }
                    Ok(response) => this.show_toast(format!(
                        "Antigravity sign-in returned an unexpected response: {response:?}"
                    )),
                    Err(error) => {
                        this.show_toast(tr!("providers.agy_sign_in_failed", error = error))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn logout_agy(&mut self, cx: &mut Context<Self>) {
        if self.agy_installing {
            return;
        }
        self.agy_installing = true;
        cx.notify();
        let daemon = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    daemon.request(
                        uuid::Uuid::nil(),
                        uuid::Uuid::nil(),
                        padu_client::Command::LogoutAgy,
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.agy_installing = false;
                if let Ok(padu_client::ResponsePayload::Ack) = result {
                    this.agy_authenticated = false;
                    this.show_success_toast(tr!("providers.agy_signed_out"));
                } else if let Err(error) = result {
                    this.show_toast(tr!("providers.agy_sign_out_failed", error = error));
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn remove_agy_download(&mut self, cx: &mut Context<Self>) {
        if self.agy_installing {
            return;
        }
        self.agy_installing = true;
        cx.notify();
        let daemon = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    daemon.request(
                        uuid::Uuid::nil(),
                        uuid::Uuid::nil(),
                        padu_client::Command::RemoveAgyAcp,
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.agy_installing = false;
                match result {
                    Ok(padu_client::ResponsePayload::Ack) => {
                        this.state
                            .provider_binary_overrides
                            .remove(&ProviderKind::Agy);
                        this.save();
                        this.refresh_provider_detection(Some(ProviderKind::Agy));
                        this.show_success_toast(tr!("providers.agy_removed"));
                    }
                    Ok(response) => this.show_toast(format!(
                        "Antigravity removal returned an unexpected response: {response:?}"
                    )),
                    Err(error) => {
                        this.show_toast(tr!("providers.agy_remove_failed", error = error))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The expanded row's settings body: the binary override for this
    /// provider, with the detection result as its caption.
    fn render_provider_expanded_settings(
        &self,
        kind: ProviderKind,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let override_value = self.state.provider_binary_overrides.get(&kind).cloned();
        let installed = self
            .provider_probe(kind)
            .is_some_and(|probe| probe.installed);
        let full_path = self
            .provider_probe(kind)
            .filter(|probe| probe.installed)
            .and_then(|probe| probe.path.as_ref())
            .map(|path| path.display().to_string());

        let caption = match (&override_value, full_path) {
            (Some(_), Some(path)) => tr!("providers.using_override", path = path),
            (Some(_), None) => tr!("providers.invalid_override"),
            (None, Some(path)) => tr!("providers.detected_at", path = path),
            (None, None) => tr!("providers.searches_path", command = kind.command()),
        };

        let reset = div()
            .id(SharedString::from(format!(
                "provider-path-reset-{}",
                kind.id()
            )))
            .tab_index(0)
            .focus_visible(|style| style.border_color(theme.accent))
            .h(px(29.0))
            .px(px(10.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .flex_none()
            .items_center()
            .cursor_pointer()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .hover(|element| element.bg(theme.overlay))
            .child(tr!("common.reset"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.provider_path_input
                    .update(cx, |input, cx| input.clear(cx));
                this.apply_provider_path_override(cx);
            }));

        div()
            .mt(px(10.0))
            .pl(px(42.0))
            .flex()
            .flex_col()
            .gap(px(5.0))
            .child(
                div()
                    .text_size(sp(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(tr!("providers.binary_path")),
            )
            .child(
                div()
                    .text_size(sp(12.5))
                    .line_height(sp(15.0))
                    .text_color(theme.text_tertiary)
                    .child(SharedString::from(tr!(
                        "providers.binary_path_description",
                        provider = kind.short_name()
                    ))),
            )
            .child(
                div()
                    .mt(px(3.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .when(kind == ProviderKind::Agy && !installed, |element| {
                        let installing = self.agy_installing;
                        element.child(
                            div()
                                .id("install-agy-acp")
                                .tab_index(0)
                                .focus_visible(|style| style.border_color(theme.accent))
                                .h(px(29.0))
                                .px(px(10.0))
                                .rounded(px(7.0))
                                .border_1()
                                .border_color(theme.border_strong)
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .cursor(if installing {
                                    gpui::CursorStyle::Arrow
                                } else {
                                    gpui::CursorStyle::PointingHand
                                })
                                .opacity(if installing { 0.65 } else { 1.0 })
                                .text_size(sp(12.5))
                                .text_color(theme.text_secondary)
                                .hover(|element| element.bg(theme.overlay))
                                .when(installing, |element| {
                                    element.child(crate::ui::motion::spin(icon(
                                        "icons/loader-circle.svg",
                                        12.0,
                                        theme.text_secondary,
                                    )))
                                })
                                .child(if installing {
                                    SharedString::from(tr!(
                                        "providers.agy_downloading",
                                        percent = self.agy_install_percent
                                    ))
                                } else {
                                    SharedString::from(tr!("providers.agy_install_acp"))
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !installing {
                                        this.install_agy_acp(cx);
                                    }
                                }))
                                .on_key_down(cx.listener(
                                    move |this, event: &KeyDownEvent, _, cx| {
                                        if !installing
                                            && matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            )
                                        {
                                            this.install_agy_acp(cx);
                                            cx.stop_propagation();
                                        }
                                    },
                                )),
                        )
                    })
                    .child(
                        TextField::new(
                            SharedString::from(format!("provider-path-field-{}", kind.id())),
                            self.provider_path_input.clone(),
                        )
                        .flex_1()
                        .max_w(px(430.0)),
                    )
                    .when(override_value.is_some(), |element| element.child(reset)),
            )
            .when(kind == ProviderKind::Agy && installed, |element| {
                let installing = self.agy_installing;
                element.child(
                    div()
                        .mt(px(6.0))
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .id("agy-sign-in")
                                .tab_index(0)
                                .focus_visible(|style| style.border_color(theme.accent))
                                .h(px(29.0))
                                .px(px(9.0))
                                .rounded(px(7.0))
                                .border_1()
                                .border_color(theme.border_strong)
                                .flex()
                                .items_center()
                                .cursor(if installing {
                                    gpui::CursorStyle::Arrow
                                } else {
                                    gpui::CursorStyle::PointingHand
                                })
                                .opacity(if installing { 0.65 } else { 1.0 })
                                .text_size(sp(12.5))
                                .text_color(theme.text_secondary)
                                .hover(|element| element.bg(theme.overlay))
                                .child(if installing {
                                    tr!("providers.agy_working")
                                } else if self.agy_authenticated {
                                    tr!("providers.agy_sign_out")
                                } else {
                                    tr!("providers.agy_sign_in")
                                })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if !this.agy_installing {
                                        if this.agy_authenticated {
                                            this.confirm_sign_out_agy(window, cx);
                                        } else {
                                            this.authenticate_agy(cx);
                                        }
                                    }
                                }))
                                .on_key_down(cx.listener(
                                    move |this, event: &KeyDownEvent, window, cx| {
                                        if !this.agy_installing
                                            && matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            )
                                        {
                                            if this.agy_authenticated {
                                                this.confirm_sign_out_agy(window, cx);
                                            } else {
                                                this.authenticate_agy(cx);
                                            }
                                            cx.stop_propagation();
                                        }
                                    },
                                )),
                        )
                        .child(
                            div()
                                .id("agy-reinstall")
                                .tab_index(0)
                                .focus_visible(|style| style.border_color(theme.accent))
                                .h(px(29.0))
                                .px(px(9.0))
                                .rounded(px(7.0))
                                .border_1()
                                .border_color(theme.border_strong)
                                .flex()
                                .items_center()
                                .cursor(if installing {
                                    gpui::CursorStyle::Arrow
                                } else {
                                    gpui::CursorStyle::PointingHand
                                })
                                .opacity(if installing { 0.65 } else { 1.0 })
                                .text_size(sp(12.5))
                                .text_color(theme.text_secondary)
                                .hover(|element| element.bg(theme.overlay))
                                .child(tr!("providers.agy_reinstall"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if !this.agy_installing {
                                        this.confirm_reinstall_agy(window, cx);
                                    }
                                }))
                                .on_key_down(cx.listener(
                                    move |this, event: &KeyDownEvent, window, cx| {
                                        if !this.agy_installing
                                            && matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            )
                                        {
                                            this.confirm_reinstall_agy(window, cx);
                                            cx.stop_propagation();
                                        }
                                    },
                                )),
                        )
                        .child(
                            div()
                                .id("agy-remove-download")
                                .tab_index(0)
                                .focus_visible(|style| style.border_color(theme.accent))
                                .h(px(29.0))
                                .px(px(9.0))
                                .rounded(px(7.0))
                                .border_1()
                                .border_color(theme.border_strong)
                                .flex()
                                .items_center()
                                .cursor(if installing {
                                    gpui::CursorStyle::Arrow
                                } else {
                                    gpui::CursorStyle::PointingHand
                                })
                                .opacity(if installing { 0.65 } else { 1.0 })
                                .text_size(sp(12.5))
                                .text_color(theme.warning)
                                .hover(|element| element.bg(theme.overlay))
                                .child(tr!("providers.agy_remove_download"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if !this.agy_installing {
                                        this.confirm_remove_agy(window, cx);
                                    }
                                }))
                                .on_key_down(cx.listener(
                                    move |this, event: &KeyDownEvent, window, cx| {
                                        if !this.agy_installing
                                            && matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            )
                                        {
                                            this.confirm_remove_agy(window, cx);
                                            cx.stop_propagation();
                                        }
                                    },
                                )),
                        ),
                )
            })
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_ghost)
                    .child(SharedString::from(caption)),
            )
    }

    fn toggle_provider_expanded(
        &mut self,
        provider: ProviderKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Commit any pending edit for the previously expanded provider before
        // the input is handed to another row.
        self.apply_provider_path_override(cx);
        if self.expanded_provider_settings == Some(provider) {
            self.expanded_provider_settings = None;
        } else {
            self.expanded_provider_settings = Some(provider);
            let override_value = self
                .state
                .provider_binary_overrides
                .get(&provider)
                .cloned()
                .unwrap_or_default();
            self.provider_path_input
                .update(cx, |input, cx| input.set_content(override_value, cx));
            let focus = self.provider_path_input.read(cx).focus();
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    /// Commit the binary override edit for the expanded provider: empty means
    /// detect from PATH. Re-detects that provider and refreshes every catalog
    /// keyed by the executable path.
    pub(crate) fn apply_provider_path_override(&mut self, cx: &mut Context<Self>) {
        let Some(provider) = self.expanded_provider_settings else {
            return;
        };
        let text = self
            .provider_path_input
            .read(cx)
            .content()
            .trim()
            .to_owned();
        let current = self
            .state
            .provider_binary_overrides
            .get(&provider)
            .cloned()
            .unwrap_or_default();
        if text == current {
            return;
        }
        if text.is_empty() {
            self.state.provider_binary_overrides.remove(&provider);
        } else {
            self.state.provider_binary_overrides.insert(provider, text);
        }
        self.save();
        self.refresh_provider_detection(Some(provider));
        self.refresh_composer_sources(cx);
        cx.notify();
    }

    /// Providers switched off here stop offering models to new sessions;
    /// sessions already locked to them keep working.
    fn set_provider_enabled(
        &mut self,
        provider: ProviderKind,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if enabled {
            self.state
                .disabled_providers
                .retain(|kind| *kind != provider);
        } else if !self.state.disabled_providers.contains(&provider) {
            self.state.disabled_providers.push(provider);
        }
        if !enabled
            && let Some(fallback) = ProviderKind::ALL
                .into_iter()
                .find(|kind| self.provider_enabled(*kind))
        {
            // New work must land somewhere usable: move the new-session
            // default and any unstarted drafts off the switched-off provider.
            // The remembered model belongs to the old provider, so it resets
            // with it.
            if self.state.last_provider == provider {
                self.state.last_provider = fallback;
                self.state.last_model = None;
                self.state.last_reasoning_effort = None;
                self.state.last_service_tier = None;
                self.state.last_context_window = None;
            }
            let draft_ids = self
                .state
                .sessions
                .iter()
                .filter(|session| session.provider == provider && !session.has_started())
                .map(|session| session.id)
                .collect::<Vec<_>>();
            for id in draft_ids {
                if let Some(session) = self.state.session_mut(id) {
                    session.provider = fallback;
                    session.model = None;
                    session.reasoning_effort = None;
                    session.service_tier = None;
                    session.context_window = None;
                }
            }
        }
        self.save();
        cx.notify();
    }
}

/// "Checked …" caption for the Providers page. Recomputed whenever the page
/// redraws; precision beyond the minute is noise here.
fn detection_checked_label(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    if seconds < 90 {
        tr!("providers.checked_just_now")
    } else if seconds < 3600 {
        tr!("providers.checked_minutes_ago", count = seconds / 60)
    } else {
        tr!("providers.checked_hours_ago", count = seconds / 3600)
    }
}

/// Keep the full binary path, abbreviating only the user's home directory.
fn abbreviate_home_path(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(relative) if relative.as_os_str().is_empty() => "~".to_owned(),
        Some(relative) => format!("~/{}", relative.display()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::abbreviate_home_path;
    use std::path::Path;

    #[test]
    fn provider_paths_abbreviate_only_the_home_prefix() {
        let home = Path::new("/Users/example");

        assert_eq!(
            abbreviate_home_path(Path::new("/Users/example/.local/bin/amp"), Some(home)),
            "~/.local/bin/amp"
        );
        assert_eq!(
            abbreviate_home_path(Path::new("/opt/homebrew/bin/codex"), Some(home)),
            "/opt/homebrew/bin/codex"
        );
    }
}
