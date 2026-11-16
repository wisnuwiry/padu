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
        KeyBinding::new("secondary-enter", ConfirmDialogAction, Some(DIALOG_CONTEXT)),
        KeyBinding::new("escape", DismissDialogAction, Some(DIALOG_CONTEXT)),
    ]);
}

pub(crate) enum ConfirmAction {
    DeleteSession {
        session_id: Uuid,
    },
    DeletePath {
        path: PathBuf,
    },
    DeleteNote {
        note_id: Uuid,
    },
    DeleteTask {
        task_id: Uuid,
        expected_version: u64,
    },
    DeleteHost {
        profile_id: String,
    },
    DeleteProjectScript {
        project_id: Uuid,
        script_id: String,
    },
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
    #[allow(clippy::too_many_arguments)]
    fn open_confirm_dialog(
        &mut self,
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
        confirm_label: impl Into<SharedString>,
        cancel_label: impl Into<SharedString>,
        variant: ConfirmVariant,
        icon_name: Option<&'static str>,
        action: ConfirmAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous_focus = window.focused(cx);
        let cancel_focus = cx.focus_handle();
        let confirm_focus = cx.focus_handle();
        let focus_target = confirm_focus.clone();
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| window.focus(&focus_target, cx));
        });

        self.confirm_dialog = Some(ConfirmDialogState {
            title: title.into(),
            message: message.into(),
            confirm_label: confirm_label.into(),
            cancel_label: cancel_label.into(),
            variant,
            icon_name,
            action,
            cancel_focus,
            confirm_focus,
            previous_focus,
        });
        cx.notify();
    }

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
        self.open_confirm_dialog(
            tr!("session.delete_title"),
            tr!("session.delete_message", title = title),
            tr!("session.delete_confirm"),
            tr!("common.cancel"),
            ConfirmVariant::Danger,
            Some("icons/trash.svg"),
            ConfirmAction::DeleteSession { session_id },
            window,
            cx,
        );
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
        self.open_confirm_dialog(
            tr!("notes.delete_title"),
            tr!("notes.delete_message", title = title),
            tr!("notes.delete"),
            tr!("common.cancel"),
            ConfirmVariant::Danger,
            Some("icons/trash.svg"),
            ConfirmAction::DeleteNote { note_id },
            window,
            cx,
        );
    }

    pub(crate) fn confirm_delete_task(
        &mut self,
        task_id: Uuid,
        title: String,
        expected_version: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_confirm_dialog(
            tr!("board.delete_task"),
            format!("{}\n\"{}\"", tr!("board.delete_task_confirm"), title),
            tr!("board.delete_task"),
            tr!("board.cancel"),
            ConfirmVariant::Danger,
            Some("icons/trash.svg"),
            ConfirmAction::DeleteTask {
                task_id,
                expected_version,
            },
            window,
            cx,
        );
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
        self.open_confirm_dialog(
            tr!("files.delete"),
            tr!("files.delete_confirm", name = name),
            tr!("files.delete"),
            tr!("common.cancel"),
            ConfirmVariant::Danger,
            Some("icons/trash.svg"),
            ConfirmAction::DeletePath { path },
            window,
            cx,
        );
    }

    pub(crate) fn confirm_reinstall_agy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_confirm_dialog(
            tr!("providers.agy_reinstall_title"),
            tr!("providers.agy_reinstall_confirm"),
            tr!("providers.agy_reinstall"),
            tr!("common.cancel"),
            ConfirmVariant::Danger,
            Some("icons/trash.svg"),
            ConfirmAction::ReinstallAgy,
            window,
            cx,
        );
    }

    pub(crate) fn confirm_sign_out_agy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_confirm_dialog(
            tr!("providers.agy_sign_out_title"),
            tr!("providers.agy_sign_out_confirm"),
            tr!("providers.agy_sign_out"),
            tr!("common.cancel"),
            ConfirmVariant::Danger,
            Some("icons/trash.svg"),
            ConfirmAction::SignOutAgy,
            window,
            cx,
        );
    }

    pub(crate) fn confirm_remove_agy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_confirm_dialog(
            tr!("providers.agy_remove_title"),
            tr!("providers.agy_remove_confirm"),
            tr!("providers.agy_remove_download"),
            tr!("common.cancel"),
            ConfirmVariant::Danger,
            Some("icons/trash.svg"),
            ConfirmAction::RemoveAgy,
            window,
            cx,
        );
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

        self.open_confirm_dialog(
            tr!("host.delete_confirm_title"),
            tr!("host.delete_confirm_message", name = host_name),
            tr!("host.remove_host"),
            tr!("common.cancel"),
            ConfirmVariant::Danger,
            Some("icons/trash.svg"),
            ConfirmAction::DeleteHost { profile_id },
            window,
            cx,
        );
    }

    pub(crate) fn confirm_delete_project_script(
        &mut self,
        project_id: Uuid,
        script_id: String,
        script_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_confirm_dialog(
            tr!("actions.delete_confirm_title", name = &script_name),
            tr!("actions.delete_confirm_description"),
            tr!("actions.delete_confirm_button"),
            tr!("actions.cancel"),
            ConfirmVariant::Danger,
            Some("icons/trash.svg"),
            ConfirmAction::DeleteProjectScript {
                project_id,
                script_id,
            },
            window,
            cx,
        );
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
            ConfirmAction::DeleteTask {
                task_id,
                expected_version,
            } => {
                self.delete_board_task(task_id, expected_version, cx);
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
            ConfirmAction::DeleteProjectScript {
                project_id,
                script_id,
            } => {
                self.delete_project_script(project_id, &script_id, cx);
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
            &theme,
            Some(&dialog.confirm_focus),
            Some(&dialog.cancel_focus),
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

    #[gpui::test]
    fn confirm_dialog_card_keyboard_shortcuts(cx: &mut gpui::TestAppContext) {
        use gpui::{IntoElement, Render};
        use std::sync::atomic::{AtomicBool, Ordering};

        static CONFIRMED: AtomicBool = AtomicBool::new(false);
        static CANCELLED: AtomicBool = AtomicBool::new(false);

        CONFIRMED.store(false, Ordering::SeqCst);
        CANCELLED.store(false, Ordering::SeqCst);

        cx.update(|cx| {
            let _ = bezel::ui::register_fonts(cx);
            bezel::theme::appearance::init(bezel::theme::appearance::AppearanceMode::Dark, cx);
            bezel::ui::focus::init(cx);
            init(cx);
        });

        struct DialogHarness {
            confirm_focus: FocusHandle,
            cancel_focus: FocusHandle,
        }

        impl Render for DialogHarness {
            fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let theme = Theme::current(cx);
                render_confirm_dialog_card(
                    "confirm-card",
                    "Title".into(),
                    "Message".into(),
                    "Confirm".into(),
                    "Cancel".into(),
                    ConfirmVariant::Danger,
                    None,
                    &theme,
                    Some(&self.confirm_focus),
                    Some(&self.cancel_focus),
                    cx,
                    |_, _, _| CONFIRMED.store(true, Ordering::SeqCst),
                    |_, _, _| CANCELLED.store(true, Ordering::SeqCst),
                )
                .key_context(DIALOG_CONTEXT)
                .on_action(cx.listener(|_, _: &ConfirmDialogAction, _, _| {
                    CONFIRMED.store(true, Ordering::SeqCst);
                }))
                .on_action(cx.listener(|_, _: &DismissDialogAction, _, _| {
                    CANCELLED.store(true, Ordering::SeqCst);
                }))
            }
        }

        let confirm_focus = cx.update(|cx| cx.focus_handle());
        let cancel_focus = cx.update(|cx| cx.focus_handle());

        let harness = DialogHarness {
            confirm_focus: confirm_focus.clone(),
            cancel_focus: cancel_focus.clone(),
        };

        let (_view, cx) = cx.add_window_view(|_, _| harness);
        cx.run_until_parked();

        // Focus confirm button and press escape
        cx.update(|window, cx| window.focus(&confirm_focus, cx));
        cx.simulate_keystrokes("escape");
        assert!(
            CANCELLED.load(Ordering::SeqCst),
            "Escape on confirm dialog should cancel"
        );

        cx.run_until_parked();

        // Focus confirm button and press enter
        cx.update(|window, cx| window.focus(&confirm_focus, cx));
        cx.simulate_keystrokes("enter");
        assert!(
            CONFIRMED.load(Ordering::SeqCst),
            "Enter on confirm button should confirm"
        );

        cx.run_until_parked();
        CANCELLED.store(false, Ordering::SeqCst);

        // Focus cancel button and press enter
        cx.update(|window, cx| window.focus(&cancel_focus, cx));
        cx.simulate_keystrokes("enter");
        assert!(
            CANCELLED.load(Ordering::SeqCst),
            "Enter on cancel button should cancel"
        );
    }
}
