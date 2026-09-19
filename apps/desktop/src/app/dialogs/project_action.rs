//! Modal editor for project-scoped actions/scripts.

use gpui::{ElementId, KeyBinding, KeyDownEvent, actions};
use uuid::Uuid;

use crate::app::*;
use crate::model::{ProjectScript, ProjectScriptIcon};
use crate::ui::dialog::dialog_backdrop;
use crate::ui::toggle_switch;

actions!(
    padu_project_action_dialog,
    [ConfirmProjectActionDialog, DismissProjectActionDialog]
);

use crate::ui::shortcut_recorder::{KeyDownResult, ShortcutRecorderState};

const DIALOG_CONTEXT: &str = "ProjectActionDialog";
const DIALOG_INPUT_CONTEXT: &str = "ProjectActionDialog > TextInput";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(
            "secondary-enter",
            ConfirmProjectActionDialog,
            Some(DIALOG_INPUT_CONTEXT),
        ),
        KeyBinding::new(
            "secondary-enter",
            ConfirmProjectActionDialog,
            Some(DIALOG_CONTEXT),
        ),
        KeyBinding::new("escape", DismissProjectActionDialog, Some(DIALOG_CONTEXT)),
    ]);
}

pub(crate) struct ProjectActionDialogRequest {
    pub project_id: Uuid,
    pub script: Option<ProjectScript>,
}

pub(crate) struct ProjectActionDialogState {
    pub project_id: Uuid,
    pub script_id: Option<String>,
    pub name_input: Entity<TextInput>,
    pub command_input: Entity<TextInput>,
    pub keybinding_input: Entity<TextInput>,
    pub preview_url_input: Entity<TextInput>,
    pub recorder: ShortcutRecorderState,
    pub icon: ProjectScriptIcon,
    pub icon_picker_open: bool,
    pub run_on_worktree_create: bool,
    pub wait_for_setup: bool,
    pub auto_open_preview: bool,
    pub validation_error: Option<String>,
    pub save_focus: FocusHandle,
    pub cancel_focus: FocusHandle,
    pub delete_focus: Option<FocusHandle>,
}

impl Padu {
    pub(crate) fn request_project_action_dialog(
        &mut self,
        project_id: Uuid,
        script: Option<ProjectScript>,
        cx: &mut Context<Self>,
    ) {
        self.project_action_dialog_request =
            Some(ProjectActionDialogRequest { project_id, script });
        cx.notify();
    }

    fn materialize_project_action_dialog(
        &mut self,
        request: ProjectActionDialogRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing = request.script;
        let script_id = existing.as_ref().map(|s| s.id.clone());
        let initial_name = existing
            .as_ref()
            .map(|s| s.name.clone())
            .unwrap_or_default();
        let initial_command = existing
            .as_ref()
            .map(|s| s.command.clone())
            .unwrap_or_default();
        let initial_kb = existing
            .as_ref()
            .and_then(|s| s.keybinding.clone())
            .unwrap_or_default();
        let initial_preview_url = existing
            .as_ref()
            .and_then(|s| s.preview_url.clone())
            .unwrap_or_default();
        let icon = existing
            .as_ref()
            .map(|s| s.icon)
            .unwrap_or(ProjectScriptIcon::Play);
        let run_on_worktree_create = existing
            .as_ref()
            .map(|s| s.run_on_worktree_create)
            .unwrap_or(false);
        let wait_for_setup = existing
            .as_ref()
            .map(|s| !s.async_run.unwrap_or(false))
            .unwrap_or(false);
        let auto_open_preview = existing
            .as_ref()
            .map(|s| s.auto_open_preview)
            .unwrap_or(false);

        let name_input = cx.new(|cx| {
            let mut input = TextInput::new(window, cx).placeholder(tr!("actions.name_placeholder"));
            if !initial_name.is_empty() {
                input.set_content(initial_name, cx);
            }
            input
        });

        let command_input = cx.new(|cx| {
            let mut input = TextInput::new(window, cx)
                .multi_line()
                .placeholder(tr!("actions.command_placeholder"));
            if !initial_command.is_empty() {
                input.set_content(initial_command, cx);
            }
            input
        });

        let keybinding_input = cx.new(|cx| {
            let mut input =
                TextInput::new(window, cx).placeholder(tr!("actions.keybinding_placeholder"));
            if !initial_kb.is_empty() {
                input.set_content(initial_kb, cx);
            }
            input
        });

        let preview_url_input = cx.new(|cx| {
            let mut input =
                TextInput::new(window, cx).placeholder(tr!("actions.preview_url_placeholder"));
            if !initial_preview_url.is_empty() {
                input.set_content(initial_preview_url, cx);
            }
            input
        });

        let delete_focus = if script_id.is_some() {
            Some(cx.focus_handle())
        } else {
            None
        };

        let name_focus = name_input.read(cx).focus();
        let shortcut_focus = cx.focus_handle();
        let recorder = ShortcutRecorderState::new(shortcut_focus);
        self.project_action_dialog = Some(ProjectActionDialogState {
            project_id: request.project_id,
            script_id,
            name_input,
            command_input,
            keybinding_input,
            preview_url_input,
            recorder,
            icon,
            icon_picker_open: false,
            run_on_worktree_create,
            wait_for_setup,
            auto_open_preview,
            validation_error: None,
            save_focus: cx.focus_handle(),
            cancel_focus: cx.focus_handle(),
            delete_focus,
        });

        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| window.focus(&name_focus, cx));
        });
        cx.notify();
    }

    pub(crate) fn close_project_action_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.project_action_dialog_request = None;
        if self.project_action_dialog.take().is_none() {
            return;
        }
        let focus = self.composer_focus(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    pub(crate) fn project_action_dialog_save(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.project_action_dialog.as_ref() else {
            return;
        };

        let name = dialog.name_input.read(cx).content().trim().to_owned();
        if name.is_empty() {
            if let Some(dialog) = self.project_action_dialog.as_mut() {
                dialog.validation_error = Some(tr!("actions.name_required"));
            }
            cx.notify();
            return;
        }

        let command = dialog.command_input.read(cx).content().trim().to_owned();
        if command.is_empty() {
            if let Some(dialog) = self.project_action_dialog.as_mut() {
                dialog.validation_error = Some(tr!("actions.command_required"));
            }
            cx.notify();
            return;
        }

        let keybinding = {
            let kb = dialog.keybinding_input.read(cx).content().trim().to_owned();
            if kb.is_empty() { None } else { Some(kb) }
        };

        let preview_url = {
            let url = dialog
                .preview_url_input
                .read(cx)
                .content()
                .trim()
                .to_owned();
            if url.is_empty() { None } else { Some(url) }
        };

        let project_id = dialog.project_id;
        let script_id = dialog.script_id.clone().unwrap_or_else(|| slug_id(&name));
        let is_editing = dialog.script_id.is_some();

        let script = ProjectScript {
            id: script_id.clone(),
            name,
            command,
            icon: dialog.icon,
            run_on_worktree_create: dialog.run_on_worktree_create,
            async_run: if dialog.run_on_worktree_create {
                Some(!dialog.wait_for_setup)
            } else {
                None
            },
            preview_url,
            auto_open_preview: dialog.auto_open_preview,
            keybinding,
        };

        if is_editing {
            self.update_project_script(project_id, &script_id, script, cx);
        } else {
            self.add_project_script(project_id, script, cx);
        }

        self.close_project_action_dialog(window, cx);
    }

    pub(crate) fn project_action_dialog_delete(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.project_action_dialog.as_ref() else {
            return;
        };
        let Some(script_id) = dialog.script_id.clone() else {
            return;
        };
        let project_id = dialog.project_id;
        let script_name = dialog.name_input.read(cx).content().trim().to_owned();

        self.close_project_action_dialog(window, cx);
        self.confirm_delete_project_script(project_id, script_id, script_name, window, cx);
    }

    pub(crate) fn render_project_action_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if let Some(request) = self.project_action_dialog_request.take() {
            self.materialize_project_action_dialog(request, window, cx);
        }
        let dialog = self.project_action_dialog.as_ref()?;
        let theme = Theme::current(cx);
        let is_editing = dialog.script_id.is_some();
        let title = if is_editing {
            tr!("actions.edit_title")
        } else {
            tr!("actions.add_title")
        };
        let save_label = if is_editing {
            tr!("actions.save_changes")
        } else {
            tr!("actions.save_action")
        };

        let name_input = dialog.name_input.clone();
        let command_input = dialog.command_input.clone();
        let keybinding_input = dialog.keybinding_input.clone();
        let preview_url_input = dialog.preview_url_input.clone();
        let current_icon = dialog.icon;
        let icon_picker_open = dialog.icon_picker_open;
        let run_on_worktree_create = dialog.run_on_worktree_create;
        let wait_for_setup = dialog.wait_for_setup;
        let auto_open_preview = dialog.auto_open_preview;
        let validation_error = dialog.validation_error.clone();

        let save_focus = dialog.save_focus.clone();
        let cancel_focus = dialog.cancel_focus.clone();
        let delete_focus = dialog.delete_focus.clone();

        let is_recording_keybinding = dialog.recorder.is_recording;
        let shortcut_focus = dialog.recorder.focus.clone();
        let is_shortcut_focused = shortcut_focus.is_focused(window);
        let is_recording = is_recording_keybinding || is_shortcut_focused;
        let current_shortcut = keybinding_input.read(cx).content().trim().to_string();
        let live_recording_preview = dialog.recorder.live_preview.clone();

        let has_preview_url = !preview_url_input.read(cx).content().trim().is_empty();

        let card = div()
            .key_context(DIALOG_CONTEXT)
            .on_action(
                cx.listener(|this, _: &ConfirmProjectActionDialog, window, cx| {
                    this.project_action_dialog_save(window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &DismissProjectActionDialog, window, cx| {
                    this.close_project_action_dialog(window, cx);
                }),
            )
            .id("project-action-dialog-card")
            .w(px(520.0))
            .max_h(px(680.0))
            .rounded(px(14.0))
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.raised)
            .shadow_lg()
            .overflow_hidden()
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            // Header (Pinned)
            .child(
                div()
                    .px(px(20.0))
                    .pt(px(18.0))
                    .pb(px(14.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .size(px(32.0))
                                    .rounded(px(8.0))
                                    .bg(theme.surface)
                                    .border_1()
                                    .border_color(theme.border)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon(current_icon.icon_path(), 16.0, theme.text)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_size(sp(15.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.text)
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .text_size(sp(12.0))
                                            .text_color(theme.text_secondary)
                                            .child(tr!("actions.description")),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("close-project-action-dialog")
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
                                this.close_project_action_dialog(window, cx);
                            })),
                    ),
            )
            // Scrollable Content Body
            .child(
                div()
                    .id("project-action-dialog-scroll-body")
                    .overflow_y_scroll()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    .px(px(20.0))
                    .py(px(16.0))
                    // Validation Alert Banner
                    .when_some(validation_error, |parent, err| {
                        parent.child(
                            div()
                                .px(px(12.0))
                                .py(px(8.0))
                                .rounded(px(8.0))
                                .border_1()
                                .border_color(theme.danger)
                                .bg(theme.surface)
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(icon("icons/alert.svg", 14.0, theme.danger))
                                .child(
                                    div()
                                        .text_size(sp(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.danger)
                                        .child(err),
                                ),
                        )
                    })
                    // Field 1: Action Name & Icon Selector
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(sp(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text_secondary)
                                    .child(tr!("actions.name")),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    // Icon selector trigger
                                    .child(
                                        div()
                                            .id("project-action-icon-button")
                                            .h(px(34.0))
                                            .px(px(10.0))
                                            .rounded(px(7.0))
                                            .border_1()
                                            .border_color(if icon_picker_open {
                                                theme.accent
                                            } else {
                                                theme.border
                                            })
                                            .bg(theme.surface)
                                            .hover(|e| e.bg(theme.overlay))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(icon(current_icon.icon_path(), 16.0, theme.text))
                                            .child(icon(
                                                "icons/chevron-down.svg",
                                                10.0,
                                                theme.text_tertiary,
                                            ))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Some(d) = this.project_action_dialog.as_mut()
                                                {
                                                    d.icon_picker_open = !d.icon_picker_open;
                                                    cx.notify();
                                                }
                                            })),
                                    )
                                    // Name input
                                    .child(
                                        div()
                                            .flex_1()
                                            .h(px(34.0))
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
                            // Inline Icon Grid Drawer
                            .when(icon_picker_open, |parent| {
                                let options = ProjectScriptIcon::ALL;
                                parent.child(
                                    div()
                                        .id("project-action-icon-tray")
                                        .p(px(8.0))
                                        .rounded(px(8.0))
                                        .border_1()
                                        .border_color(theme.border)
                                        .bg(theme.surface)
                                        .flex()
                                        .flex_col()
                                        .gap(px(6.0))
                                        .child(
                                            div()
                                                .text_size(sp(11.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme.text_tertiary)
                                                .child(tr!("actions.icon_picker_title")),
                                        )
                                        .child(div().flex().flex_wrap().gap(px(6.0)).children(
                                            options.into_iter().map(|entry| {
                                                let is_selected = entry == current_icon;
                                                div()
                                                    .id(ElementId::from(format!(
                                                        "icon-opt-{}",
                                                        entry.id()
                                                    )))
                                                    .px(px(10.0))
                                                    .py(px(6.0))
                                                    .rounded(px(6.0))
                                                    .cursor_pointer()
                                                    .border_1()
                                                    .border_color(if is_selected {
                                                        theme.accent
                                                    } else {
                                                        theme.border
                                                    })
                                                    .bg(if is_selected {
                                                        theme.overlay_strong
                                                    } else {
                                                        theme.surface
                                                    })
                                                    .hover(|e| e.bg(theme.overlay))
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(6.0))
                                                    .child(icon(
                                                        entry.icon_path(),
                                                        14.0,
                                                        if is_selected {
                                                            theme.accent
                                                        } else {
                                                            theme.text
                                                        },
                                                    ))
                                                    .child(
                                                        div()
                                                            .text_size(sp(12.0))
                                                            .font_weight(if is_selected {
                                                                FontWeight::MEDIUM
                                                            } else {
                                                                FontWeight::NORMAL
                                                            })
                                                            .text_color(theme.text)
                                                            .child(entry.label()),
                                                    )
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        if let Some(d) =
                                                            this.project_action_dialog.as_mut()
                                                        {
                                                            d.icon = entry;
                                                            d.icon_picker_open = false;
                                                            cx.notify();
                                                        }
                                                    }))
                                            }),
                                        )),
                                )
                            }),
                    )
                    // Field 2: Terminal Command
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(sp(12.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.text_secondary)
                                            .child(tr!("actions.command")),
                                    )
                                    .child(
                                        div()
                                            .px(px(6.0))
                                            .py(px(1.0))
                                            .rounded(px(4.0))
                                            .bg(theme.overlay)
                                            .text_size(sp(10.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.text_tertiary)
                                            .child("sh / bash / zsh"),
                                    ),
                            )
                            .child(
                                div()
                                    .h(px(76.0))
                                    .px(px(10.0))
                                    .py(px(8.0))
                                    .rounded(px(7.0))
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.surface)
                                    .flex()
                                    .items_start()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .pt(px(1.0))
                                            .text_size(sp(12.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme.text_tertiary)
                                            .child("$"),
                                    )
                                    .child(div().flex_1().child(command_input)),
                            )
                            .child(
                                div()
                                    .text_size(sp(11.0))
                                    .text_color(theme.text_tertiary)
                                    .child(tr!("actions.command_help")),
                            ),
                    )
                    // Section 3: Keyboard Shortcut (Full-width Column with Auto Input)
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(sp(12.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.text_secondary)
                                            .child(tr!("actions.keybinding")),
                                    )
                                    .when(is_recording, |row| {
                                        row.child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(5.0))
                                                .child(
                                                    div()
                                                        .size(px(6.0))
                                                        .rounded_full()
                                                        .bg(theme.accent),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(sp(11.0))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(theme.accent)
                                                        .child(tr!("actions.recording_shortcut")),
                                                ),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .id("project-action-shortcut-recorder")
                                    .track_focus(&shortcut_focus)
                                    .tab_index(0)
                                    .w_full()
                                    .h(px(36.0))
                                    .px(px(10.0))
                                    .rounded(px(7.0))
                                    .border_1()
                                    .border_color(if is_recording {
                                        theme.accent
                                    } else {
                                        theme.border
                                    })
                                    .bg(if is_recording {
                                        theme.raised
                                    } else {
                                        theme.surface
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .cursor_pointer()
                                    .focus_visible(|style| {
                                        style.border_1().border_color(theme.accent)
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .min_w_0()
                                            .child(icon(
                                                "icons/command.svg",
                                                13.0,
                                                if is_recording {
                                                    theme.accent
                                                } else {
                                                    theme.text_tertiary
                                                },
                                            ))
                                            .child(if is_recording {
                                                if let Some(preview) = &live_recording_preview {
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(4.0))
                                                        .child(
                                                            div()
                                                                .px(px(7.0))
                                                                .py(px(2.0))
                                                                .rounded(px(5.0))
                                                                .bg(theme.accent.opacity(0.12))
                                                                .border_1()
                                                                .border_color(theme.accent)
                                                                .text_size(sp(12.0))
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(theme.accent)
                                                                .child(preview.clone()),
                                                        )
                                                        .into_any_element()
                                                } else if !current_shortcut.is_empty() {
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(4.0))
                                                        .child(
                                                            div()
                                                                .px(px(7.0))
                                                                .py(px(2.0))
                                                                .rounded(px(5.0))
                                                                .bg(theme.raised)
                                                                .border_1()
                                                                .border_color(theme.border_strong)
                                                                .text_size(sp(12.0))
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(theme.text)
                                                                .child(current_shortcut.clone()),
                                                        )
                                                        .into_any_element()
                                                } else {
                                                    div()
                                                        .text_size(sp(12.0))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(theme.accent)
                                                        .child(tr!("actions.recording_shortcut"))
                                                        .into_any_element()
                                                }
                                            } else if !current_shortcut.is_empty() {
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(4.0))
                                                    .child(
                                                        div()
                                                            .px(px(7.0))
                                                            .py(px(2.0))
                                                            .rounded(px(5.0))
                                                            .bg(theme.raised)
                                                            .border_1()
                                                            .border_color(theme.border_strong)
                                                            .text_size(sp(12.0))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(theme.text)
                                                            .child(current_shortcut.clone()),
                                                    )
                                                    .into_any_element()
                                            } else {
                                                div()
                                                    .text_size(sp(12.0))
                                                    .text_color(theme.text_tertiary)
                                                    .child(tr!("actions.click_to_record"))
                                                    .into_any_element()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .when(is_recording, |row| {
                                                row.child(
                                                    div()
                                                        .id("done-keybinding-record-btn")
                                                        .px(px(8.0))
                                                        .py(px(3.0))
                                                        .rounded(px(5.0))
                                                        .bg(theme.inverse)
                                                        .border_1()
                                                        .border_color(theme.inverse)
                                                        .text_size(sp(11.0))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(theme.on_inverse)
                                                        .cursor_pointer()
                                                        .hover(|e| e.opacity(0.9))
                                                        .child(tr!("actions.done_recording"))
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            cx.stop_propagation();
                                                            if let Some(d) =
                                                                this.project_action_dialog.as_mut()
                                                            {
                                                                d.recorder.stop_recording();
                                                                cx.notify();
                                                            }
                                                        })),
                                                )
                                                .child(
                                                    div()
                                                        .id("cancel-keybinding-record-btn")
                                                        .px(px(8.0))
                                                        .py(px(3.0))
                                                        .rounded(px(5.0))
                                                        .bg(theme.raised)
                                                        .border_1()
                                                        .border_color(theme.border)
                                                        .text_size(sp(11.0))
                                                        .text_color(theme.text_secondary)
                                                        .cursor_pointer()
                                                        .hover(|e| e.bg(theme.overlay))
                                                        .child(tr!("actions.cancel_recording"))
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            cx.stop_propagation();
                                                            if let Some(d) =
                                                                this.project_action_dialog.as_mut()
                                                            {
                                                                if let Some(orig) =
                                                                    d.recorder.cancel_recording()
                                                                {
                                                                    d.keybinding_input.update(
                                                                        cx,
                                                                        |input, cx| {
                                                                            input.set_content(
                                                                                orig, cx,
                                                                            );
                                                                        },
                                                                    );
                                                                }
                                                                cx.notify();
                                                            }
                                                        })),
                                                )
                                            })
                                            .when(
                                                !is_recording && !current_shortcut.is_empty(),
                                                |row| {
                                                    let kb_clone = keybinding_input.clone();
                                                    row.child(
                                                        div()
                                                            .id("clear-keybinding-button")
                                                            .size(px(20.0))
                                                            .rounded(px(4.0))
                                                            .cursor_pointer()
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .hover(|e| e.bg(theme.overlay))
                                                            .tooltip(Tooltip::text(tr!(
                                                                "actions.clear_shortcut"
                                                            )))
                                                            .child(icon(
                                                                "icons/x.svg",
                                                                11.0,
                                                                theme.text_tertiary,
                                                            ))
                                                            .on_click(cx.listener(
                                                                move |this, _, _, cx| {
                                                                    cx.stop_propagation();
                                                                    kb_clone.update(
                                                                        cx,
                                                                        |input, cx| {
                                                                            input.set_content(
                                                                                String::new(),
                                                                                cx,
                                                                            );
                                                                        },
                                                                    );
                                                                    if let Some(d) = this
                                                                        .project_action_dialog
                                                                        .as_mut()
                                                                    {
                                                                        d.recorder.stop_recording();
                                                                    }
                                                                    cx.notify();
                                                                },
                                                            )),
                                                    )
                                                },
                                            )
                                            .when(!is_recording, |row| {
                                                row.child(
                                                    div()
                                                        .px(px(8.0))
                                                        .py(px(3.0))
                                                        .rounded(px(5.0))
                                                        .bg(theme.raised)
                                                        .border_1()
                                                        .border_color(theme.border)
                                                        .text_size(sp(11.0))
                                                        .text_color(theme.text_secondary)
                                                        .hover(|e| e.bg(theme.overlay))
                                                        .child(tr!("actions.record_shortcut")),
                                                )
                                            }),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        cx.stop_propagation();
                                        if let Some(d) = this.project_action_dialog.as_mut() {
                                            if !d.recorder.is_recording {
                                                let current = Some(
                                                    d.keybinding_input
                                                        .read(cx)
                                                        .content()
                                                        .trim()
                                                        .to_string(),
                                                )
                                                .filter(|s| !s.is_empty());
                                                d.recorder.start_recording(current, window, cx);
                                            } else {
                                                let focus = d.recorder.focus.clone();
                                                window.focus(&focus, cx);
                                            }
                                            cx.notify();
                                        }
                                    }))
                                    .on_key_down(cx.listener(
                                        move |this, event: &KeyDownEvent, _, cx| {
                                            let Some(dialog) = this.project_action_dialog.as_mut()
                                            else {
                                                return;
                                            };

                                            match dialog.recorder.handle_key_down(event) {
                                                KeyDownResult::Committed => {
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }
                                                KeyDownResult::Cancelled(orig) => {
                                                    if let Some(orig) = orig {
                                                        dialog.keybinding_input.update(
                                                            cx,
                                                            |input, cx| {
                                                                input.set_content(orig, cx);
                                                            },
                                                        );
                                                    }
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }
                                                KeyDownResult::Cleared => {
                                                    dialog.keybinding_input.update(
                                                        cx,
                                                        |input, cx| {
                                                            input.set_content(String::new(), cx);
                                                        },
                                                    );
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }
                                                KeyDownResult::Shortcut(formatted) => {
                                                    dialog.keybinding_input.update(
                                                        cx,
                                                        |input, cx| {
                                                            input.set_content(formatted, cx);
                                                        },
                                                    );
                                                    dialog.recorder.is_recording = true;
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }
                                                KeyDownResult::PendingPreview => {
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }
                                                KeyDownResult::Ignored => {
                                                    cx.notify();
                                                }
                                            }
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(sp(10.5))
                                    .text_color(theme.text_tertiary)
                                    .child(tr!("actions.keybinding_help")),
                            ),
                    )
                    // Section 4: Browser Preview URL (Full-width Column)
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .text_size(sp(12.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.text_secondary)
                                            .child(tr!("actions.preview_url")),
                                    )
                                    .child(
                                        div()
                                            .text_size(sp(10.0))
                                            .text_color(theme.text_tertiary)
                                            .child(format!(
                                                "({})",
                                                tr!("actions.preview_url_optional")
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(34.0))
                                    .px(px(8.0))
                                    .rounded(px(7.0))
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.surface)
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child({
                                        let preview_url_val =
                                            preview_url_input.read(cx).content().trim().to_string();
                                        let has_url = !preview_url_val.is_empty();
                                        div()
                                            .id("dialog-preview-globe-button")
                                            .size(px(20.0))
                                            .rounded(px(4.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .when(has_url, |b| {
                                                let url_clone = preview_url_val.clone();
                                                b.cursor_pointer()
                                                    .hover(|e| e.bg(theme.overlay))
                                                    .tooltip(Tooltip::text(tr!(
                                                        "actions.open_preview"
                                                    )))
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.open_browser_url(
                                                            url_clone.clone(),
                                                            cx,
                                                        );
                                                    }))
                                            })
                                            .child(icon(
                                                "icons/globe.svg",
                                                13.0,
                                                if has_url {
                                                    theme.accent
                                                } else {
                                                    theme.text_tertiary
                                                },
                                            ))
                                    })
                                    .child(div().flex_1().child(preview_url_input.clone()))
                                    .when(
                                        !preview_url_input.read(cx).content().trim().is_empty(),
                                        |row| {
                                            let url_clone = preview_url_input.clone();
                                            row.child(
                                                div()
                                                    .id("clear-preview-url-button")
                                                    .size(px(18.0))
                                                    .rounded(px(4.0))
                                                    .cursor_pointer()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .hover(|e| e.bg(theme.overlay))
                                                    .child(icon(
                                                        "icons/x.svg",
                                                        10.0,
                                                        theme.text_tertiary,
                                                    ))
                                                    .on_click(cx.listener(move |_, _, _, cx| {
                                                        url_clone.update(cx, |input, cx| {
                                                            input.set_content(String::new(), cx);
                                                        });
                                                    })),
                                            )
                                        },
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(sp(10.5))
                                    .text_color(theme.text_tertiary)
                                    .child(tr!("actions.preview_url_help")),
                            ),
                    )
                    // Section 5: Execution & Automation Settings Card
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .p(px(12.0))
                            .rounded(px(10.0))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.surface)
                            // Card Title
                            .child(
                                div()
                                    .text_size(sp(11.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_secondary)
                                    .child(tr!("actions.options_section")),
                            )
                            // Toggle 1: Run on workspace creation
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .text_size(sp(12.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.text)
                                                    .child(tr!("actions.run_on_worktree_create")),
                                            )
                                            .child(
                                                div()
                                                    .text_size(sp(11.0))
                                                    .text_color(theme.text_tertiary)
                                                    .child(tr!(
                                                        "actions.run_on_worktree_create_desc"
                                                    )),
                                            ),
                                    )
                                    .child(toggle_switch(
                                        "toggle-action-worktree",
                                        run_on_worktree_create,
                                        false,
                                        theme,
                                        cx,
                                        |this: &mut Self, _, cx| {
                                            if let Some(d) = this.project_action_dialog.as_mut() {
                                                d.run_on_worktree_create =
                                                    !d.run_on_worktree_create;
                                                cx.notify();
                                            }
                                        },
                                    )),
                            )
                            // Toggle 2: Wait for setup (indented sub-item)
                            .child(
                                div()
                                    .pl(px(14.0))
                                    .border_l_1()
                                    .border_color(if run_on_worktree_create {
                                        theme.border_strong
                                    } else {
                                        theme.border
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .text_size(sp(12.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(if run_on_worktree_create {
                                                        theme.text
                                                    } else {
                                                        theme.text_tertiary
                                                    })
                                                    .child(tr!("actions.wait_for_setup")),
                                            )
                                            .child(
                                                div()
                                                    .text_size(sp(11.0))
                                                    .text_color(theme.text_tertiary)
                                                    .child(tr!("actions.wait_for_setup_desc")),
                                            ),
                                    )
                                    .child(toggle_switch(
                                        "toggle-action-wait-setup",
                                        wait_for_setup,
                                        !run_on_worktree_create,
                                        theme,
                                        cx,
                                        |this: &mut Self, _, cx| {
                                            if let Some(d) = this.project_action_dialog.as_mut() {
                                                if d.run_on_worktree_create {
                                                    d.wait_for_setup = !d.wait_for_setup;
                                                    cx.notify();
                                                }
                                            }
                                        },
                                    )),
                            )
                            // Divider
                            .child(div().h(px(1.0)).bg(theme.border))
                            // Toggle 3: Auto-open browser preview
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .text_size(sp(12.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(if has_preview_url {
                                                        theme.text
                                                    } else {
                                                        theme.text_tertiary
                                                    })
                                                    .child(tr!("actions.auto_open_preview")),
                                            )
                                            .child(
                                                div()
                                                    .text_size(sp(11.0))
                                                    .text_color(theme.text_tertiary)
                                                    .child(tr!("actions.auto_open_preview_desc")),
                                            ),
                                    )
                                    .child(toggle_switch(
                                        "toggle-action-auto-preview",
                                        auto_open_preview,
                                        !has_preview_url,
                                        theme,
                                        cx,
                                        |this: &mut Self, _, cx| {
                                            if let Some(d) = this.project_action_dialog.as_mut() {
                                                let has_url = !d
                                                    .preview_url_input
                                                    .read(cx)
                                                    .content()
                                                    .trim()
                                                    .is_empty();
                                                if has_url {
                                                    d.auto_open_preview = !d.auto_open_preview;
                                                    cx.notify();
                                                }
                                            }
                                        },
                                    )),
                            ),
                    ),
            )
            // Footer Actions (Pinned)
            .child(
                div()
                    .px(px(20.0))
                    .py(px(14.0))
                    .border_t_1()
                    .border_color(theme.border)
                    .bg(theme.raised)
                    .flex()
                    .items_center()
                    .justify_between()
                    // Left: Delete Action (when editing)
                    .when_some(delete_focus.clone(), |row, del_focus| {
                        row.child(
                            div()
                                .id("delete-project-action-button")
                                .track_focus(&del_focus)
                                .tab_index(0)
                                .h(px(32.0))
                                .px(px(10.0))
                                .rounded(px(7.0))
                                .border_1()
                                .border_color(theme.border)
                                .cursor_pointer()
                                .text_size(sp(12.0))
                                .text_color(theme.danger)
                                .focus_visible(|style| style.border_1().border_color(theme.accent))
                                .hover(|e| e.bg(theme.overlay))
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap(px(6.0))
                                .child(icon("icons/trash.svg", 13.0, theme.danger))
                                .child(tr!("actions.delete"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.project_action_dialog_delete(window, cx);
                                }))
                                .on_key_down(cx.listener(
                                    |this, event: &KeyDownEvent, window, cx| {
                                        if !event.keystroke.modifiers.modified()
                                            && matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            )
                                        {
                                            this.project_action_dialog_delete(window, cx);
                                            cx.stop_propagation();
                                        }
                                    },
                                )),
                        )
                    })
                    .when(delete_focus.is_none(), |row| row.child(div()))
                    // Right: Cancel & Save Buttons
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            // Cancel Button
                            .child(
                                div()
                                    .id("cancel-project-action-dialog")
                                    .track_focus(&cancel_focus)
                                    .tab_index(0)
                                    .h(px(32.0))
                                    .px(px(14.0))
                                    .rounded(px(7.0))
                                    .border_1()
                                    .border_color(theme.border)
                                    .cursor_pointer()
                                    .text_size(sp(12.0))
                                    .text_color(theme.text_secondary)
                                    .focus_visible(|style| {
                                        style.border_1().border_color(theme.accent)
                                    })
                                    .hover(|e| e.bg(theme.overlay))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(6.0))
                                    .child(tr!("actions.cancel"))
                                    .child(
                                        div()
                                            .text_size(sp(10.0))
                                            .text_color(theme.text_tertiary)
                                            .child("Esc"),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_project_action_dialog(window, cx);
                                    }))
                                    .on_key_down(cx.listener(
                                        |this, event: &KeyDownEvent, window, cx| {
                                            if !event.keystroke.modifiers.modified()
                                                && matches!(
                                                    event.keystroke.key.as_str(),
                                                    "enter" | "space"
                                                )
                                            {
                                                this.close_project_action_dialog(window, cx);
                                                cx.stop_propagation();
                                            }
                                        },
                                    )),
                            )
                            // Primary Save Button
                            .child(
                                div()
                                    .id("save-project-action-dialog")
                                    .track_focus(&save_focus)
                                    .tab_index(0)
                                    .h(px(32.0))
                                    .px(px(14.0))
                                    .rounded(px(7.0))
                                    .border_1()
                                    .border_color(theme.inverse)
                                    .cursor_pointer()
                                    .text_size(sp(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.on_inverse)
                                    .bg(theme.inverse)
                                    .focus_visible(|style| {
                                        style.border_1().border_color(theme.accent)
                                    })
                                    .hover(|e| e.opacity(0.9))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(6.0))
                                    .child(save_label)
                                    .child(
                                        div()
                                            .text_size(sp(10.0))
                                            .font_weight(FontWeight::NORMAL)
                                            .text_color(theme.on_inverse.opacity(0.75))
                                            .child("⌘↵"),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.project_action_dialog_save(window, cx);
                                    }))
                                    .on_key_down(cx.listener(
                                        |this, event: &KeyDownEvent, window, cx| {
                                            if !event.keystroke.modifiers.modified()
                                                && matches!(
                                                    event.keystroke.key.as_str(),
                                                    "enter" | "space"
                                                )
                                            {
                                                this.project_action_dialog_save(window, cx);
                                                cx.stop_propagation();
                                            }
                                        },
                                    )),
                            ),
                    ),
            );

        Some(dialog_backdrop(
            "project-action-dialog-backdrop",
            &theme,
            cx,
            |this, window, cx| {
                this.close_project_action_dialog(window, cx);
            },
            card,
        ))
    }
}

fn slug_id(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        format!("{}-{}", trimmed, &Uuid::new_v4().to_string()[..8])
    }
}
