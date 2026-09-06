use super::*;

impl Padu {
    pub(super) fn render_computer_use_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let enabled = self.state.computer_use_enabled;
        let permissions = self.computer_permissions.clone();
        let pending = self.computer_permission_request_pending;
        let helper_name = crate::computer_use::helper_display_name();
        let mut allowed_apps = div().flex().flex_col().gap(px(1.0));
        if self.state.computer_use_allowed_apps.is_empty() {
            allowed_apps = allowed_apps.child(
                div()
                    .py(px(12.0))
                    .text_size(sp(12.5))
                    .text_color(theme.text_tertiary)
                    .child(tr!("computer_use.no_always_allowed_apps")),
            );
        } else {
            for (index, grant) in self.state.computer_use_allowed_apps.iter().enumerate() {
                let key = grant.key();
                let is_last = index + 1 == self.state.computer_use_allowed_apps.len();
                let app_icon = self.computer_use_app_icon(&grant.bundle_id, cx);
                allowed_apps = allowed_apps.child(
                    div()
                        .py(px(9.0))
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .when(!is_last, |element| {
                            element.border_b_1().border_color(theme.border)
                        })
                        .child(
                            div()
                                .w(px(32.0))
                                .h(px(32.0))
                                .flex_none()
                                .rounded(px(7.0))
                                .when_some(app_icon, |element, app_icon| {
                                    element.child(img(app_icon).size_full().rounded(px(7.0)))
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_size(sp(12.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text)
                                        .child(SharedString::from(grant.app_name.clone())),
                                )
                                .child(
                                    div()
                                        .mt(px(2.0))
                                        .text_size(sp(12.5))
                                        .text_color(theme.text_tertiary)
                                        .truncate()
                                        .child(SharedString::from(grant.bundle_id.clone())),
                                ),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("revoke-computer-app-{key}")))
                                .h(px(25.0))
                                .px(px(9.0))
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(theme.border_strong)
                                .flex()
                                .items_center()
                                .cursor_pointer()
                                .text_size(sp(12.5))
                                .text_color(theme.text_secondary)
                                .hover(|element| element.bg(theme.overlay).text_color(theme.danger))
                                .child(tr!("common.revoke"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.revoke_computer_app(&key, cx);
                                })),
                        ),
                );
            }
        }

        div()
            .mt(px(15.0))
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .px(px(20.0))
                    .py(px(14.0))
                    .rounded(px(13.0))
                    .bg(theme.raised)
                    .flex()
                    .items_center()
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
                                    .child(tr!("computer_use.allow_apps")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("computer_use.availability")),
                            ),
                    )
                    .child(toggle_switch(
                        "computer-use-enabled",
                        enabled,
                        false,
                        theme,
                        cx,
                        move |this, _, cx| this.set_computer_use_enabled(!enabled, cx),
                    )),
            )
            .child(
                div()
                    .px(px(20.0))
                    .py(px(14.0))
                    .rounded(px(13.0))
                    .bg(theme.raised)
                    .child(
                        div()
                            .text_size(sp(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(tr!("computer_use.macos_access")),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(sp(12.5))
                            .text_color(theme.text_secondary)
                            .child(SharedString::from(tr!(
                                "computer_use.helper_access",
                                helper = helper_name
                            ))),
                    )
                    .child(permission_status_row(
                        tr!("computer_use.screen_recording"),
                        tr!("computer_use.screen_recording_description"),
                        permissions.screen_recording,
                        "screen-recording-settings",
                        theme,
                        cx,
                    ))
                    .child(permission_status_row(
                        tr!("computer_use.accessibility"),
                        tr!("computer_use.accessibility_description"),
                        permissions.accessibility,
                        "accessibility-settings",
                        theme,
                        cx,
                    ))
                    .child(
                        div().mt(px(11.0)).flex().items_center().gap(px(8.0)).child(
                            div()
                                .id("recheck-computer-permissions")
                                .h(px(28.0))
                                .px(px(11.0))
                                .rounded(px(7.0))
                                .border_1()
                                .border_color(theme.border_strong)
                                .text_color(theme.text_secondary)
                                .flex()
                                .items_center()
                                .cursor_pointer()
                                .text_size(sp(12.5))
                                .opacity(if pending { 0.6 } else { 1.0 })
                                .child(if pending {
                                    tr!("common.checking")
                                } else {
                                    tr!("common.recheck")
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.request_computer_permissions(false, cx);
                                })),
                        ),
                    ),
            )
            .child(
                div()
                    .px(px(20.0))
                    .py(px(14.0))
                    .rounded(px(13.0))
                    .bg(theme.raised)
                    .child(
                        div()
                            .text_size(sp(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(tr!("computer_use.always_allowed_apps")),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(sp(12.5))
                            .text_color(theme.text_secondary)
                            .child(tr!("computer_use.always_allowed_apps_description")),
                    )
                    .child(allowed_apps),
            )
            .into_any_element()
    }

    fn set_computer_use_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.state.computer_use_enabled = enabled;
        self.save();
        if enabled {
            self.request_computer_permissions(true, cx);
        }
        cx.notify();
    }

    pub(crate) fn request_computer_permissions(&mut self, prompt: bool, cx: &mut Context<Self>) {
        if self.computer_permission_request_pending {
            return;
        }
        self.computer_permission_request_pending = true;
        let tx = self.computer_permission_tx.clone();
        let event_wake = self.event_wake_tx.clone();
        let daemon = self.daemon.client();
        std::thread::Builder::new()
            .name("padu-computer-permission-request".into())
            .spawn(move || {
                let result = match daemon.request(
                    Uuid::nil(),
                    Uuid::nil(),
                    padu_client::Command::ProbeComputerPermissions { prompt },
                ) {
                    Ok(padu_client::ResponsePayload::ComputerPermissions { permissions }) => {
                        Ok(permissions)
                    }
                    Ok(_) => Err("the daemon returned an invalid permission response".into()),
                    Err(error) => Err(error.to_string()),
                };
                if tx.send(result).is_ok() {
                    signal_event_pump(&event_wake);
                }
            })
            .ok();
        cx.notify();
    }

    fn revoke_computer_app(&mut self, key: &str, cx: &mut Context<Self>) {
        self.state
            .computer_use_allowed_apps
            .retain(|grant| grant.key() != key);
        self.save();
        cx.notify();
    }

    fn computer_use_app_icon(
        &self,
        bundle_id: &str,
        cx: &mut Context<Self>,
    ) -> Option<std::sync::Arc<gpui::Image>> {
        if let Some(icon) = self.computer_use_app_icons.borrow().get(bundle_id) {
            return icon.clone();
        }

        let bundle_id = bundle_id.to_owned();
        if self
            .computer_use_app_icon_loads
            .borrow_mut()
            .insert(bundle_id.clone())
        {
            cx.spawn(async move |this, cx| {
                let load_bundle_id = bundle_id.clone();
                let icon =
                    cx.background_executor()
                        .spawn(async move {
                            crate::platform::load_app_icon_for_bundle_id(&load_bundle_id)
                        })
                        .await;
                let _ = this.update(cx, |this, cx| {
                    this.computer_use_app_icon_loads
                        .borrow_mut()
                        .remove(&bundle_id);
                    this.computer_use_app_icons
                        .borrow_mut()
                        .insert(bundle_id, icon);
                    cx.notify();
                });
            })
            .detach();
        }
        None
    }
}

fn permission_status_row(
    name: String,
    description: String,
    granted: bool,
    id: &'static str,
    theme: Theme,
    cx: &mut Context<Padu>,
) -> Div {
    let status = if granted {
        div()
            .id(id)
            .h(px(25.0))
            .px(px(4.0))
            .rounded(px(6.0))
            .flex()
            .items_center()
            .gap(px(5.0))
            .cursor_default()
            .text_size(sp(12.5))
            .text_color(theme.success)
            .child(icon("icons/check.svg", 12.0, theme.success))
            .child(tr!("computer_use.access_granted"))
    } else {
        div()
            .id(id)
            .h(px(25.0))
            .px(px(9.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .hover(|element| element.bg(theme.overlay).text_color(theme.text))
            .child(tr!("computer_use.grant_access"))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.request_computer_permissions(true, cx);
            }))
    };

    div()
        .mt(px(10.0))
        .pt(px(10.0))
        .border_t_1()
        .border_color(theme.border)
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(sp(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(name),
                )
                .child(
                    div()
                        .mt(px(2.0))
                        .text_size(sp(12.5))
                        .text_color(theme.text_tertiary)
                        .child(description),
                ),
        )
        .child(status)
}
