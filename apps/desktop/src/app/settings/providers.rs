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
                this.refresh_provider_detection(None, cx);
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
        if self.agy_is_busy() {
            return;
        }

        self.invalidate_agy_auth_status();
        self.agy_action = AgyActionState::Installing;
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
                let cancelled = this.agy_is_cancelling_install();
                this.agy_action = AgyActionState::Idle;
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
                        this.refresh_provider_detection(Some(ProviderKind::Agy), cx);
                        this.refresh_agy_auth_status(cx);
                    }
                    Ok(response) => {
                        this.show_toast(format!(
                            "Antigravity ACP installation returned an unexpected response: {response:?}"
                        ));
                    }
                    Err(error) if !cancelled => {
                        this.show_toast(tr!(
                            "providers.agy_install_failed",
                            error = error
                        ));
                    }
                    Err(_) => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn cancel_agy_install(&mut self, cx: &mut Context<Self>) {
        if self.agy_action != AgyActionState::Installing {
            return;
        }
        self.agy_action = AgyActionState::CancellingInstall;
        cx.notify();
        let daemon = self.daemon.client();
        cx.spawn(async move |_this, cx| {
            let _ = cx
                .background_executor()
                .spawn(async move {
                    daemon.request_with_timeout(
                        uuid::Uuid::nil(),
                        uuid::Uuid::nil(),
                        padu_client::Command::CancelAgyAcpInstall,
                        std::time::Duration::from_secs(10),
                    )
                })
                .await;
        })
        .detach();
    }

    pub(crate) fn refresh_agy_auth_status(&mut self, cx: &mut Context<Self>) {
        self.agy_auth_check_generation = self.agy_auth_check_generation.wrapping_add(1);
        let generation = self.agy_auth_check_generation;
        self.agy_auth_checking = true;
        let daemon = self.daemon.client();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let status = cx
                .background_executor()
                .spawn(async move {
                    match daemon.request(
                        uuid::Uuid::nil(),
                        uuid::Uuid::nil(),
                        padu_client::Command::CheckAgyAuth,
                    ) {
                        Ok(padu_client::ResponsePayload::AgyAuthStatus {
                            authenticated,
                            account_label,
                        }) => Some((authenticated, account_label)),
                        _ => None,
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.agy_auth_check_generation == generation {
                    this.agy_auth_checking = false;
                    if let Some((authenticated, account_label)) = status {
                        this.agy_authenticated = authenticated;
                        this.agy_account = account_label;
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn invalidate_agy_auth_status(&mut self) {
        self.agy_auth_check_generation = self.agy_auth_check_generation.wrapping_add(1);
        self.agy_auth_checking = false;
        self.agy_account = None;
    }

    fn authenticate_agy(&mut self, cx: &mut Context<Self>) {
        if self.agy_is_busy() || self.agy_auth_checking {
            return;
        }
        self.invalidate_agy_auth_status();
        self.agy_auth_url = None;
        self.agy_action = AgyActionState::SigningIn;
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
                this.agy_action = AgyActionState::Idle;
                this.agy_auth_url = None;
                match result {
                    Ok(padu_client::ResponsePayload::Ack) => {
                        this.agy_authenticated = true;
                        this.show_success_toast(tr!("providers.agy_signed_in"));
                        // The fresh credential now carries the Google
                        // identity; refresh so the account row appears
                        // without reopening the settings row.
                        this.refresh_agy_auth_status(cx);
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

    pub(crate) fn copy_agy_auth_url(&mut self, cx: &mut Context<Self>) {
        if let Some(url) = &self.agy_auth_url {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(url.clone()));
            self.show_success_toast(tr!("common.copied"));
        }
    }

    pub(crate) fn logout_agy(&mut self, cx: &mut Context<Self>) {
        if self.agy_is_busy() {
            return;
        }
        self.invalidate_agy_auth_status();
        self.agy_action = AgyActionState::SigningOut;
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
                this.agy_action = AgyActionState::Idle;
                if let Ok(padu_client::ResponsePayload::Ack) = result {
                    this.agy_authenticated = false;
                    this.agy_account = None;
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
        if self.agy_is_busy() {
            return;
        }
        self.invalidate_agy_auth_status();
        self.agy_action = AgyActionState::Removing;
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
                this.agy_action = AgyActionState::Idle;
                match result {
                    Ok(padu_client::ResponsePayload::Ack) => {
                        this.state
                            .provider_binary_overrides
                            .remove(&ProviderKind::Agy);
                        this.save();
                        this.refresh_provider_detection(Some(ProviderKind::Agy), cx);
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
                        element.child(self.render_agy_install_control(&theme, cx))
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
                element.child(self.render_agy_expanded_controls(&theme, cx))
            })
            .when_some(self.provider_account_label(kind), |element, label| {
                element.child(
                    div()
                        .text_size(sp(12.5))
                        .text_color(theme.text_secondary)
                        .child(SharedString::from(label)),
                )
            })
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_ghost)
                    .child(SharedString::from(caption)),
            )
    }

    /// Compact "user • plan" account line for the expanded provider row.
    /// Antigravity exposes identity but no plan, so its row shows the Google
    /// account alone. Returns `None` when neither identity nor plan is known,
    /// so the row hides instead of showing a placeholder. Reads only cached
    /// snapshots; refreshing happens off-thread when the row expands.
    fn provider_account_label(&self, kind: ProviderKind) -> Option<String> {
        if kind == ProviderKind::Agy {
            return self.agy_account.clone();
        }
        let usage = self.plan_usage.get(&kind)?;
        match (&usage.account_label, &usage.plan_label) {
            (Some(account), Some(plan)) => {
                Some(tr!("providers.account_plan", user = account, plan = plan))
            }
            (Some(account), None) => Some(account.clone()),
            (None, Some(plan)) => Some(plan.clone()),
            (None, None) => None,
        }
    }

    fn render_settings_action_button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: impl Into<SharedString>,
        leading_icon: Option<AnyElement>,
        trailing_icon: Option<AnyElement>,
        disabled: bool,
        text_color: Hsla,
        theme: &Theme,
        cx: &mut Context<Self>,
        on_action: impl Fn(&mut Padu, &mut Window, &mut Context<Padu>) + 'static + Copy,
    ) -> Stateful<Div> {
        div()
            .id(id.into())
            .tab_index(0)
            .focus_visible(|style| style.border_color(theme.accent))
            .h(px(29.0))
            .px(px(9.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .gap(px(6.0))
            .cursor(if disabled {
                gpui::CursorStyle::Arrow
            } else {
                gpui::CursorStyle::PointingHand
            })
            .opacity(if disabled { 0.65 } else { 1.0 })
            .text_size(sp(12.5))
            .text_color(text_color)
            .when(!disabled, |element| {
                element.hover(|el| el.bg(theme.overlay))
            })
            .children(leading_icon)
            .child(label.into())
            .children(trailing_icon)
            .on_click(cx.listener(move |this, _, window, cx| {
                if !disabled {
                    on_action(this, window, cx);
                }
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !disabled && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    on_action(this, window, cx);
                    cx.stop_propagation();
                }
            }))
    }

    fn render_agy_install_control(&self, theme: &Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        let installing = self.agy_is_installing();
        let cancelling = self.agy_is_cancelling_install();
        let leading_icon = if installing {
            Some(
                crate::ui::motion::spin(icon(
                    "icons/loader-circle.svg",
                    12.0,
                    theme.text_secondary,
                ))
                .into_any_element(),
            )
        } else {
            None
        };
        let trailing_icon = if installing && !cancelling {
            Some(icon("icons/x.svg", 12.0, theme.text_secondary).into_any_element())
        } else {
            None
        };
        let label = if cancelling {
            SharedString::from(tr!("providers.agy_cancelling"))
        } else if installing {
            SharedString::from(tr!(
                "providers.agy_downloading",
                percent = self.agy_install_percent
            ))
        } else {
            SharedString::from(tr!("providers.agy_install_acp"))
        };

        self.render_settings_action_button(
            "install-agy-acp",
            label,
            leading_icon,
            trailing_icon,
            cancelling,
            theme.text_secondary,
            theme,
            cx,
            |this, _, cx| {
                if this.agy_is_installing() {
                    this.cancel_agy_install(cx);
                } else {
                    this.install_agy_acp(cx);
                }
            },
        )
    }

    fn render_agy_expanded_controls(&self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let mut controls = div().mt(px(6.0)).flex().items_center().gap(px(6.0));

        controls = controls.child(self.render_agy_auth_control(theme, cx));

        if self.agy_is_signing_in() {
            controls = controls.child(self.render_agy_copy_link_control(theme, cx));
        }

        controls = controls.child(self.render_agy_reinstall_control(theme, cx));
        controls = controls.child(self.render_agy_remove_control(theme, cx));

        controls
    }

    fn render_agy_auth_control(&self, theme: &Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        let signing_in = self.agy_is_signing_in();
        let signing_out = self.agy_is_signing_out();
        let checking = self.agy_auth_checking && self.agy_action == AgyActionState::Idle;
        let busy = signing_in || signing_out || checking;

        let leading_icon = if busy {
            Some(
                crate::ui::motion::spin(icon(
                    "icons/loader-circle.svg",
                    12.0,
                    theme.text_secondary,
                ))
                .into_any_element(),
            )
        } else {
            None
        };

        let label = if signing_in {
            tr!("providers.agy_signing_in")
        } else if signing_out {
            tr!("providers.agy_signing_out")
        } else if checking {
            tr!("common.checking")
        } else if self.agy_authenticated {
            tr!("providers.agy_sign_out")
        } else {
            tr!("providers.agy_sign_in")
        };

        self.render_settings_action_button(
            "agy-sign-in",
            label,
            leading_icon,
            None,
            busy || self.agy_is_busy(),
            theme.text_secondary,
            theme,
            cx,
            |this, window, cx| {
                if !this.agy_is_busy() && !this.agy_auth_checking {
                    if this.agy_authenticated {
                        this.confirm_sign_out_agy(window, cx);
                    } else {
                        this.authenticate_agy(cx);
                    }
                }
            },
        )
    }

    fn render_agy_copy_link_control(&self, theme: &Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        let has_url = self.agy_auth_url.is_some();
        let button = self.render_settings_action_button(
            "agy-copy-auth-link",
            tr!("providers.agy_copy_link"),
            Some(icon("icons/copy.svg", 12.0, theme.text_secondary).into_any_element()),
            None,
            !has_url,
            theme.text_secondary,
            theme,
            cx,
            |this, _, cx| {
                this.copy_agy_auth_url(cx);
            },
        );

        button.tooltip(Tooltip::text(tr!("providers.agy_copy_link_tooltip")))
    }

    fn render_agy_reinstall_control(&self, theme: &Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        let installing = self.agy_is_installing();
        let cancelling = self.agy_is_cancelling_install();
        let busy = self.agy_is_busy();

        let leading_icon = if installing {
            Some(
                crate::ui::motion::spin(icon(
                    "icons/loader-circle.svg",
                    12.0,
                    theme.text_secondary,
                ))
                .into_any_element(),
            )
        } else {
            None
        };
        let trailing_icon = if installing && !cancelling {
            Some(icon("icons/x.svg", 12.0, theme.text_secondary).into_any_element())
        } else {
            None
        };

        let label = if cancelling {
            SharedString::from(tr!("providers.agy_cancelling"))
        } else if installing {
            SharedString::from(tr!(
                "providers.agy_downloading",
                percent = self.agy_install_percent
            ))
        } else {
            SharedString::from(tr!("providers.agy_reinstall"))
        };

        self.render_settings_action_button(
            "agy-reinstall",
            label,
            leading_icon,
            trailing_icon,
            cancelling || (busy && !installing),
            theme.text_secondary,
            theme,
            cx,
            |this, window, cx| {
                if this.agy_is_installing() {
                    this.cancel_agy_install(cx);
                } else if !this.agy_is_busy() {
                    this.confirm_reinstall_agy(window, cx);
                }
            },
        )
    }

    fn render_agy_remove_control(&self, theme: &Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        let removing = self.agy_is_removing();
        let busy = self.agy_is_busy();

        let leading_icon = if removing {
            Some(
                crate::ui::motion::spin(icon("icons/loader-circle.svg", 12.0, theme.warning))
                    .into_any_element(),
            )
        } else {
            None
        };

        let label = if removing {
            SharedString::from(tr!("providers.agy_removing"))
        } else {
            SharedString::from(tr!("providers.agy_remove_download"))
        };

        self.render_settings_action_button(
            "agy-remove-download",
            label,
            leading_icon,
            None,
            busy,
            theme.warning,
            theme,
            cx,
            |this, window, cx| {
                if !this.agy_is_busy() {
                    this.confirm_remove_agy(window, cx);
                }
            },
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
            if provider == ProviderKind::Agy {
                self.refresh_agy_auth_status(cx);
            }
            // Providers with an account-level plan fetcher refresh their
            // cached snapshot here; the expanded row reads only that cache
            // and hides the account line until it lands.
            if super::super::usage_meter::PLAN_USAGE_PROVIDERS.contains(&provider) {
                self.plan_usage_stale.insert(provider);
                self.maybe_refresh_plan_usage(cx);
            }
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
        self.refresh_provider_detection(Some(provider), cx);
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
