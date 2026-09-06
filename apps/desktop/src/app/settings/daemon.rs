use super::*;

impl Padu {
    fn render_remote_hosts_section(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let hosts = &self.state.hosts;
        let active_host_id = self.state.active_host_id.as_deref();

        div()
            .px(px(20.0))
            .py(px(16.0))
            .rounded(px(13.0))
            .bg(theme.raised)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(sp(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(tr!("host.remote_hosts")),
                            )
                            .child(
                                div()
                                    .mt(px(4.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("host.remote_hosts_description")),
                            ),
                    )
                    .child(
                        div()
                            .id("add-remote-host-btn")
                            .tab_index(0)
                            .h(px(28.0))
                            .px(px(11.0))
                            .rounded(px(7.0))
                            .border_1()
                            .border_color(theme.border_strong)
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .cursor_pointer()
                            .text_size(sp(12.5))
                            .text_color(theme.text)
                            .hover(|e| e.bg(theme.overlay))
                            .focus_visible(|style| style.border_color(theme.accent))
                            .child(icon("icons/plus.svg", 12.0, theme.text_secondary))
                            .child(tr!("host.add_host"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request_host_dialog(None, cx);
                            }))
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                if !event.keystroke.modifiers.modified()
                                    && matches!(event.keystroke.key.as_str(), "enter" | "space")
                                {
                                    this.request_host_dialog(None, cx);
                                    cx.stop_propagation();
                                }
                            })),
                    ),
            )
            .child(if hosts.is_empty() {
                div()
                    .mt(px(14.0))
                    .py(px(24.0))
                    .px(px(16.0))
                    .rounded(px(10.0))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.overlay.opacity(0.4))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(38.0))
                            .h(px(38.0))
                            .rounded_full()
                            .bg(theme.overlay)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon("icons/server.svg", 18.0, theme.text_tertiary)),
                    )
                    .child(
                        div()
                            .text_size(sp(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_secondary)
                            .child(tr!("host.no_remote_hosts")),
                    )
            } else {
                div()
                    .mt(px(14.0))
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .children(hosts.iter().map(|host| {
                        let host_id = host.id.clone();
                        let is_active = active_host_id == Some(&host.id);
                        let edit_id = host_id.clone();
                        let switch_id = host_id.clone();
                        let has_token = host.token.as_ref().is_some_and(|t| !t.trim().is_empty());
                        let last_conn_label = host_last_connected_label(host.last_connected_at);

                        div()
                            .id(SharedString::from(format!("remote-host-{}", host.id)))
                            .px(px(14.0))
                            .py(px(12.0))
                            .rounded(px(10.0))
                            .bg(theme.surface)
                            .border_1()
                            .border_color(if is_active {
                                theme.accent
                            } else {
                                theme.border
                            })
                            .hover(|e| {
                                if !is_active {
                                    e.border_color(theme.border_strong)
                                } else {
                                    e
                                }
                            })
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(12.0))
                                    .min_w_0()
                                    .flex_1()
                                    .child(
                                        div()
                                            .relative()
                                            .w(px(36.0))
                                            .h(px(36.0))
                                            .flex_none()
                                            .rounded(px(8.0))
                                            .bg(theme.overlay)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(icon(
                                                "icons/server.svg",
                                                16.0,
                                                if is_active {
                                                    theme.accent
                                                } else {
                                                    theme.text_secondary
                                                },
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
                                                    .border_color(theme.surface)
                                                    .bg(if is_active {
                                                        theme.success
                                                    } else {
                                                        theme.text_ghost
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
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
                                                            .truncate()
                                                            .child(host.display_name().to_string()),
                                                    )
                                                    .when(is_active, |row| {
                                                        row.child(
                                                            div()
                                                                .px(px(6.0))
                                                                .py(px(1.5))
                                                                .rounded(px(4.0))
                                                                .bg(theme.success.opacity(0.15))
                                                                .text_size(sp(11.0))
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(theme.success)
                                                                .child(tr!("host.active")),
                                                        )
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .font_family(crate::md::render::MONO_FAMILY)
                                                    .text_size(sp(12.0))
                                                    .text_color(theme.text_secondary)
                                                    .truncate()
                                                    .child(host.address.clone()),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(6.0))
                                                    .text_size(sp(11.5))
                                                    .text_color(theme.text_tertiary)
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(3.5))
                                                            .child(icon(
                                                                "icons/lock.svg",
                                                                10.5,
                                                                theme.text_tertiary,
                                                            ))
                                                            .child(if has_token {
                                                                tr!("host.authenticated")
                                                            } else {
                                                                tr!("host.no_auth")
                                                            }),
                                                    )
                                                    .child(SharedString::from("·"))
                                                    .child(last_conn_label),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .when(!is_active, |row| {
                                        let switch_id = switch_id.clone();
                                        row.child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "connect-host-{}",
                                                    switch_id
                                                )))
                                                .tab_index(0)
                                                .h(px(28.0))
                                                .px(px(11.0))
                                                .rounded(px(6.0))
                                                .bg(theme.inverse)
                                                .text_color(theme.on_inverse)
                                                .font_weight(FontWeight::MEDIUM)
                                                .flex()
                                                .items_center()
                                                .cursor_pointer()
                                                .text_size(sp(12.0))
                                                .hover(|e| e.opacity(0.9))
                                                .focus_visible(|style| {
                                                    style.border_color(theme.accent)
                                                })
                                                .child(tr!("host.connect"))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.switch_to_host(
                                                        Some(switch_id.clone()),
                                                        cx,
                                                    );
                                                })),
                                        )
                                    })
                                    .child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "edit-host-{}",
                                                edit_id
                                            )))
                                            .tab_index(0)
                                            .h(px(28.0))
                                            .px(px(10.0))
                                            .rounded(px(6.0))
                                            .border_1()
                                            .border_color(theme.border_strong)
                                            .flex()
                                            .items_center()
                                            .gap(px(4.0))
                                            .cursor_pointer()
                                            .text_size(sp(12.0))
                                            .text_color(theme.text_secondary)
                                            .hover(|e| e.bg(theme.overlay).text_color(theme.text))
                                            .focus_visible(|style| style.border_color(theme.accent))
                                            .child(icon(
                                                "icons/pencil.svg",
                                                11.0,
                                                theme.text_tertiary,
                                            ))
                                            .child(tr!("common.edit"))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.request_host_dialog(Some(edit_id.clone()), cx);
                                            })),
                                    ),
                            )
                    }))
            })
    }

    pub(super) fn render_daemon_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        if self.daemon.is_remote() {
            let active_host = self
                .state
                .active_host_id
                .as_ref()
                .and_then(|id| self.state.hosts.iter().find(|h| &h.id == id));
            let active_host_name = active_host
                .map(|h| h.display_name().to_string())
                .unwrap_or_else(|| self.daemon_hostname.clone());

            let active_banner = div()
                .w_full()
                .px(px(20.0))
                .py(px(16.0))
                .rounded(px(13.0))
                .bg(theme.raised)
                .border_1()
                .border_color(theme.accent.opacity(0.4))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(16.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(14.0))
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .w(px(38.0))
                                .h(px(38.0))
                                .rounded(px(9.0))
                                .bg(theme.accent.opacity(0.12))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon("icons/server.svg", 18.0, theme.accent)),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap(px(3.0))
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
                                                .child(tr!(
                                                    "daemon.remote_active_banner_title",
                                                    name = active_host_name
                                                )),
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
                                                .child(tr!("host.active")),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(sp(12.5))
                                        .line_height(sp(17.0))
                                        .text_color(theme.text_secondary)
                                        .child(tr!("daemon.remote_active_banner_desc")),
                                ),
                        ),
                )
                .child(
                    div()
                        .id("switch-to-local-btn")
                        .tab_index(0)
                        .h(px(30.0))
                        .px(px(12.0))
                        .rounded(px(7.0))
                        .border_1()
                        .border_color(theme.border_strong)
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .cursor_pointer()
                        .text_size(sp(12.5))
                        .text_color(theme.text)
                        .hover(|e| e.bg(theme.overlay))
                        .focus_visible(|style| style.border_color(theme.accent))
                        .child(icon("icons/arrow-left.svg", 12.0, theme.text_secondary))
                        .child(tr!("daemon.switch_to_local"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.switch_to_host(None, cx);
                        }))
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                            if !event.keystroke.modifiers.modified()
                                && matches!(event.keystroke.key.as_str(), "enter" | "space")
                            {
                                this.switch_to_host(None, cx);
                                cx.stop_propagation();
                            }
                        })),
                );

            return div()
                .mt(px(15.0))
                .w_full()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(active_banner)
                .child(self.render_remote_hosts_section(cx))
                .into_any_element();
        }

        let enabled = self.state.daemon_exposure.enabled;
        let pending = self.daemon_reconfigure_pending;
        let fields_dirty = self.daemon_exposure_fields_dirty(cx);
        let port = self.state.daemon_exposure.port;
        let websocket_url = format!("ws://{}:{port}", self.daemon_hostname);
        let token = self.state.daemon_exposure.token.clone();

        let exposure_toggle = toggle_switch(
            "daemon-exposure-toggle",
            enabled,
            pending,
            theme,
            cx,
            move |this, _, cx| this.set_daemon_exposure_enabled(!enabled, cx),
        );

        let apply_disabled = pending || !fields_dirty;
        let apply_button = div()
            .id("apply-daemon-settings")
            .tab_index(0)
            .h(px(29.0))
            .px(px(11.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .opacity(if apply_disabled { 0.55 } else { 1.0 })
            .focus_visible(|style| style.border_color(theme.accent))
            .when(!apply_disabled, |element| {
                element
                    .hover(|element| element.bg(theme.overlay))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.apply_daemon_exposure_fields(cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if !event.keystroke.modifiers.modified()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            this.apply_daemon_exposure_fields(cx);
                            cx.stop_propagation();
                        }
                    }))
            })
            .child(if pending {
                tr!("daemon.restarting")
            } else {
                tr!("daemon.apply")
            });

        let copy_url_feedback_id = "daemon-url";
        let url_copied = self.control_was_copied(copy_url_feedback_id);
        let copy_url = websocket_url.clone();
        let copy_url_button = div()
            .id("copy-daemon-url")
            .tab_index(0)
            .h(px(27.0))
            .px(px(9.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .gap(px(5.0))
            .cursor_pointer()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .focus_visible(|style| style.border_color(theme.accent))
            .hover(|element| element.bg(theme.overlay))
            .child(icon(
                if url_copied {
                    "icons/check.svg"
                } else {
                    "icons/copy.svg"
                },
                11.0,
                theme.text_tertiary,
            ))
            .child(if url_copied {
                tr!("common.copied")
            } else {
                tr!("common.copy")
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(copy_url.clone()));
                this.show_control_copied(copy_url_feedback_id, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.modified()
                    && matches!(event.keystroke.key.as_str(), "enter" | "space")
                {
                    cx.write_to_clipboard(ClipboardItem::new_string(websocket_url.clone()));
                    this.show_control_copied(copy_url_feedback_id, cx);
                    cx.stop_propagation();
                }
            }));

        let copy_token_feedback_id = "daemon-token";
        let token_copied = self.control_was_copied(copy_token_feedback_id);
        let click_token = token.clone();
        let key_token = token.clone();
        let token_revealed = self.daemon_token_revealed;
        let reveal_token_button = div()
            .id("reveal-daemon-token")
            .tab_index(0)
            .size(px(27.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(theme.text_secondary)
            .focus_visible(|style| style.border_color(theme.accent))
            .hover(|element| element.bg(theme.overlay))
            .active(|element| element.bg(theme.overlay_strong))
            .child(icon(
                if token_revealed {
                    "icons/eye-off.svg"
                } else {
                    "icons/eye.svg"
                },
                12.0,
                theme.text_tertiary,
            ))
            .tooltip(Tooltip::text(if token_revealed {
                tr!("daemon.hide_token")
            } else {
                tr!("daemon.reveal_token")
            }))
            .on_click(cx.listener(|this, _, _, cx| {
                this.daemon_token_revealed = !this.daemon_token_revealed;
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.modified()
                    && matches!(event.keystroke.key.as_str(), "enter" | "space")
                {
                    this.daemon_token_revealed = !this.daemon_token_revealed;
                    cx.stop_propagation();
                    cx.notify();
                }
            }));
        let copy_token_button = div()
            .id("copy-daemon-token")
            .tab_index(0)
            .h(px(27.0))
            .px(px(9.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .gap(px(5.0))
            .cursor_pointer()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .focus_visible(|style| style.border_color(theme.accent))
            .hover(|element| element.bg(theme.overlay))
            .child(icon(
                if token_copied {
                    "icons/check.svg"
                } else {
                    "icons/copy.svg"
                },
                11.0,
                theme.text_tertiary,
            ))
            .child(if token_copied {
                tr!("common.copied")
            } else {
                tr!("common.copy")
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(click_token.clone()));
                this.show_control_copied(copy_token_feedback_id, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.modified()
                    && matches!(event.keystroke.key.as_str(), "enter" | "space")
                {
                    cx.write_to_clipboard(ClipboardItem::new_string(key_token.clone()));
                    this.show_control_copied(copy_token_feedback_id, cx);
                    cx.stop_propagation();
                }
            }));

        let regenerate_button = div()
            .id("regenerate-daemon-token")
            .tab_index(0)
            .h(px(27.0))
            .px(px(9.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .opacity(if pending { 0.55 } else { 1.0 })
            .focus_visible(|style| style.border_color(theme.accent))
            .when(!pending, |element| {
                element
                    .hover(|element| element.bg(theme.overlay))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.regenerate_daemon_token(cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if !event.keystroke.modifiers.modified()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            this.regenerate_daemon_token(cx);
                            cx.stop_propagation();
                        }
                    }))
            })
            .child(tr!("daemon.regenerate_token"));

        div()
            .mt(px(15.0))
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(self.render_remote_hosts_section(cx))
            .child(
                div()
                    .min_h(px(66.0))
                    .px(px(20.0))
                    .py(px(13.0))
                    .rounded(px(13.0))
                    .bg(theme.raised)
                    .flex()
                    .items_center()
                    .gap(px(24.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(7.0))
                                    .child(
                                        div()
                                            .text_size(sp(13.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.text)
                                            .child(tr!("daemon.expose_title")),
                                    )
                                    .child(
                                        div()
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .rounded_full()
                                            .text_size(sp(12.5))
                                            .text_color(if enabled {
                                                theme.success
                                            } else {
                                                theme.text_tertiary
                                            })
                                            .bg(theme.overlay)
                                            .child(if pending {
                                                tr!("daemon.status_restarting")
                                            } else if enabled {
                                                tr!("daemon.status_exposed")
                                            } else {
                                                tr!("daemon.status_local")
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .min_w_0()
                                    .whitespace_normal()
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("daemon.expose_description")),
                            ),
                    )
                    .child(exposure_toggle),
            )
            .when(enabled, |column| {
                column.child(
                    div()
                        .px(px(20.0))
                        .py(px(15.0))
                        .rounded(px(13.0))
                        .bg(theme.raised)
                        .child(
                            div()
                                .text_size(sp(13.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(tr!("daemon.connection_title")),
                        )
                        .child(
                            div()
                                .mt(px(4.0))
                                .min_w_0()
                                .whitespace_normal()
                                .text_size(sp(12.5))
                                .line_height(sp(16.0))
                                .text_color(theme.text_secondary)
                                .child(tr!("daemon.connection_description")),
                        )
                        .child(
                            div()
                                .mt(px(14.0))
                                .flex()
                                .items_start()
                                .gap(px(24.0))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_size(sp(12.5))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme.text)
                                                .child(tr!("daemon.port")),
                                        )
                                        .child(
                                            div()
                                                .mt(px(3.0))
                                                .whitespace_normal()
                                                .text_size(sp(12.5))
                                                .line_height(sp(14.0))
                                                .text_color(theme.text_tertiary)
                                                .child(tr!("daemon.port_description")),
                                        ),
                                )
                                .child(
                                    div().flex_1().min_w_0().flex().justify_end().child(
                                        TextField::new(
                                            "daemon-port-field",
                                            self.daemon_port_input.clone(),
                                        )
                                        .w(px(150.0)),
                                    ),
                                ),
                        )
                        .child(
                            div()
                                .mt(px(14.0))
                                .flex()
                                .items_start()
                                .gap(px(24.0))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_size(sp(12.5))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme.text)
                                                .child(tr!("daemon.allowed_origins")),
                                        )
                                        .child(
                                            div()
                                                .mt(px(3.0))
                                                .whitespace_normal()
                                                .text_size(sp(12.5))
                                                .line_height(sp(14.0))
                                                .text_color(theme.text_tertiary)
                                                .child(tr!("daemon.allowed_origins_description")),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .max_w(px(360.0))
                                        .flex()
                                        .flex_col()
                                        .gap(px(8.0))
                                        .children(self.daemon_origin_inputs.iter().enumerate().map(
                                            |(i, input)| {
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(8.0))
                                                    .w_full()
                                                    .child(
                                                        TextField::new(
                                                            ("daemon-origin-field", i),
                                                            input.clone(),
                                                        )
                                                        .w_full(),
                                                    )
                                                    .child(
                                                        icon_button(
                                                            ("delete-daemon-origin", i),
                                                            "icons/trash.svg",
                                                            theme,
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.remove_daemon_origin(i, cx);
                                                            },
                                                        ))
                                                        .tooltip(Tooltip::text(tr!(
                                                            "daemon.remove_origin"
                                                        ))),
                                                    )
                                            },
                                        ))
                                        .child(
                                            div().flex().items_center().justify_start().child(
                                                div()
                                                    .id("add-daemon-origin-btn")
                                                    .tab_index(0)
                                                    .h(px(26.0))
                                                    .px(px(8.0))
                                                    .rounded(px(6.0))
                                                    .border_1()
                                                    .border_color(theme.border_strong)
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(5.0))
                                                    .cursor_pointer()
                                                    .text_size(sp(12.0))
                                                    .text_color(theme.text_secondary)
                                                    .hover(|el| el.bg(theme.overlay))
                                                    .focus_visible(|style| {
                                                        style.border_color(theme.accent)
                                                    })
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        this.add_daemon_origin(window, cx);
                                                    }))
                                                    .on_key_down(cx.listener(
                                                        |this, event: &KeyDownEvent, window, cx| {
                                                            if !event.keystroke.modifiers.modified()
                                                                && matches!(
                                                                    event.keystroke.key.as_str(),
                                                                    "enter" | "space"
                                                                )
                                                            {
                                                                this.add_daemon_origin(window, cx);
                                                                cx.stop_propagation();
                                                            }
                                                        },
                                                    ))
                                                    .child(icon(
                                                        "icons/plus.svg",
                                                        11.0,
                                                        theme.text_secondary,
                                                    ))
                                                    .child(tr!("daemon.add_origin")),
                                            ),
                                        ),
                                ),
                        )
                        .child(div().mt(px(13.0)).flex().justify_end().child(apply_button)),
                )
            })
            .when(enabled, |column| {
                column.child(
                    div()
                        .px(px(20.0))
                        .py(px(15.0))
                        .rounded(px(13.0))
                        .bg(theme.raised)
                        .child(
                            div()
                                .text_size(sp(13.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(tr!("daemon.credentials_title")),
                        )
                        .child(
                            div()
                                .mt(px(4.0))
                                .min_w_0()
                                .whitespace_normal()
                                .text_size(sp(12.5))
                                .line_height(sp(16.0))
                                .text_color(theme.text_secondary)
                                .child(tr!("daemon.credentials_description")),
                        )
                        .child(
                            div()
                                .mt(px(13.0))
                                .py(px(8.0))
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .w(px(80.0))
                                        .flex_none()
                                        .text_size(sp(12.5))
                                        .text_color(theme.text_tertiary)
                                        .child(tr!("daemon.websocket_url")),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .font_family(".SystemUIFontMonospaced")
                                        .text_size(sp(12.5))
                                        .text_color(theme.text)
                                        .child(SharedString::from(format!(
                                            "ws://{}:{port}",
                                            self.daemon_hostname
                                        ))),
                                )
                                .child(copy_url_button),
                        )
                        .child(
                            div()
                                .py(px(8.0))
                                .border_t_1()
                                .border_color(theme.border)
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .w(px(80.0))
                                        .flex_none()
                                        .text_size(sp(12.5))
                                        .text_color(theme.text_tertiary)
                                        .child(tr!("daemon.token")),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .font_family(".SystemUIFontMonospaced")
                                        .text_size(sp(12.5))
                                        .text_color(theme.text)
                                        .child(SharedString::from(if token_revealed {
                                            token.clone()
                                        } else {
                                            "••••••••••••••••••••••••••••••••".to_owned()
                                        })),
                                )
                                .child(reveal_token_button)
                                .child(copy_token_button)
                                .child(regenerate_button),
                        )
                        .child(
                            div()
                                .mt(px(7.0))
                                .px(px(10.0))
                                .py(px(8.0))
                                .rounded(px(8.0))
                                .bg(theme.inset)
                                .w_full()
                                .min_w_0()
                                .flex()
                                .gap(px(8.0))
                                .child(icon("icons/alert.svg", 13.0, theme.warning))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .whitespace_normal()
                                        .text_size(sp(12.5))
                                        .line_height(sp(15.0))
                                        .text_color(theme.text_secondary)
                                        .child(tr!("daemon.security_warning")),
                                ),
                        ),
                )
            })
            .into_any_element()
    }

    pub(crate) fn add_daemon_origin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .select_all_on_focus_click()
                .placeholder(tr!("daemon.allowed_origins_placeholder"))
        });
        cx.subscribe(
            &input,
            |this: &mut Self, _, event: &InputEvent, cx| match event {
                InputEvent::Submit(_) => this.apply_daemon_exposure_fields(cx),
                InputEvent::Edited => cx.notify(),
                _ => {}
            },
        )
        .detach();
        self.daemon_origin_inputs.push(input);
        cx.notify();
    }

    pub(crate) fn remove_daemon_origin(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.daemon_origin_inputs.len() {
            self.daemon_origin_inputs.remove(index);
            cx.notify();
        }
    }

    fn daemon_exposure_from_fields(
        &self,
        cx: &App,
    ) -> Result<padu_client::DaemonExposureSettings, String> {
        let port = self
            .daemon_port_input
            .read(cx)
            .content()
            .trim()
            .parse::<u16>()
            .map_err(|_| tr!("daemon.invalid_port"))?;
        if port == 0 {
            return Err(tr!("daemon.invalid_port"));
        }
        let origins: Vec<String> = self
            .daemon_origin_inputs
            .iter()
            .map(|input| input.read(cx).content().trim().to_owned())
            .filter(|origin| !origin.is_empty())
            .collect();
        let mut settings = self.state.daemon_exposure.clone();
        settings.port = port;
        settings
            .with_allowed_origins(origins)
            .and_then(padu_client::DaemonExposureSettings::validate)
            .map_err(|error| error.to_string())
    }

    fn daemon_exposure_fields_dirty(&self, cx: &App) -> bool {
        self.daemon_exposure_from_fields(cx)
            .map(|settings| {
                settings.port != self.state.daemon_exposure.port
                    || settings.allowed_origins != self.state.daemon_exposure.allowed_origins
            })
            .unwrap_or(true)
    }

    fn set_daemon_exposure_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if !enabled {
            self.daemon_token_revealed = false;
        }
        let settings = if enabled {
            match self.daemon_exposure_from_fields(cx) {
                Ok(mut settings) => {
                    settings.enabled = true;
                    settings
                }
                Err(error) => {
                    self.show_toast(tr!("daemon.invalid_settings", error = error));
                    return;
                }
            }
        } else {
            let mut settings = self.state.daemon_exposure.clone();
            settings.enabled = false;
            settings
        };
        self.apply_daemon_exposure(settings, cx);
    }

    pub(crate) fn apply_daemon_exposure_fields(&mut self, cx: &mut Context<Self>) {
        let settings = match self.daemon_exposure_from_fields(cx) {
            Ok(settings) => settings,
            Err(error) => {
                self.show_toast(tr!("daemon.invalid_settings", error = error));
                return;
            }
        };
        self.apply_daemon_exposure(settings, cx);
    }

    fn regenerate_daemon_token(&mut self, cx: &mut Context<Self>) {
        let mut settings = match self.daemon_exposure_from_fields(cx) {
            Ok(settings) => settings,
            Err(error) => {
                self.show_toast(tr!("daemon.invalid_settings", error = error));
                return;
            }
        };
        settings.token = padu_client::DaemonExposureSettings::new_token();
        self.daemon_token_revealed = false;
        self.apply_daemon_exposure(settings, cx);
    }

    fn apply_daemon_exposure(
        &mut self,
        settings: padu_client::DaemonExposureSettings,
        cx: &mut Context<Self>,
    ) {
        if self.daemon_reconfigure_pending || settings == self.state.daemon_exposure {
            return;
        }
        if self.daemon.is_remote() {
            self.show_toast(tr!("daemon.external_description"));
            return;
        }
        if self
            .state
            .sessions
            .iter()
            .any(|session| !matches!(session.status, SessionStatus::Idle | SessionStatus::Failed))
        {
            self.show_toast(tr!("daemon.stop_active_tasks"));
            return;
        }

        let needs_restart = self.state.daemon_exposure.enabled || settings.enabled;
        if !needs_restart {
            self.state.daemon_exposure = settings;
            self.save();
            cx.notify();
            return;
        }

        self.daemon_reconfigure_pending = true;
        let daemon = self.daemon.clone();
        let applied = settings.clone();
        let restart = cx
            .background_executor()
            .spawn(async move { daemon.reconfigure(settings) });
        cx.spawn(async move |this, cx| {
            let result = restart.await;
            let _ = this.update(cx, |this, cx| {
                this.daemon_reconfigure_pending = false;
                match result {
                    Ok(()) => {
                        this.state.daemon_exposure = applied.clone();
                        this.runtimes.clear();
                        this.daemon_port_input.update(cx, |input, cx| {
                            input.set_content(applied.port.to_string(), cx)
                        });
                        for (i, origin) in applied.allowed_origins.iter().enumerate() {
                            if let Some(input) = this.daemon_origin_inputs.get(i) {
                                input.update(cx, |input, cx| input.set_content(origin.clone(), cx));
                            }
                        }
                        this.daemon_origin_inputs
                            .truncate(applied.allowed_origins.len());
                        this.save();
                        this.show_success_toast(tr!("daemon.settings_applied"));
                    }
                    Err(error) => {
                        this.show_toast(tr!("daemon.restart_failed", error = error.to_string()))
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

fn host_last_connected_label(last_connected: Option<u64>) -> String {
    let Some(timestamp) = last_connected else {
        return tr!("host.never_connected");
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let seconds = now.saturating_sub(timestamp);
    let time_str = if seconds < 90 {
        tr!("providers.checked_just_now")
    } else if seconds < 3600 {
        tr!("providers.checked_minutes_ago", count = seconds / 60)
    } else if seconds < 86400 {
        tr!("providers.checked_hours_ago", count = seconds / 3600)
    } else {
        format!("{}d ago", seconds / 86400)
    };
    tr!("host.last_connected", time = time_str)
}
