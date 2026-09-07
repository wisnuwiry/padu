//! Modal editor for creating and editing remote daemon host profiles.

use gpui::{KeyBinding, actions};

use padu_client::persistence::{HostProfile, normalize_daemon_address};

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
}

pub(crate) struct HostDialogState {
    pub editing_profile_id: Option<String>,
    pub name_input: Entity<TextInput>,
    pub address_input: Entity<TextInput>,
    pub token_input: Entity<TextInput>,
    pub error: Option<String>,
    pub save_focus: FocusHandle,
    pub cancel_focus: FocusHandle,
    pub delete_focus: Option<FocusHandle>,
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
            name_input,
            address_input,
            token_input,
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

        let raw_name = dialog.name_input.read(cx).content().trim().to_string();
        let raw_address = dialog.address_input.read(cx).content().trim().to_string();
        let raw_token = dialog.token_input.read(cx).content().trim().to_string();
        let editing_id = dialog.editing_profile_id.clone();

        let normalized_address = match normalize_daemon_address(&raw_address) {
            Ok(addr) => addr,
            Err(error) => {
                if let Some(dialog) = self.host_dialog.as_mut() {
                    dialog.error = Some(error.to_string());
                }
                cx.notify();
                return;
            }
        };

        let token = if raw_token.is_empty() {
            None
        } else {
            Some(raw_token)
        };

        let profile_id = if let Some(id) = editing_id {
            if let Some(existing) = self.state.hosts.iter_mut().find(|h| h.id == id) {
                existing.name = if raw_name.is_empty() {
                    padu_client::persistence::display_host(&normalized_address)
                } else {
                    raw_name
                };
                existing.address = normalized_address;
                existing.token = token;
                existing.updated_at = unix_time();
            }
            id
        } else {
            let name = if raw_name.is_empty() {
                padu_client::persistence::display_host(&normalized_address)
            } else {
                raw_name
            };
            let now = unix_time();
            let new_profile = HostProfile {
                id: Uuid::new_v4().to_string(),
                name,
                address: normalized_address,
                token,
                created_at: now,
                updated_at: now,
                last_connected_at: None,
            };
            let id = new_profile.id.clone();
            self.state.add_host_profile(new_profile);
            id
        };

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
        let dialog = self.host_dialog.as_ref()?;
        let theme = Theme::current(cx);
        let is_editing = dialog.editing_profile_id.is_some();
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

        let name_input = dialog.name_input.clone();
        let address_input = dialog.address_input.clone();
        let token_input = dialog.token_input.clone();
        let error_message = dialog.error.clone();

        let save_focus = dialog.save_focus.clone();
        let cancel_focus = dialog.cancel_focus.clone();
        let delete_focus = dialog.delete_focus.clone();

        let card = div()
            .key_context(DIALOG_CONTEXT)
            .on_action(cx.listener(|this, _: &ConfirmHostDialog, window, cx| {
                this.host_dialog_save(window, cx);
            }))
            .on_action(cx.listener(|this, _: &DismissHostDialog, window, cx| {
                this.close_host_dialog(window, cx);
            }))
            .id("host-dialog-card")
            .w(px(460.0))
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
            // Fields
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    // Name Field
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(sp(12.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text_secondary)
                                    .child(tr!("host.name")),
                            )
                            .child(
                                div()
                                    .h(px(32.0))
                                    .px(px(10.0))
                                    .rounded(px(7.0))
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.surface)
                                    .flex()
                                    .items_center()
                                    .child(name_input),
                            ),
                    )
                    // Address Field
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(sp(12.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text_secondary)
                                    .child(tr!("host.address")),
                            )
                            .child(
                                div()
                                    .h(px(32.0))
                                    .px(px(10.0))
                                    .rounded(px(7.0))
                                    .border_1()
                                    .border_color(if error_message.is_some() {
                                        theme.accent
                                    } else {
                                        theme.border
                                    })
                                    .bg(theme.surface)
                                    .flex()
                                    .items_center()
                                    .child(address_input),
                            )
                            .when_some(error_message, |el, error| {
                                el.child(
                                    div()
                                        .text_size(sp(11.5))
                                        .text_color(gpui::hsla(0.0, 0.7, 0.55, 1.0))
                                        .child(error),
                                )
                            }),
                    )
                    // Token Field
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(sp(12.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text_secondary)
                                    .child(tr!("host.token")),
                            )
                            .child(
                                div()
                                    .h(px(32.0))
                                    .px(px(10.0))
                                    .rounded(px(7.0))
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.surface)
                                    .flex()
                                    .items_center()
                                    .child(token_input),
                            ),
                    ),
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
}
