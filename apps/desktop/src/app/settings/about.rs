use super::*;

impl Padu {
    pub(super) fn render_about_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let version = env!("CARGO_PKG_VERSION");
        let status = self.updater_status;
        let is_checking = self.updater_checking;
        let is_updating = status == crate::updater::UpdateStatus::Updating;
        let is_loading = is_checking || is_updating;

        let (host_name, host_address, is_remote) = if self.daemon.is_remote() {
            let active_host = self
                .state
                .active_host_id
                .as_ref()
                .and_then(|id| self.state.hosts.iter().find(|h| &h.id == id));
            let name = active_host
                .map(|h| h.display_name().to_string())
                .unwrap_or_else(|| self.daemon_hostname.clone());
            let address = active_host
                .map(|h| h.address.clone())
                .unwrap_or_else(|| "Remote".to_string());
            (name, address, true)
        } else {
            let name = tr!("about.local_daemon").to_string();
            let address = format!(
                "ws://{}:{}",
                self.daemon_hostname, self.state.daemon_exposure.port
            );
            (name, address, false)
        };

        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            // Unified About + Version + Updates Card
            .child(
                div()
                    .mt(px(15.0))
                    .w_full()
                    .px(px(20.0))
                    .py(px(18.0))
                    .rounded(px(13.0))
                    .bg(theme.raised)
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(16.0))
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .w(px(60.0))
                                    .h(px(60.0))
                                    .rounded(px(15.0))
                                    .bg(gpui::black())
                                    .border_1()
                                    .border_color(theme.border)
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon("icons/logo.svg", 36.0, gpui::white())),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .text_size(sp(16.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme.text)
                                                    .child("Padu"),
                                            )
                                            .child(
                                                div()
                                                    .px(px(6.0))
                                                    .py(px(1.5))
                                                    .rounded(px(5.0))
                                                    .bg(theme.surface)
                                                    .border_1()
                                                    .border_color(theme.border)
                                                    .font_family(".SystemUIFontMonospaced")
                                                    .text_size(sp(11.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.text_secondary)
                                                    .child(format!("v{version}")),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .mt(px(3.0))
                                            .text_size(sp(12.0))
                                            .line_height(sp(17.0))
                                            .text_color(theme.text_secondary)
                                            .child(tr!("about.tagline")),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap(px(4.0))
                            .flex_none()
                            .child(if status == crate::updater::UpdateStatus::Available {
                                div()
                                    .id("install-update-btn")
                                    .tab_index(0)
                                    .cursor_pointer()
                                    .focus_visible(|style| {
                                        style.border_1().border_color(theme.accent)
                                    })
                                    .h(px(30.0))
                                    .px(px(13.0))
                                    .rounded(px(7.0))
                                    .bg(theme.inverse)
                                    .text_size(sp(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.on_inverse)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(6.0))
                                    .child(icon("icons/cloud-upload.svg", 13.0, theme.on_inverse))
                                    .child(tr!("about.install_update"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.start_available_update(cx);
                                    }))
                                    .on_key_down(cx.listener(
                                        |this, event: &KeyDownEvent, _, cx| {
                                            if matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            ) {
                                                this.start_available_update(cx);
                                                cx.stop_propagation();
                                            }
                                        },
                                    ))
                            } else if is_loading {
                                div()
                                    .id("updating-btn")
                                    .cursor_default()
                                    .h(px(30.0))
                                    .px(px(13.0))
                                    .rounded(px(7.0))
                                    .border_1()
                                    .border_color(theme.border_strong)
                                    .text_size(sp(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text_secondary)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(6.0))
                                    .child(motion::spin_slow(icon(
                                        "icons/loader-circle.svg",
                                        13.0,
                                        theme.text_secondary,
                                    )))
                                    .child(tr!("about.checking_for_updates"))
                            } else {
                                div()
                                    .id("check-for-updates-btn")
                                    .tab_index(0)
                                    .cursor_pointer()
                                    .focus_visible(|style| {
                                        style.border_1().border_color(theme.accent)
                                    })
                                    .h(px(30.0))
                                    .px(px(13.0))
                                    .rounded(px(7.0))
                                    .border_1()
                                    .border_color(theme.border_strong)
                                    .hover(|element| element.bg(theme.overlay))
                                    .active(|element| element.bg(theme.overlay_strong))
                                    .text_size(sp(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(6.0))
                                    .child(icon("icons/rotate-cw.svg", 12.0, theme.text_secondary))
                                    .child(tr!("about.check_for_updates"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.check_for_updates(cx);
                                    }))
                                    .on_key_down(cx.listener(
                                        |this, event: &KeyDownEvent, _, cx| {
                                            if matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            ) {
                                                this.check_for_updates(cx);
                                                cx.stop_propagation();
                                            }
                                        },
                                    ))
                            })
                            .child(
                                div()
                                    .text_size(sp(11.5))
                                    .text_color(
                                        if status == crate::updater::UpdateStatus::Available {
                                            theme.accent
                                        } else {
                                            theme.text_tertiary
                                        },
                                    )
                                    .child(if is_loading {
                                        tr!("about.checking_for_updates").to_string()
                                    } else {
                                        match status {
                                            crate::updater::UpdateStatus::Updating => {
                                                tr!("about.checking_for_updates").to_string()
                                            }
                                            crate::updater::UpdateStatus::Available => {
                                                tr!("about.update_available").to_string()
                                            }
                                            crate::updater::UpdateStatus::Idle => {
                                                tr!("about.up_to_date").to_string()
                                            }
                                        }
                                    }),
                            ),
                    ),
            )
            // Connected Host Card
            .child(
                div()
                    .w_full()
                    .min_h(px(60.0))
                    .px(px(20.0))
                    .py(px(14.0))
                    .rounded(px(13.0))
                    .bg(theme.raised)
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .w(px(36.0))
                                    .h(px(36.0))
                                    .rounded(px(8.0))
                                    .bg(theme.surface)
                                    .border_1()
                                    .border_color(theme.border)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon("icons/server.svg", 16.0, theme.accent)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .text_size(sp(13.5))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.text)
                                                    .child(host_name),
                                            )
                                            .child(
                                                div()
                                                    .px(px(6.0))
                                                    .py(px(1.5))
                                                    .rounded(px(4.0))
                                                    .bg(theme.success.opacity(0.15))
                                                    .text_size(sp(11.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.success)
                                                    .child(if is_remote {
                                                        tr!("about.remote_host")
                                                    } else {
                                                        tr!("about.local_daemon")
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .mt(px(3.0))
                                            .font_family(".SystemUIFontMonospaced")
                                            .text_size(sp(12.0))
                                            .text_color(theme.text_secondary)
                                            .child(host_address),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("about-manage-hosts-btn")
                            .tab_index(0)
                            .cursor_pointer()
                            .focus_visible(|style| style.border_1().border_color(theme.accent))
                            .h(px(30.0))
                            .px(px(13.0))
                            .rounded(px(7.0))
                            .border_1()
                            .border_color(theme.border_strong)
                            .hover(|element| element.bg(theme.overlay))
                            .active(|element| element.bg(theme.overlay_strong))
                            .text_size(sp(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(6.0))
                            .child(tr!("about.manage_hosts"))
                            .child(icon("icons/arrow-right.svg", 12.0, theme.text_secondary))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_settings_page(SettingsPage::Daemon, cx);
                            }))
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    this.open_settings_page(SettingsPage::Daemon, cx);
                                    cx.stop_propagation();
                                }
                            })),
                    ),
            )
            // Website, GitHub & Sponsor single row of plain buttons, aligned center
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .pt(px(2.0))
                    .child(
                        div()
                            .id("about-website-btn")
                            .tab_index(0)
                            .cursor_pointer()
                            .focus_visible(|style| style.border_1().border_color(theme.accent))
                            .h(px(30.0))
                            .px(px(12.0))
                            .rounded(px(7.0))
                            .border_1()
                            .border_color(theme.border_strong)
                            .hover(|element| element.bg(theme.overlay).text_color(theme.text))
                            .active(|element| element.bg(theme.overlay_strong))
                            .text_size(sp(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_secondary)
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(icon("icons/globe.svg", 14.0, theme.text_secondary))
                            .child(tr!("about.website"))
                            .child(icon("icons/arrow-up-right.svg", 11.0, theme.text_tertiary))
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.open_url("https://padu.dev");
                            }))
                            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    cx.open_url("https://padu.dev");
                                    cx.stop_propagation();
                                }
                            })),
                    )
                    .child(
                        div()
                            .id("about-github-btn")
                            .tab_index(0)
                            .cursor_pointer()
                            .focus_visible(|style| style.border_1().border_color(theme.accent))
                            .h(px(30.0))
                            .px(px(12.0))
                            .rounded(px(7.0))
                            .border_1()
                            .border_color(theme.border_strong)
                            .hover(|element| element.bg(theme.overlay).text_color(theme.text))
                            .active(|element| element.bg(theme.overlay_strong))
                            .text_size(sp(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_secondary)
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(icon("icons/github.svg", 14.0, theme.text_secondary))
                            .child("GitHub")
                            .child(icon("icons/arrow-up-right.svg", 11.0, theme.text_tertiary))
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.open_url("https://github.com/wisnuwiry/padu");
                            }))
                            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    cx.open_url("https://github.com/wisnuwiry/padu");
                                    cx.stop_propagation();
                                }
                            })),
                    )
                    .child(
                        div()
                            .id("about-sponsor-btn")
                            .tab_index(0)
                            .cursor_pointer()
                            .focus_visible(|style| style.border_1().border_color(theme.accent))
                            .h(px(30.0))
                            .px(px(12.0))
                            .rounded(px(7.0))
                            .border_1()
                            .border_color(theme.border_strong)
                            .hover(|element| element.bg(theme.overlay).text_color(theme.text))
                            .active(|element| element.bg(theme.overlay_strong))
                            .text_size(sp(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_secondary)
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(icon("icons/heart.svg", 14.0, theme.danger))
                            .child("Sponsor")
                            .child(icon("icons/arrow-up-right.svg", 11.0, theme.text_tertiary))
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.open_url("https://github.com/sponsors/wisnuwiry");
                            }))
                            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    cx.open_url("https://github.com/sponsors/wisnuwiry");
                                    cx.stop_propagation();
                                }
                            })),
                    ),
            )
            .into_any_element()
    }
}
