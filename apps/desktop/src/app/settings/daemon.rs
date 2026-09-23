use std::sync::Arc;

use gpui::{Image, ImageFormat};
use padu_client::persistence::HostKind;
use padu_client::transport::{
    CloudflareTransport, QrPayload, Transport, TransportContext, TransportStatus, render_svg,
};

use super::*;

/// Edge length, in pixels, of the credentials card's QR code.
const DAEMON_QR_PX: u32 = 180;
/// Registry key for the tunnel that fronts this desktop's own daemon.
const DAEMON_QR_TUNNEL_KEY: &str = "daemon-exposure";
/// How another device can be told to reach this daemon.
const DAEMON_QR_TRANSPORTS: [HostKind; 4] = [
    HostKind::Direct,
    HostKind::Tailscale,
    HostKind::Cloudflare,
    HostKind::SshRelay,
];

/// `tr!` needs a literal key, so the label is resolved through a match.
fn host_transport_label(kind: HostKind) -> String {
    match kind {
        HostKind::Direct => tr!("host.transport_direct"),
        HostKind::Tailscale => tr!("host.transport_tailscale"),
        HostKind::Cloudflare => tr!("host.transport_cloudflare"),
        HostKind::SshRelay => tr!("host.transport_ssh"),
    }
}

fn qr_input_row(label: String, input: Entity<TextInput>, theme: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(sp(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text_secondary)
                .child(label),
        )
        .child(input)
}

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
                                                    .child(last_conn_label)
                                                    .when_some(
                                                        self.host_transport_status(&host.id),
                                                        |row, (label, healthy)| {
                                                            row.child(SharedString::from("·"))
                                                                .child(
                                                                    div()
                                                                        .flex()
                                                                        .items_center()
                                                                        .gap(px(3.5))
                                                                        .child(icon(
                                                                            if healthy {
                                                                                "icons/check.svg"
                                                                            } else {
                                                                                "icons/alert.svg"
                                                                            },
                                                                            10.5,
                                                                            if healthy {
                                                                                theme.success
                                                                            } else {
                                                                                theme.warning
                                                                            },
                                                                        ))
                                                                        .child(SharedString::from(
                                                                            label,
                                                                        )),
                                                                )
                                                        },
                                                    ),
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

    /// Compact tunnel state for the host list row.
    ///
    /// Uses `try_lock` so the render path never blocks on a transport that is
    /// mid-start; a contended lock simply renders no status this frame.
    fn host_transport_status(&self, host_id: &str) -> Option<(String, bool)> {
        use padu_client::transport::TransportStatus;
        let slot = self.host_transports.get(host_id)?;
        let guard = slot.try_lock()?;
        match guard.status() {
            TransportStatus::Ready { .. } => Some((tr!("host.tunnel_connected"), true)),
            TransportStatus::Starting => Some((tr!("host.tunnel_starting"), true)),
            TransportStatus::Failed { .. } => Some((tr!("host.tunnel_failed"), false)),
            _ => None,
        }
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
                        )
                        .child(self.render_daemon_qr_section(&token, &theme, cx))
                        .child(
                            div()
                                .mt(px(10.0))
                                .flex()
                                .justify_end()
                                .child(regenerate_button),
                        ),
                )
            })
            .into_any_element()
    }

    /// The connection code for this daemon — the only place its address and
    /// token are shown.
    ///
    /// This exists to get a *phone* onto the daemon this desktop is exposing:
    /// the code carries the address and token the card already displays, so
    /// nothing has to be typed on a glass keyboard. It is not part of adding a
    /// remote host — a host profile describes a daemon reached *from* here.
    fn render_daemon_qr_section(&self, token: &str, theme: &Theme, cx: &mut Context<Self>) -> Div {
        // The code is always visible: no expand/hide toggle. Header pairs the
        // section title with a notes view-mode style segmented transport
        // picker, then the body splits into code (left) and instructions
        // (right).
        let mut header_tabs = div()
            .flex()
            .items_center()
            .gap(px(1.0))
            .p(px(2.0))
            .rounded(px(7.0))
            .bg(theme.overlay);
        for kind in DAEMON_QR_TRANSPORTS {
            let active = kind == self.daemon_qr_transport;
            header_tabs = header_tabs.child(
                div()
                    .id(SharedString::from(format!("daemon-qr-tab-{kind:?}")))
                    .tab_index(0)
                    .h(px(28.0))
                    .px(px(10.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_size(sp(12.5))
                    .text_color(if active {
                        theme.text
                    } else {
                        theme.text_secondary
                    })
                    .when(active, |el| el.bg(theme.overlay_strong))
                    .hover(|el| el.bg(theme.overlay_strong))
                    .focus_visible(|style| style.border_color(theme.accent))
                    .child(host_transport_label(kind))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.daemon_qr_transport != kind {
                            this.daemon_qr_transport = kind;
                            // Addresses differ per transport, so the previous
                            // one must not linger as a stale value.
                            this.daemon_qr_field_input
                                .update(cx, |input, cx| input.set_content("", cx));
                            this.daemon_qr_cache.replace(None);
                            this.daemon_qr_tunnel_error = None;
                            cx.notify();
                        }
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if !event.keystroke.modifiers.modified()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            this.daemon_qr_transport = kind;
                            this.daemon_qr_field_input
                                .update(cx, |input, cx| input.set_content("", cx));
                            this.daemon_qr_cache.replace(None);
                            this.daemon_qr_tunnel_error = None;
                            cx.notify();
                            cx.stop_propagation();
                        }
                    })),
            );
        }
        // Left column: transport inputs, tunnel action, address, and code.
        // Right column: what the other device needs for this transport.
        let header = div().mt(px(10.0)).child(header_tabs);
        let mut left = div().flex().flex_col().gap(px(10.0)).flex_1().min_w_0();
        let right = qr_instruction_panel(self.daemon_qr_transport, theme);

        match self.daemon_qr_transport {
            HostKind::Direct => {
                left = left.child(qr_input_row(
                    tr!("daemon.qr_direct_address"),
                    self.daemon_qr_field_input.clone(),
                    theme,
                ));
            }
            HostKind::Tailscale => {
                left = left.child(qr_input_row(
                    tr!("daemon.qr_tailscale_name"),
                    self.daemon_qr_field_input.clone(),
                    theme,
                ));
            }
            HostKind::SshRelay => {
                left = left
                    .child(qr_input_row(
                        tr!("daemon.qr_ssh_host"),
                        self.daemon_qr_field_input.clone(),
                        theme,
                    ))
                    .child(qr_input_row(
                        tr!("daemon.qr_ssh_port"),
                        self.daemon_qr_port_input.clone(),
                        theme,
                    ));
            }
            HostKind::Cloudflare => {
                left = left.child(self.render_daemon_qr_tunnel_button(theme, cx));
            }
        }

        let address = match self.daemon_qr_address(cx) {
            Ok(address) => address,
            Err(reason) => {
                left = left.child(hint_text(reason, theme));
                return div().flex().flex_col().gap(px(10.0)).child(header).child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(16.0))
                        .child(left)
                        .child(right),
                );
            }
        };

        let payload = QrPayload {
            kind: self.daemon_qr_transport,
            url: address.clone(),
            token: token.to_owned(),
            name: self.daemon_hostname.clone(),
        };
        let Ok(encoded) = payload.encode() else {
            return div().flex().flex_col().gap(px(10.0)).child(header).child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(16.0))
                    .child(left.child(hint_text(tr!("daemon.qr_unavailable"), theme)))
                    .child(right),
            );
        };
        // Re-encode only when the address or token actually changed, so an
        // unrelated repaint does not rebuild the matrix.
        let mut cache = self.daemon_qr_cache.borrow_mut();
        if cache.as_ref().is_none_or(|(cached, _)| cached != &encoded) {
            let svg = render_svg(&payload, DAEMON_QR_PX).unwrap_or_default();
            *cache = Some((encoded, svg));
        }
        let svg = cache
            .as_ref()
            .map(|(_, svg)| svg.clone())
            .unwrap_or_default();
        drop(cache);

        // The raw `ws://` address stays out of the Direct tab — it is
        // redundant with the QR and the expandable details below, and showing
        // a plain-text URL next to a full-access token invites copy-paste
        // over an untrusted channel. Other transports keep the same treatment:
        // the QR is for scanning, the details disclosure is for typing.
        left = left.child(render_qr_centered(&svg, theme));
        left = left.child(self.render_daemon_connection_details(&address, token, &theme, cx));
        if self.daemon_qr_transport == HostKind::SshRelay {
            // The credential panel does not provision `ssh -R` itself — the
            // QR only *describes* the jump-host address. Without a running
            // relay the code points at nothing, so say so explicitly instead
            // of rendering a scannable-but-dead code silently.
            left = left.child(hint_text(tr!("daemon.qr_ssh_needs_relay"), theme));
        }
        left = left.child(hint_text(tr!("daemon.qr_hint"), theme));

        div().flex().flex_col().gap(px(10.0)).child(header).child(
            div()
                .flex()
                .items_start()
                .gap(px(16.0))
                .child(left)
                .child(right),
        )
    }

    /// Expandable manual-connection details: the QR is for scanning, this is
    /// for typing. Collapsed by default so the token stays out of sight, and
    /// each row carries its own copy button with inline copied feedback.
    fn render_daemon_connection_details(
        &self,
        address: &str,
        token: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let expanded = self.daemon_connection_details_expanded;
        let mut card = div()
            .rounded(px(8.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.surface)
            .w_full()
            .min_w_0();
        let header = div()
            .id("daemon-connection-details-toggle")
            .tab_index(0)
            .px(px(10.0))
            .py(px(8.0))
            .w_full()
            .flex()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .hover(|e| e.bg(theme.overlay.opacity(0.5)))
            .focus_visible(|style| style.border_color(theme.accent))
            .child(icon(
                if expanded {
                    "icons/chevron-down.svg"
                } else {
                    "icons/chevron-right.svg"
                },
                12.0,
                theme.text_tertiary,
            ))
            .child(
                div()
                    .text_size(sp(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(tr!("daemon.connection_details")),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.daemon_connection_details_expanded = !this.daemon_connection_details_expanded;
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.modified()
                    && matches!(event.keystroke.key.as_str(), "enter" | "space")
                {
                    this.daemon_connection_details_expanded =
                        !this.daemon_connection_details_expanded;
                    cx.notify();
                    cx.stop_propagation();
                }
            }));
        card = card.child(header);
        if expanded {
            card = card
                .child(div().h(px(1.0)).w_full().bg(theme.border.opacity(0.7)))
                .child(self.daemon_detail_copy_row(
                    "daemon-connection-details-address",
                    tr!("daemon.websocket_url"),
                    address,
                    tr!("daemon.url_copied"),
                    theme,
                    cx,
                ))
                .child(self.daemon_detail_copy_row(
                    "daemon-connection-details-token",
                    tr!("daemon.token"),
                    token,
                    tr!("daemon.token_copied"),
                    theme,
                    cx,
                ));
        }
        card
    }

    /// One copyable row inside the connection details: label + mono value on
    /// the left, copy/check icon button on the right. Keyboard-operable so a
    /// screen-reader-adjacent flow (tab + enter) works like the mouse.
    fn daemon_detail_copy_row(
        &self,
        id: &str,
        label: String,
        value: &str,
        copied_toast: String,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let copied = self.control_was_copied(id);
        let copy_value = value.to_owned();
        let copy_toast = copied_toast.clone();
        let copy_id = id.to_owned();
        let copy_value_key = value.to_owned();
        let copy_id_key = id.to_owned();
        let copy_button = icon_button(
            SharedString::from(id.to_owned()),
            if copied {
                "icons/check.svg"
            } else {
                "icons/copy.svg"
            },
            *theme,
        )
        .tab_index(0)
        .focus_visible(|style| style.border_color(theme.accent))
        .tooltip(Tooltip::text(if copied {
            tr!("common.copied")
        } else {
            tr!("common.copy")
        }))
        .on_click(cx.listener(move |this, _, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(copy_value.clone()));
            this.show_control_copied(copy_id.clone(), cx);
            this.show_success_toast(copy_toast.clone());
        }))
        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
            if !event.keystroke.modifiers.modified()
                && matches!(event.keystroke.key.as_str(), "enter" | "space")
            {
                cx.write_to_clipboard(ClipboardItem::new_string(copy_value_key.clone()));
                this.show_control_copied(copy_id_key.clone(), cx);
                cx.stop_propagation();
            }
        }));
        div()
            .px(px(10.0))
            .py(px(8.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .min_w_0()
            .w_full()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(sp(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_tertiary)
                            .child(label),
                    )
                    .child(
                        div()
                            .font_family(crate::md::render::MONO_FAMILY)
                            .text_size(sp(12.0))
                            .text_color(theme.text)
                            .truncate()
                            .child(SharedString::from(value.to_owned())),
                    ),
            )
            .child(copy_button)
    }

    /// The address another device should use for the selected transport.
    ///
    /// Direct, Tailscale and SSH only *describe* where the exposed daemon can
    /// already be reached, so they are derived here. Cloudflare has to open a
    /// tunnel before an address exists at all. Remote transports always use
    /// `wss://`; Direct stays `ws://` and is only safe on a private network
    /// (the mobile client blocks public `ws://`).
    fn daemon_qr_address(&self, cx: &App) -> Result<String, String> {
        let port = self.state.daemon_exposure.port;
        let field = || {
            self.daemon_qr_field_input
                .read(cx)
                .content()
                .trim()
                .to_owned()
        };
        match self.daemon_qr_transport {
            HostKind::Direct => {
                let raw = field();
                let host = if raw.is_empty() {
                    // Auto-detected LAN IP first (see `Padu::new`), hostname
                    // as the offline fallback.
                    self.daemon_lan_address
                        .clone()
                        .filter(|candidate| !candidate.trim().is_empty())
                        .unwrap_or_else(|| self.daemon_hostname.clone())
                } else {
                    strip_scheme(&raw)
                };
                if host.is_empty() {
                    return Err(tr!("daemon.qr_need_direct"));
                }
                // Accept either a bare hostname (`192.168.1.10`) or a full
                // `host:port` so a pasted LAN URL does not double the port.
                if host.contains(':') && !host.starts_with('[') {
                    Ok(format!("ws://{host}"))
                } else {
                    Ok(format!("ws://{host}:{port}"))
                }
            }
            HostKind::Tailscale => {
                let name = field();
                if name.is_empty() {
                    return Err(tr!("daemon.qr_need_name"));
                }
                let host = strip_scheme(&name);
                Ok(format!("wss://{host}:{port}"))
            }
            HostKind::SshRelay => {
                let host = field();
                if host.is_empty() {
                    return Err(tr!("daemon.qr_need_host"));
                }
                let host = strip_scheme(&host);
                let remote_port: u16 = self
                    .daemon_qr_port_input
                    .read(cx)
                    .content()
                    .trim()
                    .parse()
                    .map_err(|_| tr!("daemon.qr_need_port"))?;
                if remote_port == 0 {
                    return Err(tr!("daemon.qr_need_port"));
                }
                Ok(format!("wss://{host}:{remote_port}"))
            }
            HostKind::Cloudflare => {
                let slot = self
                    .host_transports
                    .get(DAEMON_QR_TUNNEL_KEY)
                    .ok_or_else(|| tr!("daemon.qr_needs_tunnel"))?;
                let guard = slot
                    .try_lock()
                    .ok_or_else(|| tr!("daemon.qr_tunnel_starting"))?;
                match guard.status() {
                    TransportStatus::Ready { address, .. } => Ok(address),
                    TransportStatus::Starting => Err(tr!("daemon.qr_tunnel_starting")),
                    // A tunnel that published a URL and then died must surface
                    // its real reason — not the generic "start the tunnel"
                    // hint — or the QR silently never comes back.
                    TransportStatus::Failed { error } => Err(error),
                    TransportStatus::Stopped | TransportStatus::Idle => {
                        Err(tr!("daemon.qr_needs_tunnel"))
                    }
                }
            }
        }
    }

    fn render_daemon_qr_tunnel_button(&self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let starting = self.daemon_qr_tunnel_starting;
        let started = self.host_transports.get(DAEMON_QR_TUNNEL_KEY).is_some();
        let button = div()
            .id("daemon-qr-tunnel-button")
            .tab_index(0)
            .h(px(28.0))
            .px(px(12.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .cursor_pointer()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .opacity(if starting { 0.55 } else { 1.0 })
            .focus_visible(|style| style.border_color(theme.accent))
            .when(!starting, |element| {
                element
                    .hover(|e| e.bg(theme.overlay))
                    .on_click(cx.listener(|this, _, _, cx| this.start_daemon_qr_tunnel(cx)))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if !event.keystroke.modifiers.modified()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            this.start_daemon_qr_tunnel(cx);
                            cx.stop_propagation();
                        }
                    }))
            })
            .child(if starting {
                tr!("daemon.qr_tunnel_starting")
            } else if started {
                tr!("daemon.qr_restart_tunnel")
            } else {
                tr!("daemon.qr_start_tunnel")
            });
        let mut column = div().flex().flex_col().gap(px(6.0)).child(button);
        if let Some(error) = self.daemon_qr_tunnel_error.clone() {
            column = column.child(
                div()
                    .text_size(sp(11.5))
                    .line_height(sp(15.0))
                    .text_color(gpui::hsla(0.0, 0.7, 0.55, 1.0))
                    .child(error),
            );
        }
        column
    }

    /// Open (or reopen) the Cloudflare tunnel that fronts this daemon.
    fn start_daemon_qr_tunnel(&mut self, cx: &mut Context<Self>) {
        let local_port = self.state.daemon_exposure.port;
        let token = self.state.daemon_exposure.token.clone();
        if self.daemon_qr_tunnel_starting {
            return;
        }
        // A restart must not reuse the old tunnel's now-dead address — and
        // must not leak it either: `remove` alone would leave the old
        // `cloudflared` running with the stale URL.
        if let Some(old) = self.host_transports.remove(DAEMON_QR_TUNNEL_KEY) {
            smol::spawn(async move {
                let mut old = old.lock().await;
                let _ = old.stop().await;
            })
            .detach();
        }
        self.daemon_qr_cache.replace(None);
        self.daemon_qr_tunnel_error = None;
        self.daemon_qr_tunnel_starting = true;
        cx.notify();

        let registry = self.host_transports.clone();
        cx.spawn(async move |this, cx| {
            let (transport, outcome) = cx
                .background_executor()
                .spawn(async move {
                    let mut transport = CloudflareTransport::new();
                    let context = TransportContext { local_port, token };
                    let outcome = transport.start(&context).await;
                    (transport, outcome)
                })
                .await;
            match outcome {
                Ok(_) => {
                    registry.register(DAEMON_QR_TUNNEL_KEY, Box::new(transport));
                    let _ = this.update(cx, |this, cx| {
                        this.daemon_qr_tunnel_starting = false;
                        this.daemon_qr_cache.replace(None);
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.daemon_qr_tunnel_starting = false;
                        this.daemon_qr_tunnel_error = Some(error.to_string());
                        this.show_toast(tr!("daemon.qr_tunnel_failed", error = error.to_string()));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
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
        // The QR caches its rendered SVG keyed on the encoded payload (which
        // carries the token), so it refreshes on the next frame regardless —
        // clear it eagerly so no stale code lingers.
        self.daemon_qr_cache.replace(None);
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

fn hint_text(message: impl Into<SharedString>, theme: &Theme) -> Div {
    div()
        .text_size(sp(11.5))
        .line_height(sp(16.0))
        .text_color(theme.text_tertiary)
        .child(message.into())
}

/// Strip a pasted URL scheme (`ws://`, `wss://`, `http(s)://`) so a full LAN
/// URL pasted into a hostname field does not produce `ws://ws://…`.
fn strip_scheme(raw: &str) -> String {
    let trimmed = raw.trim();
    for prefix in ["wss://", "ws://", "https://", "http://"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return rest.trim_end_matches('/').to_owned();
        }
    }
    trimmed.trim_end_matches('/').to_owned()
}

/// Instruction panel on the right of the credential code: what the other
/// device needs for this transport. `tr!` needs a literal key, so the copy
/// is resolved through a match.
fn qr_instruction_panel(kind: HostKind, theme: &Theme) -> Div {
    let (title, body) = match kind {
        HostKind::Direct => (tr!("host.transport_direct"), tr!("daemon.qr_direct_hint")),
        HostKind::Tailscale => (
            tr!("host.transport_tailscale"),
            tr!("daemon.qr_tailscale_hint"),
        ),
        HostKind::Cloudflare => (
            tr!("host.transport_cloudflare"),
            tr!("daemon.qr_cloudflare_hint"),
        ),
        HostKind::SshRelay => (tr!("host.transport_ssh"), tr!("daemon.qr_ssh_hint")),
    };
    let mut panel = div()
        .flex_1()
        .min_w_0()
        .px(px(12.0))
        .py(px(10.0))
        .rounded(px(8.0))
        .bg(theme.inset)
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(icon("icons/info.svg", 13.0, theme.text_tertiary))
                .child(
                    div()
                        .text_size(sp(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(title),
                ),
        )
        .child(
            div()
                .whitespace_normal()
                .text_size(sp(12.0))
                .line_height(sp(17.0))
                .text_color(theme.text_secondary)
                .child(body),
        );
    if kind == HostKind::Cloudflare {
        // The one prerequisite that blocks everyone: no binary, no tunnel.
        panel = panel.child(
            div()
                .font_family(crate::md::render::MONO_FAMILY)
                .text_size(sp(11.5))
                .text_color(theme.text_tertiary)
                .child(SharedString::from(tr!("daemon.qr_cloudflare_install"))),
        );
    }
    panel
}

/// Centered wrapper for the credential QR: the code keeps its natural size
/// and never stretches with the column.
fn render_qr_centered(svg: &str, theme: &Theme) -> Div {
    div()
        .w_full()
        .flex()
        .justify_center()
        .child(render_qr_image(svg, theme))
}

/// Paint a QR SVG. GPUI rasterizes `ImageFormat::Svg` through resvg, and
/// `Image::from_bytes` keys its cache on the content hash, so a rebuilt image
/// is a cache hit rather than a re-rasterization.
fn render_qr_image(svg: &str, theme: &Theme) -> Div {
    let image = Arc::new(Image::from_bytes(ImageFormat::Svg, svg.as_bytes().to_vec()));
    div()
        .rounded(px(6.0))
        .border_1()
        .border_color(theme.border_strong)
        // A QR has to be scanned on white, whatever the app theme is.
        .bg(gpui::hsla(0.0, 0.0, 1.0, 1.0))
        .p(px(8.0))
        .child(
            img(image)
                .id("daemon-credentials-qr")
                .w(px(DAEMON_QR_PX as f32))
                .h(px(DAEMON_QR_PX as f32)),
        )
}
