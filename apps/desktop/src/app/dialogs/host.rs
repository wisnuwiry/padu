//! Modal editor for creating and editing remote daemon host profiles.

use std::sync::Arc;

use gpui::{Image, ImageFormat, KeyBinding, actions};

use padu_client::persistence::{
    CloudflareHostConfig, HostKind, HostProfile, SshHostConfig, TailscaleHostConfig,
    normalize_daemon_address,
};
use padu_client::transport::{
    CloudflareTransport, QrPayload, Transport, TransportContext, render_svg,
};

use crate::app::*;
use crate::ui::dialog::dialog_backdrop;

actions!(padu_host_dialog, [ConfirmHostDialog, DismissHostDialog]);

const DIALOG_CONTEXT: &str = "HostDialog";
const DIALOG_INPUT_CONTEXT: &str = "HostDialog > TextInput";
/// Edge length, in pixels, of the rendered QR code.
const QR_TARGET_PX: f32 = 180.0;

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", ConfirmHostDialog, Some(DIALOG_INPUT_CONTEXT)),
        KeyBinding::new("enter", ConfirmHostDialog, Some(DIALOG_CONTEXT)),
        KeyBinding::new(
            "secondary-enter",
            ConfirmHostDialog,
            Some(DIALOG_INPUT_CONTEXT),
        ),
        KeyBinding::new("secondary-enter", ConfirmHostDialog, Some(DIALOG_CONTEXT)),
        KeyBinding::new("escape", DismissHostDialog, Some(DIALOG_CONTEXT)),
    ]);
}

pub(crate) struct HostDialogRequest {
    pub editing_profile_id: Option<String>,
}

/// Live tunnel state for the dialog. The tunnel is spawned lazily when the
/// user asks for one, and kept alive in the app's transport registry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum TunnelState {
    #[default]
    Idle,
    Starting,
    Ready {
        address: String,
    },
    Failed(String),
}

/// What the "scan to connect" pane should show for the dialog's live values.
enum QrPanel {
    /// The form is not complete yet; `reason` says what is missing.
    Unavailable { reason: String },
    /// A tunnel is being provisioned, so no address exists yet.
    Pending,
    Ready {
        svg: String,
        address: String,
        has_token: bool,
    },
    /// No dialog is open.
    Empty,
}

pub(crate) struct HostDialogState {
    pub editing_profile_id: Option<String>,
    pub kind: HostKind,
    pub name_input: Entity<TextInput>,
    pub address_input: Entity<TextInput>,
    pub token_input: Entity<TextInput>,
    pub ssh_user_input: Entity<TextInput>,
    pub ssh_host_input: Entity<TextInput>,
    pub ssh_port_input: Entity<TextInput>,
    pub ssh_identity_input: Entity<TextInput>,
    /// Registry key for a dialog-owned tunnel, so it can be replaced or
    /// stopped when the dialog closes.
    pub tunnel_key: String,
    pub tunnel: TunnelState,
    /// `(encoded payload, rendered SVG)` for the last payload the panel drew.
    ///
    /// The QR is regenerated only when the encoded payload changes, so typing
    /// in an unrelated field does not re-encode the matrix every frame.
    pub qr_cache: Option<(String, String)>,
    /// Whether the "scan to connect" section is open. Collapsed by default so
    /// the dialog stays a short column for the common case of editing fields.
    pub qr_expanded: bool,
    pub error: Option<String>,
    pub save_focus: FocusHandle,
    pub cancel_focus: FocusHandle,
    pub delete_focus: Option<FocusHandle>,
}

/// Form values collected from the host dialog. Plain data so profile building
/// can be unit-tested without a GPUI window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HostFormValues {
    pub name: String,
    pub address: String,
    pub token: String,
    pub ssh_user: String,
    pub ssh_host: String,
    pub ssh_remote_port: String,
    pub ssh_identity_file: String,
    /// Resolved `wss://…` URL for a Cloudflare Quick Tunnel.
    pub cloudflare_address: Option<String>,
}

/// Split a normalized `ws(s)://host[:port]` address into `(host, port)`.
fn split_host_port(address: &str) -> Result<(String, u16), String> {
    let rest = address
        .strip_prefix("wss://")
        .or_else(|| address.strip_prefix("ws://"))
        .ok_or_else(|| "Enter a ws:// or wss:// address".to_string())?;
    let (host, port_str) = if let Some(close) = rest.find(']') {
        let host = &rest[..=close];
        let port = rest[close + 1..]
            .strip_prefix(':')
            .ok_or_else(|| "Include the port".to_string())?;
        (host.to_string(), port)
    } else {
        let (host, port) = rest
            .rsplit_once(':')
            .ok_or_else(|| "Include the port".to_string())?;
        (host.to_string(), port)
    };
    let port: u16 = port_str
        .parse()
        .map_err(|_| "Enter a valid port".to_string())?;
    if port == 0 {
        return Err("Enter a valid port".to_string());
    }
    Ok((host, port))
}

/// Build a `HostProfile` from raw dialog fields.
///
/// Every transport resolves to a `wss://…` `address` up front, because that is
/// the only field `DaemonSupervisor::connect` reads. The transport-specific
/// block is stored alongside for re-provisioning and status display.
pub(crate) fn build_host_profile(
    kind: HostKind,
    editing: Option<&HostProfile>,
    values: &HostFormValues,
    now: u64,
) -> Result<HostProfile, String> {
    let token = {
        let trimmed = values.token.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    };

    let (address, tailscale, cloudflare, ssh) = match kind {
        HostKind::Direct => {
            let address = normalize_daemon_address(&values.address).map_err(|e| e.to_string())?;
            (address, None, None, None)
        }
        HostKind::Tailscale => {
            let address = normalize_daemon_address(&values.address).map_err(|e| e.to_string())?;
            let (magic_dns, port) = split_host_port(&address)?;
            (
                address,
                Some(TailscaleHostConfig { magic_dns, port }),
                None,
                None,
            )
        }
        HostKind::Cloudflare => {
            let resolved = values
                .cloudflare_address
                .clone()
                .ok_or_else(|| "Start the tunnel before saving".to_string())?;
            let hostname = resolved
                .strip_prefix("wss://")
                .or_else(|| resolved.strip_prefix("ws://"))
                .ok_or_else(|| "The tunnel URL must use wss://".to_string())?
                .trim_end_matches('/')
                .to_string();
            (
                resolved,
                None,
                Some(CloudflareHostConfig {
                    hostname,
                    quick_tunnel: true,
                }),
                None,
            )
        }
        HostKind::SshRelay => {
            let user = values.ssh_user.trim();
            if user.is_empty() {
                return Err("Enter the SSH user".into());
            }
            let host = values.ssh_host.trim();
            if host.is_empty() {
                return Err("Enter the SSH host".into());
            }
            let remote_port: u16 = values
                .ssh_remote_port
                .trim()
                .parse()
                .map_err(|_| "Enter a valid remote port".to_string())?;
            if remote_port == 0 {
                return Err("Enter a valid remote port".into());
            }
            let identity = values.ssh_identity_file.trim();
            (
                format!("wss://{host}:{remote_port}"),
                None,
                None,
                Some(SshHostConfig {
                    user: user.to_string(),
                    host: host.to_string(),
                    remote_port,
                    identity_file: (!identity.is_empty()).then(|| identity.to_string()),
                    use_tls: false,
                }),
            )
        }
    };

    let name = if values.name.trim().is_empty() {
        padu_client::persistence::display_host(&address)
    } else {
        values.name.trim().to_string()
    };

    Ok(HostProfile {
        id: editing
            .map(|profile| profile.id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
        name,
        kind,
        address,
        token,
        tailscale,
        cloudflare,
        ssh,
        created_at: editing.map(|profile| profile.created_at).unwrap_or(now),
        updated_at: now,
        last_connected_at: editing.and_then(|profile| profile.last_connected_at),
    })
}

/// The payload a phone would import for the dialog's current values.
///
/// Built through [`build_host_profile`] so the QR always matches what saving
/// would produce — a code that scanned into a different address than the saved
/// profile would be worse than no code at all. The error is the same
/// human-readable reason the save path would report, so the panel can say why
/// no code is available yet (most often: the tunnel has not been started).
pub(crate) fn qr_payload_for(
    kind: HostKind,
    editing: Option<&HostProfile>,
    values: &HostFormValues,
    now: u64,
) -> Result<QrPayload, String> {
    let profile = build_host_profile(kind, editing, values, now)?;
    Ok(QrPayload {
        kind: profile.kind,
        url: profile.address,
        token: profile.token.unwrap_or_default(),
        name: profile.name,
    })
}

/// Transport options shown in the dialog's segmented control.
const TRANSPORT_OPTIONS: [HostKind; 4] = [
    HostKind::Direct,
    HostKind::Tailscale,
    HostKind::Cloudflare,
    HostKind::SshRelay,
];

/// `tr!` needs a literal key, so the label is resolved through a match rather
/// than a dynamic lookup.
fn transport_label(kind: HostKind) -> String {
    match kind {
        HostKind::Direct => tr!("host.transport_direct"),
        HostKind::Tailscale => tr!("host.transport_tailscale"),
        HostKind::Cloudflare => tr!("host.transport_cloudflare"),
        HostKind::SshRelay => tr!("host.transport_ssh"),
    }
}

impl Padu {
    pub(crate) fn request_host_dialog(
        &mut self,
        editing_profile_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.host_dialog_request = Some(HostDialogRequest { editing_profile_id });
        cx.notify();
    }

    fn materialize_host_dialog(
        &mut self,
        request: HostDialogRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing = request
            .editing_profile_id
            .as_ref()
            .and_then(|id| self.state.hosts.iter().find(|h| &h.id == id))
            .cloned();

        let initial_name = existing
            .as_ref()
            .map(|h| h.name.clone())
            .unwrap_or_default();
        let initial_address = existing
            .as_ref()
            .map(|h| h.address.clone())
            .unwrap_or_default();
        let initial_token = existing
            .as_ref()
            .and_then(|h| h.token.clone())
            .unwrap_or_default();
        let initial_kind = existing.as_ref().map(|h| h.kind).unwrap_or_default();
        let initial_ssh = existing.as_ref().and_then(|h| h.ssh.clone());
        let initial_ssh_user = initial_ssh
            .as_ref()
            .map(|s| s.user.clone())
            .unwrap_or_default();
        let initial_ssh_host = initial_ssh
            .as_ref()
            .map(|s| s.host.clone())
            .unwrap_or_default();
        let initial_ssh_port = initial_ssh
            .as_ref()
            .map(|s| s.remote_port.to_string())
            .unwrap_or_else(|| "19999".to_string());
        let initial_ssh_identity = initial_ssh
            .as_ref()
            .and_then(|s| s.identity_file.clone())
            .unwrap_or_default();

        // A saved Cloudflare profile's tunnel is re-provisioned on demand when
        // the dialog opens, so the dialog always starts from `Idle`.
        let name_input = cx.new(|cx| {
            let mut input = TextInput::new(window, cx).placeholder(tr!("host.name_placeholder"));
            if !initial_name.is_empty() {
                input.set_content(initial_name, cx);
            }
            input
        });

        let address_input = cx.new(|cx| {
            let mut input = TextInput::new(window, cx).placeholder(tr!("host.address_placeholder"));
            if !initial_address.is_empty() {
                input.set_content(initial_address, cx);
            }
            input
        });

        let token_input = cx.new(|cx| {
            let mut input = TextInput::new(window, cx).placeholder(tr!("host.token_placeholder"));
            if !initial_token.is_empty() {
                input.set_content(initial_token, cx);
            }
            input
        });

        let ssh_user_input = cx.new(|cx| {
            let mut input =
                TextInput::new(window, cx).placeholder(tr!("host.ssh_user_placeholder"));
            if !initial_ssh_user.is_empty() {
                input.set_content(initial_ssh_user, cx);
            }
            input
        });
        let ssh_host_input = cx.new(|cx| {
            let mut input =
                TextInput::new(window, cx).placeholder(tr!("host.ssh_host_placeholder"));
            if !initial_ssh_host.is_empty() {
                input.set_content(initial_ssh_host, cx);
            }
            input
        });
        let ssh_port_input = cx.new(|cx| {
            let mut input =
                TextInput::new(window, cx).placeholder(tr!("host.ssh_remote_port_placeholder"));
            input.set_content(initial_ssh_port, cx);
            input
        });
        let ssh_identity_input = cx.new(|cx| {
            let mut input =
                TextInput::new(window, cx).placeholder(tr!("host.ssh_identity_placeholder"));
            if !initial_ssh_identity.is_empty() {
                input.set_content(initial_ssh_identity, cx);
            }
            input
        });

        let address_focus = address_input.read(cx).focus();
        let name_focus = name_input.read(cx).focus();
        let first_focus = if request.editing_profile_id.is_some() {
            name_focus
        } else {
            address_focus
        };

        let is_editing = request.editing_profile_id.is_some();
        self.host_dialog = Some(HostDialogState {
            editing_profile_id: request.editing_profile_id,
            kind: initial_kind,
            name_input,
            address_input,
            token_input,
            ssh_user_input,
            ssh_host_input,
            ssh_port_input,
            ssh_identity_input,
            tunnel_key: format!("host-dialog-{}", Uuid::new_v4()),
            tunnel: TunnelState::Idle,
            qr_cache: None,
            qr_expanded: false,
            error: None,
            save_focus: cx.focus_handle(),
            cancel_focus: cx.focus_handle(),
            delete_focus: is_editing.then(|| cx.focus_handle()),
        });

        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| window.focus(&first_focus, cx));
        });
        cx.notify();
    }

    fn host_form_values(&self, cx: &App) -> HostFormValues {
        let Some(dialog) = &self.host_dialog else {
            return HostFormValues::default();
        };
        HostFormValues {
            name: dialog.name_input.read(cx).content().trim().to_string(),
            address: dialog.address_input.read(cx).content().trim().to_string(),
            token: dialog.token_input.read(cx).content().trim().to_string(),
            ssh_user: dialog.ssh_user_input.read(cx).content().trim().to_string(),
            ssh_host: dialog.ssh_host_input.read(cx).content().trim().to_string(),
            ssh_remote_port: dialog.ssh_port_input.read(cx).content().trim().to_string(),
            ssh_identity_file: dialog
                .ssh_identity_input
                .read(cx)
                .content()
                .trim()
                .to_string(),
            cloudflare_address: match &dialog.tunnel {
                TunnelState::Ready { address, .. } => Some(address.clone()),
                _ => None,
            },
        }
    }

    /// Provision a Cloudflare Quick Tunnel for the dialog's current fields.
    /// The tunnel is registered in the app registry so it stays alive after
    /// the dialog closes; the resolved hostname and QR land back in the dialog.
    pub(crate) fn start_host_tunnel(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = &self.host_dialog else {
            return;
        };
        if matches!(dialog.tunnel, TunnelState::Starting) {
            return;
        }
        let tunnel_key = dialog.tunnel_key.clone();
        let token = dialog.token_input.read(cx).content().trim().to_string();
        let local_port = self.state.daemon_exposure.port;

        if let Some(dialog) = self.host_dialog.as_mut() {
            dialog.tunnel = TunnelState::Starting;
            dialog.error = None;
        }
        cx.notify();

        let registry = self.host_transports.clone();
        let transport_token = token.clone();
        cx.spawn(async move |this, cx| {
            let (transport, outcome) = cx
                .background_executor()
                .spawn(async move {
                    let mut transport = CloudflareTransport::new();
                    let context = TransportContext {
                        local_port,
                        token: transport_token,
                    };
                    let outcome = transport.start(&context).await;
                    (transport, outcome)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                match outcome {
                    Ok(handle) => {
                        registry.register(tunnel_key, Box::new(transport));
                        if let Some(dialog) = this.host_dialog.as_mut() {
                            dialog.tunnel = TunnelState::Ready {
                                address: handle.address,
                            };
                            // The panel re-derives the code from the new
                            // address, so the cache must not outlive it.
                            dialog.qr_cache = None;
                        }
                    }
                    Err(error) => {
                        if let Some(dialog) = this.host_dialog.as_mut() {
                            dialog.tunnel = TunnelState::Failed(error.to_string());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn close_host_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.host_dialog_request = None;
        let Some(dialog) = self.host_dialog.take() else {
            return;
        };
        // A tunnel provisioned but not saved is discarded; one that was saved
        // is re-registered under the profile id by `switch_to_host`.
        if let Some(transport) = self.host_transports.get(&dialog.tunnel_key) {
            let registry = self.host_transports.clone();
            let key = dialog.tunnel_key.clone();
            cx.spawn(async move |_, _| {
                let mut transport = transport.lock().await;
                let _ = transport.stop().await;
                drop(transport);
                registry.remove(&key);
            })
            .detach();
        }
        let focus = self.composer_focus(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    pub(crate) fn host_dialog_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &self.host_dialog else {
            return;
        };
        let kind = dialog.kind;
        let editing_id = dialog.editing_profile_id.clone();
        let editing = editing_id
            .as_ref()
            .and_then(|id| self.state.hosts.iter().find(|h| &h.id == id))
            .cloned();
        let values = self.host_form_values(cx);

        let profile = match build_host_profile(kind, editing.as_ref(), &values, unix_time()) {
            Ok(profile) => profile,
            Err(error) => {
                if let Some(dialog) = self.host_dialog.as_mut() {
                    dialog.error = Some(error);
                }
                cx.notify();
                return;
            }
        };

        let profile_id = profile.id.clone();
        if let Some(existing) = self.state.hosts.iter_mut().find(|h| h.id == profile_id) {
            let created_at = existing.created_at;
            let last_connected_at = existing.last_connected_at;
            *existing = profile;
            existing.created_at = created_at;
            existing.last_connected_at = last_connected_at;
        } else {
            self.state.add_host_profile(profile);
        }

        let _ = self.store.write_app_settings(&self.state.app_settings());

        // A tunnel provisioned in the dialog is handed to the saved profile
        // instead of being torn down on close, so `switch_to_host` reuses it
        // rather than spawning a second tunnel with a different URL.
        if kind == HostKind::Cloudflare
            && let Some(dialog) = &self.host_dialog
        {
            self.host_transports.rekey(&dialog.tunnel_key, &profile_id);
        }

        self.close_host_dialog(window, cx);
        self.switch_to_host(Some(profile_id), cx);
    }

    /// Open or close the "scan to connect" section.
    fn toggle_host_qr(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.host_dialog.as_mut() {
            dialog.qr_expanded = !dialog.qr_expanded;
        }
        cx.notify();
    }

    pub(crate) fn host_dialog_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &self.host_dialog else {
            return;
        };
        let Some(editing_id) = dialog.editing_profile_id.clone() else {
            return;
        };

        self.confirm_delete_host(editing_id, window, cx);
    }

    pub(crate) fn render_host_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if let Some(request) = self.host_dialog_request.take() {
            self.materialize_host_dialog(request, window, cx);
        }
        let theme = Theme::current(cx);
        // Snapshot the dialog so the borrow ends before `current_qr_pane`,
        // which needs `&mut self` to refresh its cache.
        let (is_editing, kind, tunnel, error_message) = {
            let dialog = self.host_dialog.as_ref()?;
            (
                dialog.editing_profile_id.is_some(),
                dialog.kind,
                dialog.tunnel.clone(),
                dialog.error.clone(),
            )
        };
        let (name_input, address_input, token_input) = {
            let dialog = self.host_dialog.as_ref()?;
            (
                dialog.name_input.clone(),
                dialog.address_input.clone(),
                dialog.token_input.clone(),
            )
        };
        let (ssh_user_input, ssh_host_input, ssh_port_input, ssh_identity_input) = {
            let dialog = self.host_dialog.as_ref()?;
            (
                dialog.ssh_user_input.clone(),
                dialog.ssh_host_input.clone(),
                dialog.ssh_port_input.clone(),
                dialog.ssh_identity_input.clone(),
            )
        };
        let (save_focus, cancel_focus, delete_focus) = {
            let dialog = self.host_dialog.as_ref()?;
            (
                dialog.save_focus.clone(),
                dialog.cancel_focus.clone(),
                dialog.delete_focus.clone(),
            )
        };

        let title = if is_editing {
            tr!("host.edit_host")
        } else {
            tr!("host.add_host")
        };
        let save_label = if is_editing {
            tr!("host.save")
        } else {
            tr!("host.save_and_connect")
        };

        let panel = self.current_qr_panel(cx, &tunnel);
        let qr_expanded = self
            .host_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.qr_expanded);
        let qr_section = self.render_qr_section(&panel, kind, &tunnel, qr_expanded, &theme, cx);

        let mut fields = div().flex().flex_col().gap(px(12.0)).child(labelled_field(
            tr!("host.name"),
            theme.text_secondary,
            text_field_box(&theme, false, name_input),
        ));

        match kind {
            HostKind::Direct | HostKind::Tailscale => {
                fields = fields.child(labelled_field(
                    tr!("host.address"),
                    theme.text_secondary,
                    text_field_box(&theme, error_message.is_some(), address_input),
                ));
                if kind == HostKind::Tailscale {
                    fields = fields.child(hint_text(tr!("host.tailscale_hint"), &theme));
                }
                fields = fields.child(labelled_field(
                    tr!("host.token"),
                    theme.text_secondary,
                    text_field_box(&theme, false, token_input),
                ));
            }
            HostKind::Cloudflare => {
                // The tunnel lives in the right pane: the address it reports
                // is what the code encodes, so keeping them together makes
                // "start the tunnel, then scan" one continuous step.
                fields = fields
                    .child(hint_text(tr!("host.tunnel_idle_hint"), &theme))
                    .child(labelled_field(
                        tr!("host.token"),
                        theme.text_secondary,
                        text_field_box(&theme, false, token_input),
                    ));
            }
            HostKind::SshRelay => {
                fields = fields
                    .child(labelled_field(
                        tr!("host.ssh_user"),
                        theme.text_secondary,
                        text_field_box(&theme, false, ssh_user_input),
                    ))
                    .child(labelled_field(
                        tr!("host.ssh_host"),
                        theme.text_secondary,
                        text_field_box(&theme, false, ssh_host_input),
                    ))
                    .child(labelled_field(
                        tr!("host.ssh_remote_port"),
                        theme.text_secondary,
                        text_field_box(&theme, false, ssh_port_input),
                    ))
                    .child(labelled_field(
                        tr!("host.ssh_identity_file"),
                        theme.text_secondary,
                        text_field_box(&theme, false, ssh_identity_input),
                    ))
                    .child(labelled_field(
                        tr!("host.token"),
                        theme.text_secondary,
                        text_field_box(&theme, false, token_input),
                    ));
            }
        }

        if let Some(error) = error_message {
            fields = fields.child(
                div()
                    .text_size(sp(11.5))
                    .text_color(gpui::hsla(0.0, 0.7, 0.55, 1.0))
                    .child(error),
            );
        }

        let card = div()
            .key_context(DIALOG_CONTEXT)
            .on_action(cx.listener(|this, _: &ConfirmHostDialog, window, cx| {
                this.host_dialog_save(window, cx);
            }))
            .on_action(cx.listener(|this, _: &DismissHostDialog, window, cx| {
                this.close_host_dialog(window, cx);
            }))
            .id("host-dialog-card")
            .w(px(500.0))
            .rounded(px(14.0))
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.raised)
            .shadow_lg()
            .p(px(20.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            // Header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(sp(15.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(title),
                    )
                    .child(
                        div()
                            .id("close-host-dialog")
                            .tab_index(0)
                            .size(px(24.0))
                            .rounded(px(6.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|e| e.bg(theme.overlay))
                            .child(icon("icons/x.svg", 12.0, theme.text_tertiary))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_host_dialog(window, cx);
                            })),
                    ),
            )
            // Transport selector
            .child(self.render_transport_selector(kind, &theme, cx))
            // One column: what to connect to, then an optional code a phone
            // can scan. Collapsed by default so editing a host stays short.
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(section_label(tr!("host.connection_section"), &theme))
                    .child(fields),
            )
            .child(qr_section)
            // Footer Actions
            .child(
                div()
                    .pt(px(4.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .when_some(delete_focus.clone(), |row, del_focus| {
                        row.child(
                            div()
                                .id("delete-host-button")
                                .track_focus(&del_focus)
                                .tab_index(0)
                                .h(px(30.0))
                                .px(px(12.0))
                                .rounded(px(7.0))
                                .cursor_pointer()
                                .text_size(sp(12.5))
                                .text_color(gpui::hsla(0.0, 0.7, 0.55, 1.0))
                                .focus_visible(|style| style.border_1().border_color(theme.accent))
                                .hover(|e| e.bg(theme.overlay))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(tr!("host.remove_host"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.host_dialog_delete(window, cx);
                                }))
                                .on_key_down(cx.listener(
                                    |this, event: &KeyDownEvent, window, cx| {
                                        if !event.keystroke.modifiers.modified()
                                            && matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            )
                                        {
                                            this.host_dialog_delete(window, cx);
                                            cx.stop_propagation();
                                        }
                                    },
                                )),
                        )
                    })
                    .when(delete_focus.is_none(), |row| row.child(div()))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .id("cancel-host-dialog")
                                    .track_focus(&cancel_focus)
                                    .tab_index(0)
                                    .h(px(30.0))
                                    .px(px(12.0))
                                    .gap(px(6.0))
                                    .rounded(px(7.0))
                                    .border_1()
                                    .border_color(theme.border_strong)
                                    .cursor_pointer()
                                    .text_size(sp(12.5))
                                    .text_color(theme.text_secondary)
                                    .focus_visible(|style| {
                                        style.border_1().border_color(theme.accent)
                                    })
                                    .hover(|e| e.bg(theme.overlay))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(tr!("common.cancel"))
                                    .child(kbd_badge("Esc", &theme))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_host_dialog(window, cx);
                                    }))
                                    .on_key_down(cx.listener(
                                        |this, event: &KeyDownEvent, window, cx| {
                                            if !event.keystroke.modifiers.modified()
                                                && matches!(
                                                    event.keystroke.key.as_str(),
                                                    "enter" | "space"
                                                )
                                            {
                                                this.close_host_dialog(window, cx);
                                                cx.stop_propagation();
                                            }
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .id("save-host-dialog")
                                    .track_focus(&save_focus)
                                    .tab_index(0)
                                    .h(px(30.0))
                                    .px(px(14.0))
                                    .gap(px(6.0))
                                    .rounded(px(7.0))
                                    .bg(theme.inverse)
                                    .cursor_pointer()
                                    .text_size(sp(12.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.on_inverse)
                                    .focus_visible(|style| {
                                        style.border_1().border_color(theme.accent)
                                    })
                                    .hover(|e| e.opacity(0.9))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(save_label)
                                    .child(kbd_badge_icon(
                                        "icons/corner-down-left.svg",
                                        theme.inverse,
                                        theme.on_inverse,
                                        gpui::hsla(0.0, 0.0, 1.0, 0.2),
                                    ))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.host_dialog_save(window, cx);
                                    }))
                                    .on_key_down(cx.listener(
                                        |this, event: &KeyDownEvent, window, cx| {
                                            if !event.keystroke.modifiers.modified()
                                                && matches!(
                                                    event.keystroke.key.as_str(),
                                                    "enter" | "space"
                                                )
                                            {
                                                this.host_dialog_save(window, cx);
                                                cx.stop_propagation();
                                            }
                                        },
                                    )),
                            ),
                    ),
            );

        Some(dialog_backdrop(
            "host-dialog-layer",
            &theme,
            cx,
            |padu, window, cx| padu.close_host_dialog(window, cx),
            card,
        ))
    }

    /// Snapshot for the "scan to connect" pane.
    ///
    /// The SVG is regenerated only when the encoded payload changes, so typing
    /// in an unrelated field costs a string compare rather than a QR encode
    /// plus a fresh image allocation on every frame.
    fn current_qr_panel(&mut self, cx: &App, tunnel: &TunnelState) -> QrPanel {
        let Some(dialog) = self.host_dialog.as_ref() else {
            return QrPanel::Empty;
        };
        let kind = dialog.kind;
        let editing_id = dialog.editing_profile_id.clone();
        let values = self.host_form_values(cx);
        let editing = editing_id
            .as_ref()
            .and_then(|id| self.state.hosts.iter().find(|h| &h.id == id))
            .cloned();

        // A Cloudflare address only exists once cloudflared reports it, so say
        // that instead of echoing the save path's "start the tunnel" error.
        if kind == HostKind::Cloudflare && !matches!(tunnel, TunnelState::Ready { .. }) {
            return match tunnel {
                TunnelState::Starting => QrPanel::Pending,
                TunnelState::Failed(reason) => QrPanel::Unavailable {
                    reason: reason.clone(),
                },
                _ => QrPanel::Unavailable {
                    reason: tr!("host.qr_needs_tunnel"),
                },
            };
        }

        match qr_payload_for(kind, editing.as_ref(), &values, unix_time()) {
            Ok(payload) => {
                let key = payload.encode().unwrap_or_default();
                let cached = self.host_dialog.as_ref().and_then(|d| d.qr_cache.clone());
                let svg = match cached {
                    Some((cached_key, svg)) if cached_key == key => svg,
                    _ => {
                        let svg = render_svg(&payload, QR_TARGET_PX as u32).unwrap_or_default();
                        if let Some(dialog) = self.host_dialog.as_mut() {
                            dialog.qr_cache = Some((key, svg.clone()));
                        }
                        svg
                    }
                };
                QrPanel::Ready {
                    svg,
                    address: payload.url,
                    has_token: !payload.token.is_empty(),
                }
            }
            Err(reason) => QrPanel::Unavailable { reason },
        }
    }

    /// The collapsible "scan to connect" section: a live code for whatever the
    /// form currently describes, so a phone can be handed the host without
    /// typing anything.
    ///
    /// Collapsed by default — most visits only edit a field, and an expanded
    /// QR would push the save button off a short window.
    fn render_qr_section(
        &self,
        panel: &QrPanel,
        kind: HostKind,
        tunnel: &TunnelState,
        expanded: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let section = div().flex().flex_col().gap(px(10.0)).child(
            div()
                .id("toggle-host-qr")
                .tab_index(0)
                .h(px(28.0))
                .px(px(10.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(theme.border_strong)
                .flex()
                .items_center()
                .gap(px(6.0))
                .cursor_pointer()
                .text_size(sp(12.5))
                .text_color(theme.text_secondary)
                .focus_visible(|style| style.border_color(theme.accent))
                .hover(|e| e.bg(theme.overlay))
                .child(icon(
                    if expanded {
                        "icons/chevron-down.svg"
                    } else {
                        "icons/chevron-right.svg"
                    },
                    11.0,
                    theme.text_tertiary,
                ))
                .child(if expanded {
                    tr!("host.hide_qr")
                } else {
                    tr!("host.show_qr")
                })
                .on_click(cx.listener(|this, _, _, cx| this.toggle_host_qr(cx)))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if !event.keystroke.modifiers.modified()
                        && matches!(event.keystroke.key.as_str(), "enter" | "space")
                    {
                        this.toggle_host_qr(cx);
                        cx.stop_propagation();
                    }
                })),
        );

        if !expanded {
            return section;
        }

        let mut pane = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(section_label(tr!("host.qr_pane_title"), theme));

        match panel {
            QrPanel::Ready {
                svg,
                address,
                has_token,
            } => {
                pane = pane
                    .child(render_qr_image(svg, theme))
                    .child(
                        div()
                            .font_family(crate::md::render::MONO_FAMILY)
                            .text_size(sp(11.5))
                            .text_color(theme.text_secondary)
                            .truncate()
                            .child(SharedString::from(address.clone())),
                    )
                    .child(status_row(
                        if *has_token {
                            tr!("host.token_included")
                        } else {
                            tr!("host.token_missing")
                        },
                        *has_token,
                        theme,
                    ));
            }
            QrPanel::Pending => {
                pane = pane.child(pane_placeholder(tr!("host.tunnel_starting"), theme));
            }
            QrPanel::Unavailable { reason } => {
                pane = pane.child(pane_placeholder(reason.clone(), theme));
            }
            QrPanel::Empty => {}
        }

        // Only Cloudflare has to be started from here; every other transport
        // resolves its address from the fields and needs no extra step.
        if kind == HostKind::Cloudflare {
            pane = pane.child(self.render_tunnel_button(tunnel, theme, cx));
        }

        section.child(pane)
    }

    fn render_tunnel_button(
        &self,
        tunnel: &TunnelState,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let starting = matches!(tunnel, TunnelState::Starting);
        let label = if matches!(tunnel, TunnelState::Idle | TunnelState::Failed(_)) {
            tr!("host.start_tunnel")
        } else {
            tr!("host.restart_tunnel")
        };
        div()
            .id("start-tunnel-button")
            .tab_index(0)
            .h(px(30.0))
            .w_full()
            .rounded(px(7.0))
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
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.start_host_tunnel(cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if !event.keystroke.modifiers.modified()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            this.start_host_tunnel(cx);
                            cx.stop_propagation();
                        }
                    }))
            })
            .child(label)
    }

    fn render_transport_selector(
        &self,
        active: HostKind,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut row = div().flex().items_center().gap(px(6.0)).child(
            div()
                .flex_none()
                .text_size(sp(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text_secondary)
                .child(tr!("host.transport")),
        );
        for kind in TRANSPORT_OPTIONS {
            let is_active = kind == active;
            let id = SharedString::from(format!("transport-option-{:?}", kind));
            row = row.child(
                div()
                    .id(id)
                    .tab_index(0)
                    .h(px(26.0))
                    .px(px(9.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(if is_active {
                        theme.accent
                    } else {
                        theme.border
                    })
                    .bg(if is_active {
                        theme.accent.opacity(0.12)
                    } else {
                        theme.surface
                    })
                    .text_color(if is_active {
                        theme.accent
                    } else {
                        theme.text_secondary
                    })
                    .text_size(sp(12.0))
                    .font_weight(if is_active {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .cursor_pointer()
                    .focus_visible(|style| style.border_color(theme.accent))
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|e| e.bg(theme.overlay))
                    .child(transport_label(kind))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(dialog) = this.host_dialog.as_mut() {
                            dialog.kind = kind;
                            dialog.error = None;
                        }
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if !event.keystroke.modifiers.modified()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            if let Some(dialog) = this.host_dialog.as_mut() {
                                dialog.kind = kind;
                                dialog.error = None;
                            }
                            cx.notify();
                            cx.stop_propagation();
                        }
                    })),
            );
        }
        row
    }
}

fn section_label(text: impl Into<SharedString>, theme: &Theme) -> Div {
    div()
        .text_size(sp(11.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.text_tertiary)
        .child(text.into())
}

/// A quiet box standing in for the code while it cannot be drawn yet.
fn pane_placeholder(message: impl Into<SharedString>, theme: &Theme) -> Div {
    div()
        .w(px(QR_TARGET_PX))
        .h(px(QR_TARGET_PX))
        .rounded(px(6.0))
        .border_1()
        .border_color(theme.border)
        .bg(theme.overlay.opacity(0.35))
        .flex()
        .items_center()
        .justify_center()
        .p(px(14.0))
        .child(
            div()
                .text_center()
                .text_size(sp(11.5))
                .line_height(sp(16.0))
                .text_color(theme.text_tertiary)
                .child(message.into()),
        )
}

/// One line of pane status. `ok` selects the icon as well as the color, so the
/// meaning survives for anyone who cannot distinguish the two.
fn status_row(text: impl Into<SharedString>, ok: bool, theme: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(5.0))
        .text_size(sp(11.5))
        .text_color(if ok {
            theme.text_secondary
        } else {
            theme.warning
        })
        .child(icon(
            if ok {
                "icons/check.svg"
            } else {
                "icons/alert.svg"
            },
            10.5,
            if ok { theme.success } else { theme.warning },
        ))
        .child(text.into())
}

fn labelled_field(label: impl Into<SharedString>, label_color: Hsla, control: Div) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(sp(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(label_color)
                .child(label.into()),
        )
        .child(control)
}

fn text_field_box(theme: &Theme, invalid: bool, input: Entity<TextInput>) -> Div {
    div()
        .h(px(32.0))
        .px(px(10.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(if invalid { theme.accent } else { theme.border })
        .bg(theme.surface)
        .flex()
        .items_center()
        .child(input)
}

fn hint_text(text: impl Into<SharedString>, theme: &Theme) -> Div {
    div()
        .text_size(sp(11.5))
        .line_height(sp(16.0))
        .text_color(theme.text_tertiary)
        .child(text.into())
}

/// Paint the QR SVG. GPUI rasterizes `ImageFormat::Svg` through resvg, and
/// `Image::from_bytes` keys its cache on the content hash, so rebuilding the
/// image each frame is a cache hit rather than a re-rasterization.
fn render_qr_image(svg: &str, theme: &Theme) -> Div {
    let image = Arc::new(Image::from_bytes(ImageFormat::Svg, svg.as_bytes().to_vec()));
    div()
        .rounded(px(6.0))
        .border_1()
        .border_color(theme.border_strong)
        // The QR is scannable on white, so keep an opaque light plate under it
        // in both themes rather than inheriting the dialog surface.
        .bg(gpui::hsla(0.0, 0.0, 1.0, 1.0))
        .p(px(8.0))
        .child(
            img(image)
                .id("host-dialog-qr")
                .w(px(QR_TARGET_PX))
                .h(px(QR_TARGET_PX)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> HostFormValues {
        HostFormValues {
            name: "Studio".into(),
            address: "192.168.1.10:34123".into(),
            token: "  secret  ".into(),
            ssh_user: "alice".into(),
            ssh_host: "jump.example.com".into(),
            ssh_remote_port: "19999".into(),
            ssh_identity_file: String::new(),
            cloudflare_address: None,
        }
    }

    #[test]
    fn build_direct_profile_normalizes_and_trims_token() {
        let profile = build_host_profile(HostKind::Direct, None, &values(), 100).unwrap();
        assert_eq!(profile.kind, HostKind::Direct);
        assert_eq!(profile.address, "ws://192.168.1.10:34123");
        assert_eq!(profile.token.as_deref(), Some("secret"));
        assert!(profile.tailscale.is_none());
        assert!(profile.cloudflare.is_none());
        assert!(profile.ssh.is_none());
        assert_eq!(profile.created_at, 100);
        assert_eq!(profile.updated_at, 100);
    }

    #[test]
    fn build_direct_profile_names_after_address_when_blank() {
        let mut v = values();
        v.name = "   ".into();
        let profile = build_host_profile(HostKind::Direct, None, &v, 1).unwrap();
        assert_eq!(profile.name, "192.168.1.10:34123");
    }

    #[test]
    fn build_tailscale_profile_extracts_magic_dns_and_port() {
        let mut v = values();
        v.address = "wss://mac.tail-abc.ts.net:34123".into();
        let profile = build_host_profile(HostKind::Tailscale, None, &v, 1).unwrap();
        assert_eq!(profile.kind, HostKind::Tailscale);
        assert_eq!(profile.address, "wss://mac.tail-abc.ts.net:34123");
        let tailscale = profile.tailscale.expect("tailscale block");
        assert_eq!(tailscale.magic_dns, "mac.tail-abc.ts.net");
        assert_eq!(tailscale.port, 34123);
    }

    #[test]
    fn build_tailscale_profile_requires_a_port() {
        let mut v = values();
        v.address = "wss://mac.tail-abc.ts.net".into();
        let error = build_host_profile(HostKind::Tailscale, None, &v, 1).unwrap_err();
        assert!(error.contains("port"), "unexpected error: {error}");
    }

    #[test]
    fn build_cloudflare_profile_uses_resolved_tunnel_address() {
        let mut v = values();
        v.cloudflare_address = Some("wss://random-words.trycloudflare.com".into());
        let profile = build_host_profile(HostKind::Cloudflare, None, &v, 1).unwrap();
        assert_eq!(profile.kind, HostKind::Cloudflare);
        assert_eq!(profile.address, "wss://random-words.trycloudflare.com");
        let cloudflare = profile.cloudflare.expect("cloudflare block");
        assert_eq!(cloudflare.hostname, "random-words.trycloudflare.com");
        assert!(cloudflare.quick_tunnel);
    }

    #[test]
    fn build_cloudflare_profile_requires_a_started_tunnel() {
        let error = build_host_profile(HostKind::Cloudflare, None, &values(), 1).unwrap_err();
        assert!(
            error.contains("Start the tunnel"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn build_ssh_profile_derives_address_from_host_and_port() {
        let profile = build_host_profile(HostKind::SshRelay, None, &values(), 1).unwrap();
        assert_eq!(profile.kind, HostKind::SshRelay);
        assert_eq!(profile.address, "wss://jump.example.com:19999");
        let ssh = profile.ssh.expect("ssh block");
        assert_eq!(ssh.user, "alice");
        assert_eq!(ssh.host, "jump.example.com");
        assert_eq!(ssh.remote_port, 19999);
        assert!(ssh.identity_file.is_none());
    }

    #[test]
    fn build_ssh_profile_keeps_identity_file_when_set() {
        let mut v = values();
        v.ssh_identity_file = "/Users/alice/.ssh/id_ed25519".into();
        let profile = build_host_profile(HostKind::SshRelay, None, &v, 1).unwrap();
        assert_eq!(
            profile.ssh.unwrap().identity_file.as_deref(),
            Some("/Users/alice/.ssh/id_ed25519")
        );
    }

    #[test]
    fn build_ssh_profile_rejects_blank_user_host_and_bad_port() {
        let mut v = values();
        v.ssh_user = "  ".into();
        assert!(build_host_profile(HostKind::SshRelay, None, &v, 1).is_err());

        let mut v = values();
        v.ssh_host = String::new();
        assert!(build_host_profile(HostKind::SshRelay, None, &v, 1).is_err());

        let mut v = values();
        v.ssh_remote_port = "0".into();
        assert!(build_host_profile(HostKind::SshRelay, None, &v, 1).is_err());

        let mut v = values();
        v.ssh_remote_port = "not-a-port".into();
        assert!(build_host_profile(HostKind::SshRelay, None, &v, 1).is_err());
    }

    #[test]
    fn editing_preserves_id_created_at_and_last_connected() {
        let existing = HostProfile {
            id: "host-7".into(),
            name: "Old".into(),
            kind: HostKind::Direct,
            address: "ws://old:34123".into(),
            token: None,
            tailscale: None,
            cloudflare: None,
            ssh: None,
            created_at: 10,
            updated_at: 20,
            last_connected_at: Some(42),
        };
        let profile =
            build_host_profile(HostKind::Direct, Some(&existing), &values(), 999).unwrap();
        assert_eq!(profile.id, "host-7");
        assert_eq!(profile.created_at, 10);
        assert_eq!(profile.last_connected_at, Some(42));
        assert_eq!(profile.updated_at, 999);
    }

    #[test]
    fn switching_kind_drops_the_previous_transport_block() {
        let existing = HostProfile {
            id: "host-9".into(),
            name: "Tunnel".into(),
            kind: HostKind::Cloudflare,
            address: "wss://old.trycloudflare.com".into(),
            token: None,
            tailscale: None,
            cloudflare: Some(CloudflareHostConfig {
                hostname: "old.trycloudflare.com".into(),
                quick_tunnel: true,
            }),
            ssh: None,
            created_at: 1,
            updated_at: 1,
            last_connected_at: None,
        };
        let profile = build_host_profile(HostKind::Direct, Some(&existing), &values(), 2).unwrap();
        assert_eq!(profile.kind, HostKind::Direct);
        assert!(profile.cloudflare.is_none(), "stale block must be cleared");
    }

    #[test]
    fn qr_payload_matches_what_saving_would_write() {
        // A code that scanned into a different address than the saved profile
        // would be worse than no code at all.
        let values = values();
        let payload = qr_payload_for(HostKind::Direct, None, &values, 1).unwrap();
        let profile = build_host_profile(HostKind::Direct, None, &values, 1).unwrap();
        assert_eq!(payload.kind, profile.kind);
        assert_eq!(payload.url, profile.address);
        assert_eq!(payload.name, profile.name);
        assert_eq!(payload.token, profile.token.unwrap());
    }

    #[test]
    fn qr_payload_works_for_every_address_based_transport() {
        // Direct and Tailscale come straight from the address field; SSH
        // derives its address and needs no tunnel to produce a code.
        for kind in [HostKind::Direct, HostKind::Tailscale, HostKind::SshRelay] {
            let payload = qr_payload_for(kind, None, &values(), 1)
                .unwrap_or_else(|error| panic!("{kind:?} should produce a code: {error}"));
            assert_eq!(payload.kind, kind);
            assert!(payload.url.starts_with("ws"));
        }
    }

    #[test]
    fn qr_payload_for_cloudflare_needs_a_started_tunnel() {
        // No tunnel yet: the address is unknown, so there is nothing honest to
        // encode. The panel shows this reason.
        let error = qr_payload_for(HostKind::Cloudflare, None, &values(), 1).unwrap_err();
        assert!(
            error.contains("Start the tunnel"),
            "unexpected error: {error}"
        );

        let mut started = values();
        started.cloudflare_address = Some("wss://abc.trycloudflare.com".into());
        let payload = qr_payload_for(HostKind::Cloudflare, None, &started, 1).unwrap();
        assert_eq!(payload.url, "wss://abc.trycloudflare.com");
    }

    #[test]
    fn qr_payload_carries_an_empty_token_rather_than_failing() {
        // A token-less host is still importable; the pane warns instead of
        // refusing to draw a code.
        let mut v = values();
        v.token = "   ".into();
        let payload = qr_payload_for(HostKind::Direct, None, &v, 1).unwrap();
        assert!(payload.token.is_empty());
    }

    #[test]
    fn qr_payload_surfaces_the_missing_field() {
        let mut v = values();
        v.ssh_user = String::new();
        let error = qr_payload_for(HostKind::SshRelay, None, &v, 1).unwrap_err();
        assert!(error.contains("SSH user"), "unexpected error: {error}");
    }

    #[test]
    fn split_host_port_handles_ipv6_literals() {
        assert_eq!(
            split_host_port("wss://[::1]:34123").unwrap(),
            ("[::1]".to_string(), 34123)
        );
        assert!(split_host_port("wss://[::1]").is_err());
        assert!(split_host_port("wss://host:0").is_err());
        assert!(split_host_port("wss://host:abc").is_err());
    }
}
