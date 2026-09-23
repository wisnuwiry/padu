use super::*;
use crate::model::{ProjectScript, ProjectScriptIcon};
use crate::ui::menu::{MenuAlign, popover};
use gpui::prelude::FluentBuilder;
use gpui::{ElementId, StatefulInteractiveElement};
use serde::{Deserialize, Serialize};

pub const PROJECT_ACTIONS_MENU_ID: &str = "header-project-actions-menu";

/// A script definition parsed from a configuration file (`padu.json` or `t3.json`).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileScript {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub run_on_worktree_create: Option<bool>,
    #[serde(default)]
    pub async_run: Option<bool>,
    #[serde(default)]
    pub preview_url: Option<String>,
    #[serde(default)]
    pub auto_open_preview: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct ProjectConfigFile {
    #[serde(default)]
    pub scripts: Vec<FileScript>,
}

/// Reads one candidate configuration file and parses its scripts.
///
/// The read goes through the daemon, which owns the project's filesystem. A
/// local read of a project under `~/Documents` asks macOS for the folder in
/// Padu's own name, and a remote host does not have the path on this machine at
/// all. A missing or unparseable file is simply not a config.
fn read_project_config_scripts(
    workspace: &padu_client::WorkspaceClient,
    project_path: &Path,
    file_name: &str,
) -> Option<Vec<FileScript>> {
    let Ok(padu_client::WorkspaceResult::TextFile { content }) =
        workspace.request(padu_client::WorkspaceOperation::ReadTextFile {
            root: project_path.to_path_buf(),
            relative_path: PathBuf::from(file_name),
        })
    else {
        return None;
    };
    serde_json::from_str::<ProjectConfigFile>(&content)
        .ok()
        .map(|config| config.scripts)
}

/// The scripts a project declares in `padu.json`, else in `t3.json`, with the
/// file they came from — empty when it declares none.
pub fn read_project_file_scripts(
    workspace: &padu_client::WorkspaceClient,
    project_path: &Path,
) -> (Vec<FileScript>, &'static str) {
    for file_name in ["padu.json", "t3.json"] {
        if let Some(scripts) = read_project_config_scripts(workspace, project_path, file_name) {
            return (scripts, file_name);
        }
    }
    (Vec::new(), "")
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

impl Padu {
    pub(super) fn detect_file_scripts(&self, path: PathBuf, cx: &mut Context<Self>) {
        if !self.pending_file_scripts.borrow_mut().insert(path.clone()) {
            return;
        }
        let scan_path = path.clone();
        let workspace = padu_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_executor()
                .spawn(async move { read_project_file_scripts(&workspace, &scan_path) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.pending_file_scripts.borrow_mut().remove(&path);
                this.cached_file_scripts.insert(path, res);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn preferred_project_script(
        &self,
        project_id: Uuid,
        scripts: &[ProjectScript],
    ) -> Option<ProjectScript> {
        if scripts.is_empty() {
            return None;
        }
        if let Some(id) = self.state.last_invoked_script.get(&project_id) {
            if let Some(script) = scripts.iter().find(|s| &s.id == id) {
                return Some(script.clone());
            }
        }
        scripts.first().cloned()
    }

    pub(crate) fn active_terminal_view(
        &self,
    ) -> Option<gpui::Entity<crate::terminal::TerminalView>> {
        if let Some(index) = self.right_panel_active_surface {
            if let Some(surface) = self.right_panel_surfaces.get(index) {
                if let Some(terminal_id) = surface.terminal_id() {
                    if let Some(term) = self.right_panel_terminals.get(&terminal_id) {
                        return Some(term.clone());
                    }
                }
            }
        }
        self.right_panel_terminals.values().next().cloned()
    }

    pub(crate) fn open_browser_url(&mut self, url: String, cx: &mut Context<Self>) {
        self.ensure_browser_surface(None, cx);
        if let Some(index) = self.right_panel_active_surface {
            if let Some(RightPanelSurface::Browser(browser_id)) =
                self.right_panel_surfaces.get(index)
            {
                let browser_id = *browser_id;
                if let Some(browser) = self.right_panel_browsers.get(&browser_id) {
                    browser.update(cx, |b, cx| b.navigate_to_url(url, cx));
                } else {
                    self.right_panel_pending_browser_urls
                        .insert(browser_id, url);
                }
            }
        }
    }

    pub fn run_project_script(&mut self, script: &ProjectScript, cx: &mut Context<Self>) {
        let active_project = self.active_project().cloned();
        let Some(project) = active_project else {
            return;
        };

        self.state
            .last_invoked_script
            .insert(project.id, script.id.clone());
        let _ = self.save();

        let command = script.command.trim().to_owned();

        // Open preview if auto_open_preview and preview_url is present
        if script.auto_open_preview {
            if let Some(preview_url) = &script.preview_url {
                let trimmed = preview_url.trim();
                if !trimmed.is_empty() {
                    self.open_browser_url(trimmed.to_owned(), cx);
                }
            }
        }

        if !command.is_empty() {
            self.ensure_terminal_surface(None, cx);
            if let Some(term) = self.active_terminal_view() {
                if term.read(cx).is_ready() {
                    term.update(cx, |view, _| view.send_command(&command));
                } else {
                    let cmd = command.clone();
                    cx.spawn(async move |this, cx| {
                        // Poll until ready (up to 3 seconds)
                        for _ in 0..60 {
                            let ready = this
                                .update(cx, |this, cx| {
                                    if let Some(term) = this.active_terminal_view() {
                                        if term.read(cx).is_ready() {
                                            term.update(cx, |view, _| view.send_command(&cmd));
                                            return true;
                                        }
                                    }
                                    false
                                })
                                .unwrap_or(false);
                            if ready {
                                break;
                            }
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(50))
                                .await;
                        }
                    })
                    .detach();
                }
            }
        }

        self.show_success_toast(tr!("actions.running", name = &script.name));
        cx.notify();
    }

    pub fn add_project_script(
        &mut self,
        project_id: Uuid,
        script: ProjectScript,
        cx: &mut Context<Self>,
    ) {
        if let Some(project) = self.state.projects.iter_mut().find(|p| p.id == project_id) {
            self.state
                .last_invoked_script
                .insert(project_id, script.id.clone());
            project.scripts.push(script);
            let _ = self.save();
            cx.notify();
        }
    }

    pub fn update_project_script(
        &mut self,
        project_id: Uuid,
        script_id: &str,
        updated: ProjectScript,
        cx: &mut Context<Self>,
    ) {
        if let Some(project) = self.state.projects.iter_mut().find(|p| p.id == project_id) {
            if let Some(pos) = project.scripts.iter().position(|s| s.id == script_id) {
                project.scripts[pos] = updated;
                let _ = self.save();
                cx.notify();
            }
        }
    }

    pub fn delete_project_script(
        &mut self,
        project_id: Uuid,
        script_id: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(project) = self.state.projects.iter_mut().find(|p| p.id == project_id) {
            project.scripts.retain(|s| s.id != script_id);
            if self
                .state
                .last_invoked_script
                .get(&project_id)
                .map(|s| s.as_str())
                == Some(script_id)
            {
                self.state.last_invoked_script.remove(&project_id);
            }
            let _ = self.save();
            cx.notify();
        }
    }

    pub fn import_file_script(
        &mut self,
        project_id: Uuid,
        file_script: FileScript,
        cx: &mut Context<Self>,
    ) {
        let script = ProjectScript {
            id: slug_id(&file_script.name),
            name: file_script.name,
            command: file_script.command,
            icon: file_script
                .icon
                .as_deref()
                .map(ProjectScriptIcon::from_id)
                .unwrap_or(ProjectScriptIcon::Play),
            run_on_worktree_create: file_script.run_on_worktree_create.unwrap_or(false),
            async_run: file_script.async_run,
            preview_url: file_script.preview_url,
            auto_open_preview: file_script.auto_open_preview.unwrap_or(false),
            keybinding: None,
        };
        self.add_project_script(project_id, script, cx);
    }

    pub(crate) fn handle_project_action_keystroke_event(
        &mut self,
        event: &gpui::KeystrokeEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.project_action_dialog.is_some()
            || self.project_action_dialog_request.is_some()
            || self.confirm_dialog.is_some()
            || self.commit_dialog.is_some()
            || self.goal_dialog.is_some()
            || self.goal_dialog_request.is_some()
            || self.host_dialog.is_some()
            || self.host_dialog_request.is_some()
            || self.whats_new.is_some()
            || self.command_palette.open
            || self.settings_page.is_some()
        {
            return;
        }

        // Only trigger on shortcuts with modifiers or function keys to avoid intercepting normal typing
        let is_chord = event.keystroke.modifiers.platform
            || event.keystroke.modifiers.control
            || event.keystroke.modifiers.alt
            || (event.keystroke.key.starts_with('f')
                && event.keystroke.key[1..].chars().all(|c| c.is_ascii_digit()));
        if !is_chord {
            return;
        }

        let Some(project) = self.active_project().cloned() else {
            return;
        };

        for script in &project.scripts {
            if let Some(ref kb) = script.keybinding {
                if crate::ui::matches_keystroke(kb, &event.keystroke) {
                    cx.stop_propagation();
                    self.run_project_script(&script.clone(), cx);
                    return;
                }
            }
        }
    }

    /// Renders the project actions topbar split-button control.
    pub fn render_project_actions_control(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let project = self.active_project()?.clone();
        let project_id = project.id;
        let scripts = project.scripts.clone();

        let (file_scripts, config_filename) =
            if let Some(cached) = self.cached_file_scripts.get(&project.path) {
                cached.clone()
            } else {
                self.detect_file_scripts(project.path.clone(), cx);
                (Vec::new(), "")
            };

        let importable_scripts: Vec<FileScript> = file_scripts
            .into_iter()
            .filter(|fs| {
                !scripts
                    .iter()
                    .any(|s| s.command == fs.command || s.name.eq_ignore_ascii_case(&fs.name))
            })
            .collect();

        let theme = Theme::current(cx);
        let handle = self.menu_handle(PROJECT_ACTIONS_MENU_ID, cx);
        let focus = self.transcript_control_focus("header-project-actions", cx);
        let weak = cx.entity().downgrade();

        let has_scripts = !scripts.is_empty();
        let has_importable = !importable_scripts.is_empty();

        // If no configured scripts and no importable scripts, show simple "+ Add action" button
        if !has_scripts && !has_importable {
            return Some(
                div()
                    .id("header-add-action-button")
                    .track_focus(&focus)
                    .tab_index(0)
                    .h(px(28.0))
                    .px(px(8.0))
                    .gap(px(6.0))
                    .rounded(px(7.0))
                    .border_1()
                    .border_color(theme.border_strong)
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .focus_visible(|style| {
                        style
                            .bg(theme.overlay)
                            .border_1()
                            .border_color(theme.accent)
                    })
                    .hover(|style| style.bg(theme.overlay))
                    .active(|style| style.bg(theme.overlay_strong))
                    .tooltip(Tooltip::text(tr!("actions.add")))
                    .child(icon("icons/plus.svg", 13.0, theme.text_secondary))
                    .child(
                        div()
                            .text_size(sp(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_secondary)
                            .child(tr!("actions.add")),
                    )
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.request_project_action_dialog(project_id, None, cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            this.request_project_action_dialog(project_id, None, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .into_any_element(),
            );
        }

        let primary_script = self
            .preferred_project_script(project_id, &scripts)
            .or_else(|| scripts.first().cloned());

        let primary_elem = if let Some(primary) = primary_script {
            let p_click = primary.clone();
            let p_key = primary.clone();
            div()
                .id("header-action-primary")
                .track_focus(&focus)
                .tab_index(0)
                .h_full()
                .px(px(7.0))
                .gap(px(6.0))
                .rounded_tl(px(6.0))
                .rounded_bl(px(6.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .focus_visible(|style| {
                    style
                        .bg(theme.overlay)
                        .border_1()
                        .border_color(theme.accent)
                })
                .hover(|style| style.bg(theme.overlay))
                .active(|style| style.bg(theme.overlay_strong))
                .tooltip(Tooltip::text(tr!("actions.run", name = &primary.name)))
                .child(icon(primary.icon.icon_path(), 13.0, theme.text))
                .child(
                    div()
                        .max_w(px(110.0))
                        .truncate()
                        .text_size(sp(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(primary.name.clone()),
                )
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.run_project_script(&p_click, cx);
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        this.run_project_script(&p_key, cx);
                        cx.stop_propagation();
                    }
                }))
        } else {
            div()
                .id("header-action-primary-empty")
                .track_focus(&focus)
                .tab_index(0)
                .h_full()
                .px(px(8.0))
                .gap(px(6.0))
                .rounded_tl(px(6.0))
                .rounded_bl(px(6.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|style| style.bg(theme.overlay))
                .child(icon("icons/plus.svg", 13.0, theme.text_secondary))
                .child(
                    div()
                        .text_size(sp(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_secondary)
                        .child(tr!("actions.add")),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.request_project_action_dialog(project_id, None, cx);
                }))
        };

        let caret = div()
            .id("header-action-caret")
            .h_full()
            .w(px(18.0))
            .rounded_tr(px(6.0))
            .rounded_br(px(6.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .focus_visible(|style| {
                style
                    .bg(theme.overlay)
                    .border_1()
                    .border_color(theme.accent)
            })
            .hover(|style| style.bg(theme.overlay))
            .tooltip(Tooltip::text(tr!("actions.description")))
            .child(icon(
                if handle.is_open() {
                    "icons/chevron-up.svg"
                } else {
                    "icons/chevron-down.svg"
                },
                11.0,
                theme.text_tertiary,
            ));

        let menu_scripts = scripts.clone();
        let menu_importables = importable_scripts.clone();
        let config_filename_str = config_filename.to_string();

        let menu = popover(
            caret,
            &handle,
            MenuAlign::BelowRight,
            move |handle, _window, cx| {
                let theme = Theme::current(cx);
                let handle_ref = handle.clone();
                let mut content = div()
                    .id("project-actions-popover")
                    .w(px(240.0))
                    .p(px(4.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(theme.border_strong)
                    .bg(theme.raised)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());

                // Existing scripts
                for script in &menu_scripts {
                    let s_run = script.clone();
                    let s_edit = script.clone();
                    let h_run = handle_ref.clone();
                    let h_edit = handle_ref.clone();
                    let w_run = weak.clone();
                    let w_edit = weak.clone();

                    let row = div()
                        .id(ElementId::from(format!("action-row-{}", script.id)))
                        .h(px(30.0))
                        .px(px(6.0))
                        .rounded(px(6.0))
                        .flex()
                        .items_center()
                        .justify_between()
                        .hover(|e| e.bg(theme.overlay))
                        .cursor_pointer()
                        // Main clickable area to run action
                        .child(
                            div()
                                .id(ElementId::from(format!("action-run-{}", script.id)))
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(icon(script.icon.icon_path(), 14.0, theme.text))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(4.0))
                                        .min_w_0()
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(sp(12.5))
                                                .text_color(theme.text)
                                                .child(script.name.clone()),
                                        )
                                        .when(script.run_on_worktree_create, |parent| {
                                            parent.child(
                                                div()
                                                    .text_size(sp(11.0))
                                                    .text_color(theme.text_tertiary)
                                                    .child(tr!("actions.setup_suffix")),
                                            )
                                        }),
                                )
                                .when_some(script.keybinding.clone(), |parent, kb| {
                                    let display_kb = crate::ui::format_shortcut_for_display(&kb);
                                    parent.child(
                                        div()
                                            .px(px(4.0))
                                            .py(px(1.0))
                                            .rounded(px(4.0))
                                            .bg(theme.surface)
                                            .border_1()
                                            .border_color(theme.border)
                                            .text_size(sp(10.5))
                                            .text_color(theme.text_tertiary)
                                            .child(display_kb),
                                    )
                                })
                                .on_click(move |_, window, cx| {
                                    h_run.close(window, cx);
                                    let _ = w_run.update(cx, |this, cx| {
                                        this.run_project_script(&s_run, cx);
                                    });
                                }),
                        )
                        // Open browser preview button if preview_url is present
                        .when_some(script.preview_url.clone(), |parent, preview_url| {
                            let trimmed = preview_url.trim().to_string();
                            if trimmed.is_empty() {
                                return parent;
                            }
                            let w_prev = weak.clone();
                            let h_prev = handle.clone();
                            parent.child(
                                div()
                                    .id(ElementId::from(format!("preview-action-{}", script.id)))
                                    .size(px(22.0))
                                    .rounded(px(4.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .hover(|e| e.bg(theme.surface))
                                    .tooltip(Tooltip::text(tr!("actions.open_preview")))
                                    .child(icon("icons/globe.svg", 12.0, theme.text_tertiary))
                                    .on_click(move |_, window, cx| {
                                        h_prev.close(window, cx);
                                        let _ = w_prev.update(cx, |this, cx| {
                                            this.open_browser_url(trimmed.clone(), cx);
                                        });
                                    }),
                            )
                        })
                        // Edit button on right
                        .child(
                            div()
                                .id(ElementId::from(format!("edit-action-{}", script.id)))
                                .size(px(22.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .hover(|e| e.bg(theme.surface))
                                .tooltip(Tooltip::text(tr!("actions.edit", name = &script.name)))
                                .child(icon("icons/settings.svg", 12.0, theme.text_tertiary))
                                .on_click(move |_, window, cx| {
                                    h_edit.close(window, cx);
                                    let _ = w_edit.update(cx, |this, cx| {
                                        this.request_project_action_dialog(
                                            project_id,
                                            Some(s_edit.clone()),
                                            cx,
                                        );
                                    });
                                }),
                        );

                    content = content.child(row);
                }

                // Importable file scripts section
                if !menu_importables.is_empty() {
                    content = content
                        .child(div().h(px(1.0)).my(px(2.0)).bg(theme.border))
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(2.0))
                                .text_size(sp(11.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text_tertiary)
                                .child(tr!("actions.from_file", file = &config_filename_str)),
                        );

                    for file_script in &menu_importables {
                        let fs = file_script.clone();
                        let h_imp = handle_ref.clone();
                        let w_imp = weak.clone();
                        let fs_icon = file_script
                            .icon
                            .as_deref()
                            .map(ProjectScriptIcon::from_id)
                            .unwrap_or(ProjectScriptIcon::Play);

                        let row = div()
                            .id(ElementId::from(format!("import-fs-{}", file_script.name)))
                            .h(px(28.0))
                            .px(px(6.0))
                            .rounded(px(6.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .hover(|e| e.bg(theme.overlay))
                            .cursor_pointer()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(icon(fs_icon.icon_path(), 13.0, theme.text_secondary))
                                    .child(
                                        div()
                                            .text_size(sp(12.0))
                                            .text_color(theme.text)
                                            .child(file_script.name.clone()),
                                    ),
                            )
                            .child(icon("icons/download.svg", 12.0, theme.text_tertiary))
                            .on_click(move |_, window, cx| {
                                h_imp.close(window, cx);
                                let _ = w_imp.update(cx, |this, cx| {
                                    this.import_file_script(project_id, fs.clone(), cx);
                                });
                            });

                        content = content.child(row);
                    }
                }

                // Divider and "+ Add action" row
                let h_add = handle_ref.clone();
                let w_add = weak.clone();
                content = content
                    .child(div().h(px(1.0)).my(px(2.0)).bg(theme.border))
                    .child(
                        div()
                            .id("action-row-add-new")
                            .h(px(28.0))
                            .px(px(6.0))
                            .rounded(px(6.0))
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .hover(|e| e.bg(theme.overlay))
                            .cursor_pointer()
                            .child(icon("icons/plus.svg", 13.0, theme.text_secondary))
                            .child(
                                div()
                                    .text_size(sp(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(tr!("actions.add")),
                            )
                            .on_click(move |_, window, cx| {
                                h_add.close(window, cx);
                                let _ = w_add.update(cx, |this, cx| {
                                    this.request_project_action_dialog(project_id, None, cx);
                                });
                            }),
                    );

                content.into_any_element()
            },
        );

        Some(
            div()
                .h(px(28.0))
                .rounded(px(7.0))
                .border_1()
                .border_color(theme.border_strong)
                .flex_none()
                .flex()
                .items_center()
                .child(primary_elem)
                .child(div().w(px(1.0)).h_full().flex_none().bg(theme.border))
                .child(menu)
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    /// The header draws this probe for the selected project, so it must not
    /// touch the project path itself. A local read of a project under
    /// `~/Documents` makes macOS ask for the folder in Padu's own name, and a
    /// remote host does not have the path on this machine at all, so the
    /// daemon — which owns the filesystem — has to answer instead.
    #[test]
    fn the_script_probe_reads_through_the_daemon() {
        let source = include_str!("project_actions.rs");
        // Anchored past the test module so the literals below do not match
        // themselves.
        let source = source
            .split_once("\n#[cfg(test)]")
            .expect("the test module")
            .0;

        for forbidden in ["std::fs::", "is_file()", "read_to_string"] {
            assert!(
                !source.contains(forbidden),
                "project script detection must not call `{forbidden}`; \
                 read the config with WorkspaceOperation::ReadTextFile instead"
            );
        }
        assert!(source.contains("WorkspaceOperation::ReadTextFile"));
    }
}
