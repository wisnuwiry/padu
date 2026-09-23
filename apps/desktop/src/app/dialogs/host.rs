//! Modal editor for creating and editing remote daemon host profiles.

use gpui::{KeyBinding, actions};

use padu_client::persistence::{
    CloudflareHostConfig, HostKind, HostProfile, SshHostConfig, TailscaleHostConfig,
    normalize_daemon_address,
};
use padu_client::transport::TailscaleConfig;

use crate::app::*;
use crate::ui::dialog::dialog_backdrop;

actions!(padu_host_dialog, [ConfirmHostDialog, DismissHostDialog]);

const DIALOG_CONTEXT: &str = "HostDialog";
const DIALOG_INPUT_CONTEXT: &str = "HostDialog > TextInput";

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
    /// When opened from a credential card ("Add as Remote Host"), the address
    /// and token are pre-filled so the user only needs to confirm the name.
    pub prefill: Option<(String, String)>,
}

pub(crate) struct HostDialogState {
    pub editing_profile_id: Option<String>,
    /// Which tab the dialog shows. `Default` covers Direct, Tailscale, and
    /// Cloudflare — all three are the same inputs (name, address, token) and
    /// the kind is inferred from the address at save time.
    pub tab: HostTab,
    pub name_input: Entity<TextInput>,
    pub address_input: Entity<TextInput>,
    pub token_input: Entity<TextInput>,
    pub ssh_user_input: Entity<TextInput>,
    pub ssh_host_input: Entity<TextInput>,
    pub ssh_port_input: Entity<TextInput>,
    pub ssh_identity_input: Entity<TextInput>,
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

/// Host part of a normalized `ws(s)://host[:port]` address, without the port.
/// Used to probe the transport kind when no port is present (a Quick Tunnel
/// URL carries none, and only Tailscale/Direct need `host:port`).
fn host_without_port(address: &str) -> String {
    let rest = address
        .strip_prefix("wss://")
        .or_else(|| address.strip_prefix("ws://"))
        .unwrap_or(address);
    let host_part = rest.split('/').next().unwrap_or(rest);
    if host_part.starts_with('[') {
        if let Some(close) = host_part.find(']') {
            return host_part[..=close].to_string();
        }
        return host_part.to_string();
    }
    match host_part.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => {
            host.to_string()
        }
        _ => host_part.to_string(),
    }
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
            if !TailscaleConfig::is_tailnet_host(&magic_dns) {
                return Err(
                    "Use Direct for LAN addresses — Tailscale needs a MagicDNS (*.ts.net) or 100.x tailnet IP".to_string(),
                );
            }
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

/// Tabs in the host dialog. There are only two: `Default` handles Direct,
/// Tailscale, and Cloudflare through the same name/address/token inputs
/// (the transport kind is inferred from the address when saving), while
/// `Ssh` keeps its own relay fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum HostTab {
    #[default]
    Default,
    Ssh,
}

impl HostTab {
    fn of_profile_kind(kind: HostKind) -> Self {
        match kind {
            HostKind::SshRelay => HostTab::Ssh,
            HostKind::Direct | HostKind::Tailscale | HostKind::Cloudflare => HostTab::Default,
        }
    }
}

/// Build a `HostProfile` for the Default tab by inferring the transport kind
/// from the address:
///
/// - `*.trycloudflare.com` → Cloudflare (Quick Tunnel hostname),
/// - `*.ts.net` / `100.x` tailnet → Tailscale,
/// - anything else → Direct.
pub(crate) fn build_default_host_profile(
    editing: Option<&HostProfile>,
    values: &HostFormValues,
    now: u64,
) -> Result<HostProfile, String> {
    let token = {
        let trimmed = values.token.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    };

    let address = normalize_daemon_address(&values.address).map_err(|e| e.to_string())?;
    // A Quick Tunnel URL carries no port, so probe the hostname before
    // requiring one — only Tailscale/Direct need `host:port`.
    let bare_host = host_without_port(&address);
    let probe = bare_host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_lowercase();
    let (kind, tailscale, cloudflare) = if probe.ends_with(".trycloudflare.com") {
        (
            HostKind::Cloudflare,
            None,
            Some(CloudflareHostConfig {
                hostname: bare_host.clone(),
                quick_tunnel: true,
            }),
        )
    } else {
        let (host, port) = split_host_port(&address)?;
        if TailscaleConfig::is_tailnet_host(
            &host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .to_lowercase(),
        ) {
            (
                HostKind::Tailscale,
                Some(TailscaleHostConfig {
                    magic_dns: host.clone(),
                    port,
                }),
                None,
            )
        } else {
            (HostKind::Direct, None, None)
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
        ssh: None,
        created_at: editing.map(|profile| profile.created_at).unwrap_or(now),
        updated_at: now,
        last_connected_at: editing.and_then(|profile| profile.last_connected_at),
    })
}

/// Tabs shown in the dialog's segmented control.
const TRANSPORT_OPTIONS: [HostTab; 2] = [HostTab::Default, HostTab::Ssh];

/// `tr!` needs a literal key, so the label is resolved through a match rather
/// than a dynamic lookup.
fn host_tab_label(tab: HostTab) -> String {
    match tab {
        HostTab::Default => tr!("host.transport_default"),
        HostTab::Ssh => tr!("host.transport_ssh"),
    }
}

impl Padu {
    pub(crate) fn request_host_dialog(
        &mut self,
        editing_profile_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.host_dialog_request = Some(HostDialogRequest {
            editing_profile_id,
            prefill: None,
        });
        cx.notify();
    }

    /// Open the host dialog with `address` and `token` already filled in.
    /// Kept for web parity and future credential-card shortcuts; the current
    /// card exposes manual details instead of an inline add button.
    #[allow(dead_code)]
    pub(crate) fn request_host_dialog_prefilled(
        &mut self,
        address: String,
        token: String,
        cx: &mut Context<Self>,
    ) {
        self.host_dialog_request = Some(HostDialogRequest {
            editing_profile_id: None,
            prefill: Some((address, token)),
        });
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

        // Prefill wins over existing when opening a fresh dialog from the
        // credential card — address and token come from the live daemon, not
        // from a saved profile.
        let (initial_name, initial_address, initial_token) =
            if let Some((addr, tok)) = &request.prefill {
                (String::new(), addr.clone(), tok.clone())
            } else {
                (
                    existing
                        .as_ref()
                        .map(|h| h.name.clone())
                        .unwrap_or_default(),
                    existing
                        .as_ref()
                        .map(|h| h.address.clone())
                        .unwrap_or_default(),
                    existing
                        .as_ref()
                        .and_then(|h| h.token.clone())
                        .unwrap_or_default(),
                )
            };

        let initial_tab = existing
            .as_ref()
            .map(|h| HostTab::of_profile_kind(h.kind))
            .unwrap_or_default();
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
        // When prefilled from the credential card, address and token are already
        // populated — focus the name field so the user can type a friendly label
        // and hit Enter to save. Otherwise focus the address field for normal
        // new-host flow, or the name field when editing.
        let first_focus = if request.prefill.is_some() || request.editing_profile_id.is_some() {
            name_focus
        } else {
            address_focus
        };

        let is_editing = request.editing_profile_id.is_some();
        self.host_dialog = Some(HostDialogState {
            editing_profile_id: request.editing_profile_id,
            tab: initial_tab,
            name_input,
            address_input,
            token_input,
            ssh_user_input,
            ssh_host_input,
            ssh_port_input,
            ssh_identity_input,
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
            // The dialog never provisions tunnels (that lives in Settings),
            // so this is always `None` here; kept for `build_host_profile`.
            cloudflare_address: None,
        }
    }

    pub(crate) fn close_host_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.host_dialog_request = None;
        if self.host_dialog.take().is_none() {
            return;
        }
        let focus = self.composer_focus(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    pub(crate) fn host_dialog_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &self.host_dialog else {
            return;
        };
        let tab = dialog.tab;
        let editing_id = dialog.editing_profile_id.clone();
        let editing = editing_id
            .as_ref()
            .and_then(|id| self.state.hosts.iter().find(|h| &h.id == id))
            .cloned();
        let values = self.host_form_values(cx);

        let built = match tab {
            HostTab::Default => build_default_host_profile(editing.as_ref(), &values, unix_time()),
            HostTab::Ssh => {
                build_host_profile(HostKind::SshRelay, editing.as_ref(), &values, unix_time())
            }
        };
        let profile = match built {
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

        self.close_host_dialog(window, cx);
        self.switch_to_host(Some(profile_id), cx);
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
        let (is_editing, tab, error_message) = {
            let dialog = self.host_dialog.as_ref()?;
            (
                dialog.editing_profile_id.is_some(),
                dialog.tab,
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

        let mut fields = div().flex().flex_col().gap(px(12.0)).child(labelled_field(
            tr!("host.name"),
            theme.text_secondary,
            text_field_box(&theme, false, name_input),
        ));

        // The Default tab is one address field for every transport: Direct,
        // Tailscale, and Cloudflare addresses are typed or pasted in, and
        // the kind is inferred from the address at save time. Tunnels are
        // provisioned from Settings, not from here.
        match tab {
            HostTab::Default => {
                fields = fields.child(labelled_field(
                    tr!("host.address"),
                    theme.text_secondary,
                    text_field_box(&theme, error_message.is_some(), address_input),
                ));
                fields = fields.child(hint_text(tr!("host.default_hint"), &theme));
                fields = fields.child(labelled_field(
                    tr!("host.token"),
                    theme.text_secondary,
                    text_field_box(&theme, false, token_input),
                ));
            }
            HostTab::Ssh => {
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
            .child(self.render_transport_selector(tab, &theme, cx))
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

    fn render_transport_selector(
        &self,
        active: HostTab,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        // Notes view-mode segmented style: one container, active option filled.
        let mut row = div()
            .flex()
            .items_center()
            .gap(px(1.0))
            .p(px(2.0))
            .rounded(px(7.0))
            .bg(theme.overlay);
        for tab in TRANSPORT_OPTIONS {
            let is_active = tab == active;
            let id = SharedString::from(format!("transport-option-{tab:?}"));
            row = row.child(
                div()
                    .id(id)
                    .tab_index(0)
                    .h(px(28.0))
                    .px(px(10.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_size(sp(12.5))
                    .text_color(if is_active {
                        theme.text
                    } else {
                        theme.text_secondary
                    })
                    .when(is_active, |el| el.bg(theme.overlay_strong))
                    .hover(|el| el.bg(theme.overlay_strong))
                    .focus_visible(|style| style.border_color(theme.accent))
                    .child(host_tab_label(tab))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(dialog) = this.host_dialog.as_mut() {
                            dialog.tab = tab;
                            dialog.error = None;
                        }
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if !event.keystroke.modifiers.modified()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            if let Some(dialog) = this.host_dialog.as_mut() {
                                dialog.tab = tab;
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
    fn default_tab_infers_direct_for_lan() {
        let mut v = values();
        v.address = "192.168.1.10:34123".into();
        let profile = build_default_host_profile(None, &v, 1).unwrap();
        assert_eq!(profile.kind, HostKind::Direct);
        assert_eq!(profile.address, "ws://192.168.1.10:34123");
        assert!(profile.tailscale.is_none());
        assert!(profile.cloudflare.is_none());
    }

    #[test]
    fn default_tab_infers_tailscale_for_magic_dns() {
        let mut v = values();
        v.address = "wss://mac.tail-abc.ts.net:34123".into();
        let profile = build_default_host_profile(None, &v, 1).unwrap();
        assert_eq!(profile.kind, HostKind::Tailscale);
        let tailscale = profile.tailscale.expect("tailscale block");
        assert_eq!(tailscale.magic_dns, "mac.tail-abc.ts.net");
        assert_eq!(tailscale.port, 34123);
    }

    #[test]
    fn default_tab_infers_cloudflare_for_quick_tunnel() {
        let mut v = values();
        v.address = "wss://random-words.trycloudflare.com".into();
        let profile = build_default_host_profile(None, &v, 1).unwrap();
        assert_eq!(profile.kind, HostKind::Cloudflare);
        assert_eq!(profile.address, "wss://random-words.trycloudflare.com");
        let cloudflare = profile.cloudflare.expect("cloudflare block");
        assert_eq!(cloudflare.hostname, "random-words.trycloudflare.com");
        assert!(cloudflare.quick_tunnel);
    }

    #[test]
    fn host_tab_maps_profile_kinds() {
        assert_eq!(HostTab::of_profile_kind(HostKind::Direct), HostTab::Default);
        assert_eq!(
            HostTab::of_profile_kind(HostKind::Tailscale),
            HostTab::Default
        );
        assert_eq!(
            HostTab::of_profile_kind(HostKind::Cloudflare),
            HostTab::Default
        );
        assert_eq!(HostTab::of_profile_kind(HostKind::SshRelay), HostTab::Ssh);
    }

    #[test]
    fn build_tailscale_profile_requires_a_port() {
        let mut v = values();
        v.address = "wss://mac.tail-abc.ts.net".into();
        let error = build_host_profile(HostKind::Tailscale, None, &v, 1).unwrap_err();
        assert!(error.contains("port"), "unexpected error: {error}");
    }

    #[test]
    fn build_tailscale_profile_rejects_lan_address() {
        let mut v = values();
        v.address = "ws://192.168.1.10:34123".into();
        let error = build_host_profile(HostKind::Tailscale, None, &v, 1).unwrap_err();
        assert!(
            error.contains("Direct"),
            "expected Direct hint, got: {error}"
        );
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
