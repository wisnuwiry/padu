//! Unified confirmation dialog component and state engine.

use std::path::PathBuf;

use gpui::{
    AnyElement, App, Context, FocusHandle, InteractiveElement, KeyBinding, SharedString, Window,
    actions,
};
use uuid::Uuid;

use crate::app::Padu;
use crate::theme::Theme;
use crate::ui::dialog::{ConfirmVariant, dialog_backdrop, render_confirm_dialog_card};

actions!(
    padu_confirm_dialog,
    [ConfirmDialogAction, DismissDialogAction]
);

const DIALOG_CONTEXT: &str = "ConfirmDialog";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", ConfirmDialogAction, Some(DIALOG_CONTEXT)),
        KeyBinding::new("escape", DismissDialogAction, Some(DIALOG_CONTEXT)),
    ]);
}

pub(crate) enum ConfirmAction {
    DeleteSession { session_id: Uuid },
    DeletePath { path: PathBuf },
    DeleteNote { note_id: Uuid },
    DeleteHost { profile_id: String },
    ReinstallAgy,
    RemoveAgy,
    SignOutAgy,
}

pub(crate) struct ConfirmDialogState {
    pub title: SharedString,
    pub message: SharedString,
    pub confirm_label: SharedString,
    pub cancel_label: SharedString,
    pub variant: ConfirmVariant,
    pub icon_name: Option<&'static str>,
    pub action: ConfirmAction,
    pub cancel_focus: FocusHandle,
    pub confirm_focus: FocusHandle,
    pub previous_focus: Option<FocusHandle>,
}

impl Padu {
    pub(crate) fn confirm_delete_session(
        &mut self,
        session_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.state.sessions.iter().find(|s| s.id == session_id) else {
            return;
        };
        let title = session.display_title().to_owned();
        let previous_focus = window.focused(cx);
        let cancel_focus = cx.focus_handle();
        let confirm_focus = cx.focus_handle();
        let focus_target = confirm_focus.clone();
        window.on_next_frame(move |window, cx| window.focus(&focus_target, cx));

        self.confirm_dialog = Some(ConfirmDialogState {
            title: tr!("session.delete_title").into(),
            message: tr!("session.delete_message", title = title).into(),
            confirm_label: tr!("session.delete_confirm").into(),
            cancel_label: tr!("common.cancel").into(),
            variant: ConfirmVariant::Danger,
            icon_name: Some("icons/trash.svg"),
            action: ConfirmAction::DeleteSession { session_id },
            cancel_focus,
            confirm_focus,
            previous_focus,
        });
        cx.notify();
    }

    pub(crate) fn confirm_delete_note(
        &mut self,
        note_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(note) = self.notes.iter().find(|note| note.id == note_id) else {
            return;
        };
        let title = if note.title.is_empty() {
            tr!("notes.untitled")
        } else {
            note.title.clone()
        };
        let previous_focus = window.focused(cx);
        let cancel_focus = cx.focus_handle();
        let confirm_focus = cx.focus_handle();
        let focus_target = confirm_focus.clone();
        window.on_next_frame(move |window, cx| window.focus(&focus_target, cx));

        self.confirm_dialog = Some(ConfirmDialogState {
            title: tr!("notes.delete_title").into(),
            message: tr!("notes.delete_message", title = title).into(),
            confirm_label: tr!("notes.delete").into(),
            cancel_label: tr!("common.cancel").into(),
            variant: ConfirmVariant::Danger,
            icon_name: Some("icons/trash.svg"),
            action: ConfirmAction::DeleteNote { note_id },
            cancel_focus,
            confirm_focus,
            previous_focus,
        });
        cx.notify();
    }

    pub(crate) fn confirm_delete_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        let previous_focus = window.focused(cx);
        let cancel_focus = cx.focus_handle();
        let confirm_focus = cx.focus_handle();
        let focus_target = confirm_focus.clone();
        window.on_next_frame(move |window, cx| window.focus(&focus_target, cx));

        self.confirm_dialog = Some(ConfirmDialogState {
            title: tr!("files.delete").into(),
            message: tr!("files.delete_confirm", name = name).into(),
            confirm_label: tr!("files.delete").into(),
            cancel_label: tr!("common.cancel").into(),
            variant: ConfirmVariant::Danger,
            icon_name: Some("icons/trash.svg"),
            action: ConfirmAction::DeletePath { path },
            cancel_focus,
            confirm_focus,
            previous_focus,
        });
        cx.notify();
    }

    pub(crate) fn confirm_reinstall_agy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_agy_confirmation(
            tr!("providers.agy_reinstall_title"),
            tr!("providers.agy_reinstall_confirm"),
            tr!("providers.agy_reinstall"),
            ConfirmAction::ReinstallAgy,
            window,
            cx,
        );
    }

    pub(crate) fn confirm_sign_out_agy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_agy_confirmation(
            tr!("providers.agy_sign_out_title"),
            tr!("providers.agy_sign_out_confirm"),
            tr!("providers.agy_sign_out"),
            ConfirmAction::SignOutAgy,
            window,
            cx,
        );
    }

    pub(crate) fn confirm_remove_agy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_agy_confirmation(
            tr!("providers.agy_remove_title"),
            tr!("providers.agy_remove_confirm"),
            tr!("providers.agy_remove_download"),
            ConfirmAction::RemoveAgy,
            window,
            cx,
        );
    }

    fn open_agy_confirmation(
        &mut self,
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
        confirm_label: impl Into<SharedString>,
        action: ConfirmAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous_focus = window.focused(cx);
        let cancel_focus = cx.focus_handle();
        let confirm_focus = cx.focus_handle();
        let focus_target = confirm_focus.clone();
        window.on_next_frame(move |window, cx| window.focus(&focus_target, cx));
        self.confirm_dialog = Some(ConfirmDialogState {
            title: title.into(),
            message: message.into(),
            confirm_label: confirm_label.into(),
            cancel_label: tr!("common.cancel").into(),
            variant: ConfirmVariant::Danger,
            icon_name: Some("icons/trash.svg"),
            action,
            cancel_focus,
            confirm_focus,
            previous_focus,
        });
        cx.notify();
    }

    pub(crate) fn confirm_delete_host(
        &mut self,
        profile_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let host_name = self
            .state
            .hosts
            .iter()
            .find(|h| h.id == profile_id)
            .map(|h| {
                if !h.name.trim().is_empty() {
                    h.name.clone()
                } else {
                    h.address.clone()
                }
            })
            .unwrap_or_default();

        let previous_focus = window.focused(cx);
        let cancel_focus = cx.focus_handle();
        let confirm_focus = cx.focus_handle();
        let focus_target = confirm_focus.clone();
        window.on_next_frame(move |window, cx| window.focus(&focus_target, cx));

        self.confirm_dialog = Some(ConfirmDialogState {
            title: tr!("host.delete_confirm_title").into(),
            message: tr!("host.delete_confirm_message", name = host_name).into(),
            confirm_label: tr!("host.remove_host").into(),
            cancel_label: tr!("common.cancel").into(),
            variant: ConfirmVariant::Danger,
            icon_name: Some("icons/trash.svg"),
            action: ConfirmAction::DeleteHost { profile_id },
            cancel_focus,
            confirm_focus,
            previous_focus,
        });
        cx.notify();
    }

    pub(crate) fn close_confirm_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(dialog) = self.confirm_dialog.take() {
            if let Some(prev) = dialog.previous_focus {
                window.focus(&prev, cx);
            } else {
                let focus = self.composer_focus(cx);
                window.focus(&focus, cx);
            }
            cx.notify();
        }
    }

    pub(crate) fn execute_confirm_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.confirm_dialog.take() else {
            return;
        };

        match dialog.action {
            ConfirmAction::DeleteSession { session_id } => {
                self.remove_session(session_id, cx);
            }
            ConfirmAction::DeletePath { path } => {
                self.execute_delete_path(path, cx);
            }
            ConfirmAction::DeleteNote { note_id } => {
                if let Some(index) = self.notes.iter().position(|note| note.id == note_id) {
                    self.delete_note_at(index, cx);
                }
            }
            ConfirmAction::ReinstallAgy => {
                self.install_agy_acp(cx);
            }
            ConfirmAction::RemoveAgy => {
                self.remove_agy_download(cx);
            }
            ConfirmAction::SignOutAgy => {
                self.logout_agy(cx);
            }
            ConfirmAction::DeleteHost { profile_id } => {
                let was_active = self.state.active_host_id.as_deref() == Some(&profile_id);
                self.state.remove_host_profile(&profile_id);
                let _ = self.store.write_app_settings(&self.state.app_settings());
                self.host_dialog = None;
                if was_active {
                    self.switch_to_host(None, cx);
                }
            }
        }

        if let Some(prev) = dialog.previous_focus {
            window.focus(&prev, cx);
        } else {
            let focus = self.composer_focus(cx);
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    pub(crate) fn render_confirm_dialog(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let dialog = self.confirm_dialog.as_ref()?;
        let theme = Theme::current(cx);

        let card = render_confirm_dialog_card(
            "confirm-dialog-card",
            dialog.title.clone(),
            dialog.message.clone(),
            dialog.confirm_label.clone(),
            dialog.cancel_label.clone(),
            dialog.variant,
            dialog.icon_name,
            &dialog.cancel_focus,
            &dialog.confirm_focus,
            &theme,
            cx,
            |padu, window, cx| padu.execute_confirm_dialog(window, cx),
            |padu, window, cx| padu.close_confirm_dialog(window, cx),
        )
        .key_context(DIALOG_CONTEXT)
        .on_action(cx.listener(|padu, _: &ConfirmDialogAction, window, cx| {
            padu.execute_confirm_dialog(window, cx);
        }))
        .on_action(cx.listener(|padu, _: &DismissDialogAction, window, cx| {
            padu.close_confirm_dialog(window, cx);
        }));

        Some(dialog_backdrop(
            "confirm-dialog-layer",
            &theme,
            cx,
            |padu, window, cx| padu.close_confirm_dialog(window, cx),
            card,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn confirm_dialog_state_manages_focus_and_target(cx: &mut App) {
        let session_id = Uuid::new_v4();
        let cancel_focus = cx.focus_handle();
        let confirm_focus = cx.focus_handle();
        let state = ConfirmDialogState {
            title: "Delete Session".into(),
            message: "Are you sure?".into(),
            confirm_label: "Delete".into(),
            cancel_label: "Cancel".into(),
            variant: ConfirmVariant::Danger,
            icon_name: Some("icons/trash.svg"),
            action: ConfirmAction::DeleteSession { session_id },
            cancel_focus,
            confirm_focus,
            previous_focus: None,
        };
        assert_eq!(state.title, "Delete Session");
        assert_eq!(state.variant, ConfirmVariant::Danger);
    }
}
