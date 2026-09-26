use super::*;
use crate::model::ProviderKind;
use crate::theme::{Theme, sp};
use crate::ui::dialog::{dialog_backdrop, dialog_cancel_button, dialog_card};
use crate::ui::text_field::TextField;
use crate::ui::{icon, provider_icon, scrollbar};
use padu_client::kanban::{CreateTask, TaskStatus, TaskSummary};

const BOARD_COLUMNS: [TaskStatus; 5] = [
    TaskStatus::Backlog,
    TaskStatus::Queued,
    TaskStatus::Running,
    TaskStatus::Review,
    TaskStatus::Done,
];

pub fn task_status_label(status: TaskStatus) -> String {
    match status {
        TaskStatus::Backlog => tr!("board.backlog"),
        TaskStatus::Queued => tr!("board.queued"),
        TaskStatus::Running => tr!("board.running"),
        TaskStatus::Review => tr!("board.review"),
        TaskStatus::Done => tr!("board.done"),
    }
}

pub fn task_status_color(status: TaskStatus, theme: &Theme) -> gpui::Hsla {
    match status {
        TaskStatus::Backlog => theme.text_tertiary,
        TaskStatus::Queued => theme.accent,
        TaskStatus::Running => theme.warning,
        TaskStatus::Review => theme.gauge,
        TaskStatus::Done => theme.success,
    }
}

impl Padu {
    pub(super) fn open_board(&mut self, cx: &mut Context<Self>) {
        self.navigate_workspace_page(WorkspacePage::Board, cx);
        self.ensure_board_tasks_loaded(cx);
    }

    pub(super) fn ensure_board_tasks_loaded(&mut self, cx: &mut Context<Self>) {
        if self.board_loaded || self.board_load_pending {
            return;
        }
        self.load_board_tasks_from_daemon(cx);
    }

    pub(super) fn load_board_tasks_from_daemon(&mut self, cx: &mut Context<Self>) {
        if self.board_load_pending {
            return;
        }
        self.board_load_pending = true;
        self.board_load_generation = self.board_load_generation.wrapping_add(1);
        let generation = self.board_load_generation;
        let daemon = self.daemon.clone();
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let response = daemon.client().request(
                        Uuid::nil(),
                        Uuid::nil(),
                        padu_client::Command::ListTasks,
                    )?;
                    let padu_client::ResponsePayload::Tasks { tasks } = response else {
                        anyhow::bail!("daemon returned an invalid Tasks response");
                    };
                    Ok::<_, anyhow::Error>(tasks)
                })
                .await;
            let _ = padu.update(cx, |this, cx| {
                if this.board_load_generation != generation {
                    return;
                }
                this.board_load_pending = false;
                match result {
                    Ok(tasks) => {
                        this.board_tasks = tasks;
                        this.board_loaded = true;
                        cx.notify();
                    }
                    Err(error) => {
                        this.show_toast(format!("Failed to load tasks: {error}"));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(super) fn move_board_task(
        &mut self,
        task_id: Uuid,
        status: TaskStatus,
        expected_version: u64,
        cx: &mut Context<Self>,
    ) {
        if status == TaskStatus::Running {
            self.show_toast(tr!("board.drag_running_rejected"));
            return;
        }

        // Optimistically update local summary
        if let Some(task) = self.board_tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = status;
            task.version = task.version.wrapping_add(1);
        }
        if let Some(hydrated) = self.board_hydrated_task.as_mut() {
            if hydrated.id == task_id {
                hydrated.status = status;
                hydrated.version = hydrated.version.wrapping_add(1);
            }
        }
        cx.notify();

        let daemon = self.daemon.clone();
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let response = daemon.client().request(
                        Uuid::nil(),
                        Uuid::nil(),
                        padu_client::Command::MoveTask {
                            task_id,
                            status,
                            expected_version,
                        },
                    )?;
                    let padu_client::ResponsePayload::TaskMoved { task } = response else {
                        anyhow::bail!("daemon returned an invalid TaskMoved response");
                    };
                    Ok::<_, anyhow::Error>(task)
                })
                .await;
            let _ = padu.update(cx, |this, cx| match result {
                Ok(updated) => {
                    if let Some(t) = this.board_tasks.iter_mut().find(|t| t.id == updated.id) {
                        *t = TaskSummary::from_task(&updated);
                    }
                    if let Some(h) = this.board_hydrated_task.as_mut() {
                        if h.id == updated.id {
                            *h = updated;
                        }
                    }
                    cx.notify();
                }
                Err(error) => {
                    this.show_toast(format!("Failed to move task: {error}"));
                    this.load_board_tasks_from_daemon(cx);
                }
            });
        })
        .detach();
    }

    pub(super) fn select_board_task(&mut self, task_id: Option<Uuid>, cx: &mut Context<Self>) {
        self.board_selected_task_id = task_id;
        self.board_hydrated_task = None;
        if let Some(id) = task_id {
            self.hydrate_selected_task(id, cx);
        }
        cx.notify();
    }

    pub(super) fn hydrate_selected_task(&mut self, task_id: Uuid, cx: &mut Context<Self>) {
        let daemon = self.daemon.clone();
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let response = daemon.client().request(
                        Uuid::nil(),
                        Uuid::nil(),
                        padu_client::Command::HydrateTask { task_id },
                    )?;
                    let padu_client::ResponsePayload::TaskHydrated { task, .. } = response else {
                        anyhow::bail!("daemon returned an invalid TaskHydrated response");
                    };
                    Ok::<_, anyhow::Error>(task)
                })
                .await;
            let _ = padu.update(cx, |this, cx| match result {
                Ok(task) => {
                    if this.board_selected_task_id == Some(task.id) {
                        this.board_hydrated_task = Some(task);
                        cx.notify();
                    }
                }
                Err(error) => {
                    this.show_toast(format!("Failed to hydrate task: {error}"));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn delete_board_task(
        &mut self,
        task_id: Uuid,
        expected_version: u64,
        cx: &mut Context<Self>,
    ) {
        if self.board_selected_task_id == Some(task_id) {
            self.board_selected_task_id = None;
            self.board_hydrated_task = None;
        }
        self.board_tasks.retain(|t| t.id != task_id);
        cx.notify();

        let daemon = self.daemon.clone();
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let response = daemon.client().request(
                        Uuid::nil(),
                        Uuid::nil(),
                        padu_client::Command::DeleteTask {
                            task_id,
                            expected_version,
                        },
                    )?;
                    let padu_client::ResponsePayload::TaskDeleted { .. } = response else {
                        anyhow::bail!("daemon returned an invalid TaskDeleted response");
                    };
                    Ok::<_, anyhow::Error>(())
                })
                .await;
            let _ = padu.update(cx, |this, cx| match result {
                Ok(_) => {
                    this.show_success_toast(tr!("board.delete_task"));
                    cx.notify();
                }
                Err(error) => {
                    this.show_toast(format!("Failed to delete task: {error}"));
                    this.load_board_tasks_from_daemon(cx);
                }
            });
        })
        .detach();
    }

    pub(super) fn board_models_for_provider(
        &self,
        provider: ProviderKind,
    ) -> Vec<(String, String)> {
        if let Some(probe) = self.provider_probe(provider) {
            if !probe.models.is_empty() {
                return probe
                    .models
                    .iter()
                    .map(|m| (m.id.clone(), m.name.clone()))
                    .collect();
            }
        }
        match provider {
            ProviderKind::Claude => vec![
                ("claude-3-7-sonnet".into(), "Claude 3.7 Sonnet".into()),
                ("claude-3-5-sonnet".into(), "Claude 3.5 Sonnet".into()),
                ("claude-3-5-haiku".into(), "Claude 3.5 Haiku".into()),
            ],
            ProviderKind::Codex => vec![
                ("gpt-4o".into(), "GPT-4o".into()),
                ("o3-mini".into(), "o3-mini".into()),
                ("o1".into(), "o1".into()),
                ("gpt-4o-mini".into(), "GPT-4o-mini".into()),
            ],
            ProviderKind::Cursor => vec![
                ("claude-3.5-sonnet".into(), "Claude 3.5 Sonnet".into()),
                ("gpt-4o".into(), "GPT-4o".into()),
            ],
            ProviderKind::Agy => vec![
                ("gemini-2.0-flash".into(), "Gemini 2.0 Flash".into()),
                ("gemini-2.0-pro".into(), "Gemini 2.0 Pro".into()),
                ("gemini-1.5-pro".into(), "Gemini 1.5 Pro".into()),
            ],
            ProviderKind::DeepSeek => vec![
                ("deepseek-chat".into(), "DeepSeek Chat".into()),
                ("deepseek-reasoner".into(), "DeepSeek Reasoner".into()),
            ],
            ProviderKind::OpenCode => vec![("default".into(), "Default".into())],
            _ => vec![],
        }
    }

    pub(super) fn create_board_task(
        &mut self,
        project_id: Uuid,
        title: String,
        description: String,
        assigned_agent: Option<ProviderKind>,
        model: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let title = title.trim().to_owned();
        if title.is_empty() {
            return;
        }
        self.board_new_task_modal_open = false;
        self.board_new_task_model = None;
        let daemon = self.daemon.clone();
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let response = daemon.client().request(
                        Uuid::nil(),
                        Uuid::nil(),
                        padu_client::Command::CreateTask {
                            task: CreateTask {
                                project_id,
                                title,
                                description,
                                labels: vec![],
                                assigned_agent,
                                model,
                            },
                        },
                    )?;
                    let padu_client::ResponsePayload::TaskCreated { task } = response else {
                        anyhow::bail!("daemon returned an invalid TaskCreated response");
                    };
                    Ok::<_, anyhow::Error>(task)
                })
                .await;
            let _ = padu.update(cx, |this, cx| match result {
                Ok(created) => {
                    this.board_tasks.insert(0, TaskSummary::from_task(&created));
                    this.show_success_toast(tr!("board.create_task"));
                    cx.notify();
                }
                Err(error) => {
                    this.show_toast(format!("Failed to create task: {error}"));
                    this.load_board_tasks_from_daemon(cx);
                }
            });
        })
        .detach();
    }

    pub(super) fn board_pane_content(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_board_page(window, cx)
    }

    pub(super) fn render_board_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let search_query = self.board_search.read(cx).content().trim().to_lowercase();
        let selected_task_id = self.board_selected_task_id;

        // Filter tasks
        let filtered_tasks: Vec<TaskSummary> = self
            .board_tasks
            .iter()
            .filter(|t| {
                if let Some(project_id) = self.board_filter_project {
                    if t.project_id != project_id {
                        return false;
                    }
                }
                if let Some(agent) = self.board_filter_agent {
                    if t.assigned_agent != Some(agent) {
                        return false;
                    }
                }
                if self.board_filter_needs_attention && !t.needs_attention {
                    return false;
                }
                if self.board_filter_sync_failed && t.sync_failed.is_none() {
                    return false;
                }
                if !search_query.is_empty() {
                    let matches_title = t.title.to_lowercase().contains(&search_query);
                    let matches_desc = t.description_preview.to_lowercase().contains(&search_query);
                    if !matches_title && !matches_desc {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // Root container
        let mut page = div()
            .id("board-page")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.canvas);

        // Header toolbar
        page = page.child(self.render_board_header(cx));

        // Content area: 5 columns + detail flyout
        let mut columns_container = div()
            .id("board-columns-container")
            .flex_1()
            .min_h_0()
            .flex()
            .overflow_x_scroll()
            .p(px(16.0))
            .gap(px(14.0));

        for status in BOARD_COLUMNS {
            let column_tasks = filtered_tasks
                .iter()
                .filter(|t| t.status == status)
                .cloned()
                .collect::<Vec<_>>();
            columns_container = columns_container.child(self.render_board_column(
                status,
                column_tasks,
                selected_task_id,
                &theme,
                window,
                cx,
            ));
        }

        let mut main_content = div()
            .id("board-main-content")
            .flex_1()
            .min_h_0()
            .flex()
            .relative()
            .child(columns_container);

        // Detail Flyout Drawer
        if let Some(task_id) = selected_task_id {
            main_content =
                main_content.child(self.render_task_detail_drawer(task_id, &theme, window, cx));
        }

        page = page.child(main_content);

        // Modal for New Task
        if self.board_new_task_modal_open {
            page = page.child(self.render_new_task_modal(&theme, window, cx));
        }

        page.into_any_element()
    }

    fn render_board_header(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = Theme::current(cx);
        let has_filter = self.board_filter_project.is_some()
            || self.board_filter_agent.is_some()
            || self.board_filter_needs_attention
            || self.board_filter_sync_failed
            || !self.board_search.read(cx).content().trim().is_empty();

        let project_handle = self.menu_handle("board-filter-project", cx);
        let current_project_id = self.board_filter_project;
        let current_project_name = current_project_id
            .and_then(|id| self.state.projects.iter().find(|p| p.id == id))
            .map(|p| p.display_name())
            .unwrap_or_else(|| tr!("board.all_projects"));
        let project_options = self
            .state
            .projects
            .iter()
            .filter(|p| !p.is_projectless())
            .map(|p| (p.id, p.display_name()))
            .collect::<Vec<_>>();
        let weak_project = cx.entity().downgrade();
        let project_filter_selector = dropdown_menu(
            MenuChip::new("board-filter-project-chip")
                .icon("icons/folder.svg", theme.text_tertiary)
                .label(current_project_name)
                .outlined()
                .background(theme.inset)
                .height(px(28.0))
                .selected(project_handle.is_open()),
            "board-filter-project-menu",
            &project_handle,
            MenuAlign::BelowLeft,
            move |_| {
                let mut items = Vec::new();
                let weak = weak_project.clone();
                items.push(
                    MenuItem::new(tr!("board.all_projects"), move |_, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.board_filter_project = None;
                            cx.notify();
                        });
                    })
                    .icon("icons/folder.svg")
                    .selected(current_project_id.is_none()),
                );
                if !project_options.is_empty() {
                    items.push(MenuItem::Separator);
                }
                for (pid, pname) in project_options.clone() {
                    let weak = weak_project.clone();
                    let is_sel = current_project_id == Some(pid);
                    items.push(
                        MenuItem::new(pname, move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.board_filter_project = Some(pid);
                                cx.notify();
                            });
                        })
                        .icon("icons/folder.svg")
                        .selected(is_sel),
                    );
                }
                items
            },
        );

        let agent_handle = self.menu_handle("board-filter-agent", cx);
        let current_agent = self.board_filter_agent;
        let current_agent_name = current_agent
            .map(|a| a.short_name().to_string())
            .unwrap_or_else(|| tr!("board.all_agents"));
        let weak_agent = cx.entity().downgrade();
        let agent_filter_selector = dropdown_menu(
            MenuChip::new("board-filter-agent-chip")
                .icon(
                    match current_agent {
                        Some(a) => provider_icon(a),
                        None => "icons/bot.svg",
                    },
                    if current_agent.is_some() {
                        theme.accent
                    } else {
                        theme.text_tertiary
                    },
                )
                .label(current_agent_name)
                .outlined()
                .background(theme.inset)
                .height(px(28.0))
                .selected(agent_handle.is_open()),
            "board-filter-agent-menu",
            &agent_handle,
            MenuAlign::BelowLeft,
            move |_| {
                let mut items = Vec::new();
                let weak = weak_agent.clone();
                items.push(
                    MenuItem::new(tr!("board.all_agents"), move |_, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.board_filter_agent = None;
                            cx.notify();
                        });
                    })
                    .icon("icons/bot.svg")
                    .selected(current_agent.is_none()),
                );
                items.push(MenuItem::Separator);
                for p in [
                    ProviderKind::Agy,
                    ProviderKind::Claude,
                    ProviderKind::Codex,
                    ProviderKind::Cursor,
                    ProviderKind::DeepSeek,
                    ProviderKind::OpenCode,
                ] {
                    let weak = weak_agent.clone();
                    let is_sel = current_agent == Some(p);
                    items.push(
                        MenuItem::new(p.short_name(), move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.board_filter_agent = Some(p);
                                cx.notify();
                            });
                        })
                        .icon(provider_icon(p))
                        .selected(is_sel),
                    );
                }
                items
            },
        );

        let header = div()
            .id("board-header")
            .h(px(48.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px(px(16.0))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .when(!self.sidebar_visible, |element| {
                        element
                            .child(
                                self.window_drag_region(
                                    div()
                                        .id("board-traffic-light-drag-region")
                                        .w(px((TRAFFIC_LIGHT_CLEARANCE - 8.0).max(0.0)))
                                        .h_full()
                                        .flex_none(),
                                    cx,
                                ),
                            )
                            .child(self.render_sidebar_navigation_controls(true, cx))
                    })
                    .child(
                        div()
                            .text_size(sp(15.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(tr!("board.title")),
                    )
                    // Search field
                    .child(
                        div()
                            .w(px(180.0))
                            .h(px(28.0))
                            .rounded(px(6.0))
                            .bg(theme.inset)
                            .child(
                                TextField::new("board-search", self.board_search.clone())
                                    .icon("icons/search.svg", 12.0),
                            ),
                    )
                    // Project dropdown filter
                    .child(project_filter_selector)
                    // Agent dropdown filter
                    .child(agent_filter_selector)
                    // Needs Attention chip
                    .child({
                        let active = self.board_filter_needs_attention;
                        div()
                            .id("board-filter-attention")
                            .h(px(26.0))
                            .px(px(8.0))
                            .rounded(px(6.0))
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .cursor_pointer()
                            .text_size(sp(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .bg(if active {
                                theme.warning.opacity(0.18)
                            } else {
                                theme.inset
                            })
                            .text_color(if active {
                                theme.warning
                            } else {
                                theme.text_secondary
                            })
                            .border_1()
                            .border_color(if active {
                                theme.warning.opacity(0.4)
                            } else {
                                theme.border
                            })
                            .child(icon(
                                "icons/alert.svg",
                                12.0,
                                if active {
                                    theme.warning
                                } else {
                                    theme.text_tertiary
                                },
                            ))
                            .child(tr!("board.needs_attention"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.board_filter_needs_attention =
                                    !this.board_filter_needs_attention;
                                cx.notify();
                            }))
                    })
                    // Sync Failed chip
                    .child({
                        let active = self.board_filter_sync_failed;
                        div()
                            .id("board-filter-sync-failed")
                            .h(px(26.0))
                            .px(px(8.0))
                            .rounded(px(6.0))
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .cursor_pointer()
                            .text_size(sp(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .bg(if active {
                                theme.danger.opacity(0.18)
                            } else {
                                theme.inset
                            })
                            .text_color(if active {
                                theme.danger
                            } else {
                                theme.text_secondary
                            })
                            .border_1()
                            .border_color(if active {
                                theme.danger.opacity(0.4)
                            } else {
                                theme.border
                            })
                            .child(icon(
                                "icons/block.svg",
                                12.0,
                                if active {
                                    theme.danger
                                } else {
                                    theme.text_tertiary
                                },
                            ))
                            .child(tr!("board.sync_failed"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.board_filter_sync_failed = !this.board_filter_sync_failed;
                                cx.notify();
                            }))
                    })
                    // Clear filters button if active
                    .when(has_filter, |element| {
                        element.child(
                            div()
                                .id("board-clear-filters")
                                .h(px(24.0))
                                .px(px(6.0))
                                .rounded(px(4.0))
                                .cursor_pointer()
                                .text_size(sp(11.0))
                                .text_color(theme.text_tertiary)
                                .hover(|s| s.text_color(theme.text))
                                .child(tr!("board.clear_filters"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.board_filter_project = None;
                                    this.board_filter_agent = None;
                                    this.board_filter_needs_attention = false;
                                    this.board_filter_sync_failed = false;
                                    this.board_search.update(cx, |s, cx| s.set_content("", cx));
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // Refresh button
                    .child(
                        div()
                            .id("board-refresh")
                            .h(px(28.0))
                            .w(px(28.0))
                            .rounded(px(6.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .bg(theme.inset)
                            .text_color(theme.text_secondary)
                            .hover(|s| s.bg(theme.overlay_strong).text_color(theme.text))
                            .child(icon("icons/rotate-cw.svg", 14.0, theme.text_secondary))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.load_board_tasks_from_daemon(cx);
                            })),
                    )
                    // New Task button
                    .child(
                        div()
                            .id("board-new-task-button")
                            .h(px(28.0))
                            .px(px(10.0))
                            .rounded(px(6.0))
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .cursor_pointer()
                            .bg(theme.inverse)
                            .text_color(theme.on_inverse)
                            .text_size(sp(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .hover(|s| s.opacity(0.92))
                            .child(icon("icons/plus.svg", 13.0, theme.on_inverse))
                            .child(tr!("board.new_task"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.board_new_task_modal_open = true;
                                this.board_new_task_title
                                    .update(cx, |t, cx| t.set_content("", cx));
                                this.board_new_task_description
                                    .update(cx, |t, cx| t.set_content("", cx));
                                this.board_new_task_project_id = this
                                    .board_filter_project
                                    .or_else(|| this.current_project_id());
                                this.board_new_task_agent = None;
                                this.board_new_task_model = None;
                                cx.notify();
                            })),
                    ),
            );

        header
    }

    fn render_board_column(
        &self,
        status: TaskStatus,
        tasks: Vec<TaskSummary>,
        selected_task_id: Option<Uuid>,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let status_color = task_status_color(status, theme);
        let label = task_status_label(status);
        let count = tasks.len();

        let is_collapsed = self.board_collapsed_columns.contains(&status);
        if is_collapsed {
            let status_for_toggle = status;
            return div()
                .id(SharedString::from(format!(
                    "board-col-{:?}-collapsed",
                    status
                )))
                .w(px(40.0))
                .min_w(px(40.0))
                .h_full()
                .flex()
                .flex_col()
                .items_center()
                .py(px(10.0))
                .gap(px(10.0))
                .rounded(px(10.0))
                .bg(theme.surface)
                .border_1()
                .border_color(theme.border)
                .cursor_pointer()
                .hover(|s| s.bg(theme.inset))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.board_collapsed_columns.remove(&status_for_toggle);
                    cx.notify();
                }))
                .child(
                    div()
                        .w(px(24.0))
                        .h(px(24.0))
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .hover(|s| s.bg(theme.overlay_strong))
                        .child(icon("icons/chevron-right.svg", 13.0, theme.text_secondary)),
                )
                .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(status_color))
                .child(
                    div()
                        .h(px(20.0))
                        .px(px(6.0))
                        .rounded_full()
                        .bg(theme.inset)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(sp(11.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_secondary)
                        .child(count.to_string()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(2.0))
                        .mt(px(4.0))
                        .children(label.chars().map(|c| {
                            div()
                                .text_size(sp(10.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text_tertiary)
                                .child(c.to_string())
                        })),
                );
        }

        let mut column = div()
            .id(SharedString::from(format!("board-col-{:?}", status)))
            .w(px(280.0))
            .min_w(px(280.0))
            .h_full()
            .flex()
            .flex_col()
            .rounded(px(10.0))
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border);

        // Column Header
        let status_for_collapse = status;
        let col_header = div()
            .h(px(40.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px(px(12.0))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(status_color))
                    .child(
                        div()
                            .text_size(sp(13.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(label),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .h(px(20.0))
                            .px(px(6.0))
                            .rounded_full()
                            .bg(theme.inset)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(sp(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_secondary)
                            .child(count.to_string()),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("collapse-col-{:?}", status)))
                            .w(px(20.0))
                            .h(px(20.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|s| s.bg(theme.overlay_strong))
                            .child(icon("icons/chevron-left.svg", 12.0, theme.text_tertiary))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.board_collapsed_columns.insert(status_for_collapse);
                                cx.notify();
                            })),
                    ),
            );

        column = column.child(col_header);

        // Column Body (Card list)
        let mut card_list = div()
            .id(SharedString::from(format!("board-col-cards-{:?}", status)))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(8.0))
            .flex()
            .flex_col()
            .gap(px(8.0));

        if tasks.is_empty() {
            card_list = card_list.child(
                div()
                    .py(px(24.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(sp(12.0))
                            .text_color(theme.text_ghost)
                            .child(tr!("board.empty_column")),
                    ),
            );
        } else {
            for task in &tasks {
                card_list = card_list.child(self.render_task_card(
                    task,
                    selected_task_id == Some(task.id),
                    theme,
                    window,
                    cx,
                ));
            }
        }

        column.child(card_list)
    }

    fn render_task_card(
        &self,
        task: &TaskSummary,
        is_selected: bool,
        theme: &Theme,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let task_id = task.id;
        let task_version = task.version;
        let status = task.status;

        let mut card = div()
            .id(SharedString::from(format!("task-card-{}", task.id)))
            .p(px(10.0))
            .rounded(px(8.0))
            .bg(if is_selected {
                theme.raised
            } else {
                theme.inset
            })
            .border_1()
            .border_color(if is_selected {
                theme.accent
            } else {
                theme.border
            })
            .shadow_sm()
            .cursor_pointer()
            .hover(|s| s.border_color(theme.accent.opacity(0.8)))
            .flex()
            .flex_col()
            .gap(px(6.0))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_board_task(Some(task_id), cx);
            }));

        // Title
        card = card.child(
            div()
                .text_size(sp(13.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text)
                .child(task.title.clone()),
        );

        // Description preview
        if !task.description_preview.is_empty() {
            card = card.child(
                div()
                    .text_size(sp(11.5))
                    .text_color(theme.text_secondary)
                    .child(task.description_preview.clone()),
            );
        }

        // Flags & Badges row
        let mut badges_row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .mt(px(2.0));

        // Assigned Agent Badge
        if let Some(agent) = task.assigned_agent {
            badges_row = badges_row.child(
                div()
                    .h(px(18.0))
                    .px(px(5.0))
                    .rounded(px(4.0))
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(icon(provider_icon(agent), 11.0, theme.text_secondary))
                    .child(
                        div()
                            .text_size(sp(10.5))
                            .text_color(theme.text_secondary)
                            .child(agent.short_name()),
                    ),
            );
        }

        // Model Badge
        if let Some(model) = &task.model {
            badges_row = badges_row.child(
                div()
                    .h(px(18.0))
                    .px(px(5.0))
                    .rounded(px(4.0))
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap(px(3.0))
                    .child(icon("icons/sparkle.svg", 10.0, theme.text_tertiary))
                    .child(
                        div()
                            .text_size(sp(10.0))
                            .text_color(theme.text_tertiary)
                            .child(model.clone()),
                    ),
            );
        }

        // Needs Attention Badge
        if task.needs_attention {
            badges_row = badges_row.child(
                div()
                    .h(px(18.0))
                    .px(px(5.0))
                    .rounded(px(4.0))
                    .bg(theme.warning.opacity(0.15))
                    .border_1()
                    .border_color(theme.warning.opacity(0.3))
                    .flex()
                    .items_center()
                    .gap(px(3.0))
                    .child(icon("icons/alert.svg", 10.0, theme.warning))
                    .child(
                        div()
                            .text_size(sp(10.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.warning)
                            .child(tr!("board.needs_attention")),
                    ),
            );
        }

        // Sync Failed Badge
        if task.sync_failed.is_some() {
            badges_row = badges_row.child(
                div()
                    .h(px(18.0))
                    .px(px(5.0))
                    .rounded(px(4.0))
                    .bg(theme.danger.opacity(0.15))
                    .border_1()
                    .border_color(theme.danger.opacity(0.3))
                    .flex()
                    .items_center()
                    .gap(px(3.0))
                    .child(icon("icons/block.svg", 10.0, theme.danger))
                    .child(
                        div()
                            .text_size(sp(10.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.danger)
                            .child(tr!("board.sync_failed")),
                    ),
            );
        }

        // Linked session indicator
        if task.session_id.is_some() {
            badges_row = badges_row.child(
                div()
                    .h(px(18.0))
                    .px(px(4.0))
                    .rounded(px(4.0))
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(icon("icons/external-link.svg", 10.0, theme.text_tertiary))
                    .child(
                        div()
                            .text_size(sp(10.0))
                            .text_color(theme.text_tertiary)
                            .child(tr!("board.linked_session")),
                    ),
            );
        }

        card = card.child(badges_row);

        // Quick action button
        let quick_action = match status {
            TaskStatus::Backlog => Some((
                tr!("board.queue_action"),
                "icons/arrow-right.svg",
                TaskStatus::Queued,
            )),
            TaskStatus::Review => {
                Some((tr!("board.mark_done"), "icons/check.svg", TaskStatus::Done))
            }
            TaskStatus::Done => Some((
                tr!("board.reopen"),
                "icons/rotate-cw.svg",
                TaskStatus::Backlog,
            )),
            _ => None,
        };

        if let Some((label, icon_path, next_status)) = quick_action {
            card = card.child(
                div().flex().justify_end().mt(px(4.0)).child(
                    div()
                        .id(SharedString::from(format!("task-action-{}", task.id)))
                        .h(px(22.0))
                        .px(px(7.0))
                        .rounded(px(4.0))
                        .bg(theme.surface)
                        .border_1()
                        .border_color(theme.border)
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .text_size(sp(11.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_secondary)
                        .hover(|s| s.bg(theme.overlay_strong).text_color(theme.text))
                        .child(icon(icon_path, 10.0, theme.text_secondary))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.move_board_task(task_id, next_status, task_version, cx);
                            cx.stop_propagation();
                        })),
                ),
            );
        }

        card
    }

    fn render_task_detail_drawer(
        &mut self,
        task_id: Uuid,
        theme: &Theme,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let task_summary = self.board_tasks.iter().find(|t| t.id == task_id).cloned();
        let hydrated = self.board_hydrated_task.clone();

        let title = hydrated
            .as_ref()
            .map(|t| t.title.clone())
            .or_else(|| task_summary.as_ref().map(|t| t.title.clone()))
            .unwrap_or_default();
        let status = hydrated
            .as_ref()
            .map(|t| t.status)
            .or_else(|| task_summary.as_ref().map(|t| t.status))
            .unwrap_or_default();
        let assigned_agent = hydrated
            .as_ref()
            .and_then(|t| t.assigned_agent)
            .or_else(|| task_summary.as_ref().and_then(|t| t.assigned_agent));
        let session_id = hydrated
            .as_ref()
            .and_then(|t| t.session_id)
            .or_else(|| task_summary.as_ref().and_then(|t| t.session_id));
        let needs_attention = hydrated
            .as_ref()
            .map(|t| t.needs_attention)
            .or_else(|| task_summary.as_ref().map(|t| t.needs_attention))
            .unwrap_or_default();
        let sync_failed = hydrated
            .as_ref()
            .and_then(|t| t.sync_failed.clone())
            .or_else(|| task_summary.as_ref().and_then(|t| t.sync_failed.clone()));
        let version = hydrated
            .as_ref()
            .map(|t| t.version)
            .or_else(|| task_summary.as_ref().map(|t| t.version))
            .unwrap_or_default();
        let model = hydrated
            .as_ref()
            .and_then(|t| t.model.clone())
            .or_else(|| task_summary.as_ref().and_then(|t| t.model.clone()));
        let description = hydrated
            .as_ref()
            .map(|t| t.description.clone())
            .or_else(|| task_summary.as_ref().map(|t| t.description_preview.clone()))
            .unwrap_or_default();

        let mut drawer = div()
            .id("task-detail-drawer")
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .w(px(380.0))
            .bg(theme.raised)
            .border_l_1()
            .border_color(theme.border_strong)
            .shadow_xl()
            .flex()
            .flex_col()
            .p(px(16.0))
            .gap(px(14.0));

        // Drawer Header: Close button and Title
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(task_status_color(status, theme)),
                    )
                    .child(
                        div()
                            .text_size(sp(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_secondary)
                            .child(task_status_label(status)),
                    ),
            )
            .child(
                div()
                    .id("task-detail-close")
                    .w(px(24.0))
                    .h(px(24.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|s| s.bg(theme.overlay_strong))
                    .child(icon("icons/slash.svg", 14.0, theme.text_secondary))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_board_task(None, cx);
                    })),
            );

        drawer = drawer.child(header);

        // Task Title
        drawer = drawer.child(
            div()
                .text_size(sp(16.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text)
                .child(title.clone()),
        );

        // Status Changer Buttons Row
        let mut status_row = div().flex().flex_wrap().gap(px(6.0));
        for s in BOARD_COLUMNS {
            let is_current = s == status;
            let is_running = s == TaskStatus::Running;
            let s_label = task_status_label(s);
            let s_color = task_status_color(s, theme);

            status_row = status_row.child(
                div()
                    .id(SharedString::from(format!("move-status-{:?}", s)))
                    .h(px(24.0))
                    .px(px(8.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .text_size(sp(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .when(is_current, |btn| {
                        btn.bg(s_color.opacity(0.18))
                            .text_color(s_color)
                            .border_1()
                            .border_color(s_color.opacity(0.4))
                    })
                    .when(!is_current && !is_running, |btn| {
                        btn.bg(theme.surface)
                            .text_color(theme.text_secondary)
                            .border_1()
                            .border_color(theme.border)
                            .cursor_pointer()
                            .hover(|st| st.bg(theme.overlay_strong).text_color(theme.text))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.move_board_task(task_id, s, version, cx);
                            }))
                    })
                    .when(is_running && !is_current, |btn| {
                        btn.bg(theme.inset)
                            .text_color(theme.text_ghost)
                            .border_1()
                            .border_color(theme.border)
                            .cursor_not_allowed()
                    })
                    .child(s_label),
            );
        }

        drawer = drawer.child(status_row);

        // Info Badges: Agent & Session
        let mut info_section = div()
            .p(px(10.0))
            .rounded(px(8.0))
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(8.0));

        // Agent row
        info_section = info_section.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(sp(12.0))
                        .text_color(theme.text_secondary)
                        .child(tr!("board.assigned_agent")),
                )
                .child(div().flex().items_center().gap(px(4.0)).child(
                    if let Some(agent) = assigned_agent {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(icon(provider_icon(agent), 12.0, theme.text))
                            .child(
                                div()
                                    .text_size(sp(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(agent.display_name()),
                            )
                    } else {
                        div()
                            .text_size(sp(12.0))
                            .text_color(theme.text_tertiary)
                            .child(tr!("board.no_agent"))
                    },
                )),
        );

        // Model row
        if let Some(m) = model {
            info_section = info_section.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(sp(12.0))
                            .text_color(theme.text_secondary)
                            .child(tr!("board.model")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(icon("icons/sparkle.svg", 11.0, theme.text_secondary))
                            .child(
                                div()
                                    .text_size(sp(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(m),
                            ),
                    ),
            );
        }

        // Linked Session row
        if let Some(sid) = session_id {
            info_section = info_section.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(sp(12.0))
                            .text_color(theme.text_secondary)
                            .child(tr!("board.linked_session")),
                    )
                    .child(
                        div()
                            .id("board-open-session-chat")
                            .h(px(24.0))
                            .px(px(8.0))
                            .rounded(px(4.0))
                            .bg(theme.accent.opacity(0.12))
                            .border_1()
                            .border_color(theme.accent.opacity(0.3))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .text_size(sp(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.accent)
                            .hover(|s| s.bg(theme.accent.opacity(0.2)))
                            .child(icon("icons/external-link.svg", 11.0, theme.accent))
                            .child(tr!("board.open_in_chat"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_session(sid, cx);
                            })),
                    ),
            );
        }

        drawer = drawer.child(info_section);

        // Alert banners
        if needs_attention {
            drawer = drawer.child(
                div()
                    .p(px(8.0))
                    .rounded(px(6.0))
                    .bg(theme.warning.opacity(0.12))
                    .border_1()
                    .border_color(theme.warning.opacity(0.3))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(icon("icons/alert.svg", 13.0, theme.warning))
                    .child(
                        div()
                            .text_size(sp(11.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.warning)
                            .child(tr!("board.needs_attention")),
                    ),
            );
        }

        if let Some(err) = sync_failed {
            drawer = drawer.child(
                div()
                    .p(px(8.0))
                    .rounded(px(6.0))
                    .bg(theme.danger.opacity(0.12))
                    .border_1()
                    .border_color(theme.danger.opacity(0.3))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(icon("icons/block.svg", 13.0, theme.danger))
                            .child(
                                div()
                                    .text_size(sp(11.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.danger)
                                    .child(tr!("board.sync_failed")),
                            ),
                    )
                    .child(
                        div()
                            .text_size(sp(11.0))
                            .text_color(theme.text_secondary)
                            .child(err),
                    ),
            );
        }

        // Markdown Description
        let palette = MarkdownPalette::from_theme(theme);
        let cache_key = format!("task:{}", task_id);
        let mut cache = self.board_hydrated_markdown.borrow_mut();
        if !matches!(cache.as_ref(), Some((key, _)) if key == &cache_key) {
            *cache = Some((cache_key.clone(), MarkdownView::new()));
        }
        let (_, view) = cache.as_mut().expect("task markdown cache entry");
        view.set_text(&description, false);
        let md_ctx = MarkdownCtx::new(
            format!("task-desc-{}", task_id),
            &palette,
            self.scaled_markdown_metrics(MarkdownMetrics::BODY),
            self.board_hydrated_selection.clone(),
        );
        let document = crate::md::render::markdown(view, &md_ctx);
        drop(cache);

        let selection_input = canvas(|_, _, _| (), {
            let selection = self.board_hydrated_selection.clone();
            move |_, _, window, _| crate::md::render::install_selection_input(window, &selection)
        })
        .absolute()
        .w(px(0.0))
        .h(px(0.0));

        let desc_pane = div()
            .flex_1()
            .min_h_0()
            .relative()
            .rounded(px(8.0))
            .bg(theme.inset)
            .child(
                div()
                    .id("task-desc-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.board_hydrated_scroll_handle)
                    .p(px(12.0))
                    .child(crate::md::render::frame_reset(
                        self.board_hydrated_selection.clone(),
                    ))
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_normal()
                            .children(document),
                    )
                    .child(selection_input),
            )
            .child(scrollbar::vertical(
                &self.board_hydrated_scroll_handle,
                &self.board_hydrated_scrollbar,
            ));

        drawer = drawer.child(desc_pane);

        // Delete button at bottom
        let title_for_delete = title.clone();
        drawer = drawer.child(
            div().flex().justify_end().child(
                div()
                    .id("task-delete-button")
                    .h(px(28.0))
                    .px(px(10.0))
                    .rounded(px(6.0))
                    .bg(theme.danger.opacity(0.12))
                    .border_1()
                    .border_color(theme.danger.opacity(0.3))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .text_size(sp(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.danger)
                    .hover(|s| s.bg(theme.danger.opacity(0.2)))
                    .child(icon("icons/trash.svg", 12.0, theme.danger))
                    .child(tr!("board.delete_task"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.confirm_delete_task(
                            task_id,
                            title_for_delete.clone(),
                            version,
                            window,
                            cx,
                        );
                    })),
            ),
        );

        drawer
    }

    fn render_new_task_modal(
        &mut self,
        theme: &Theme,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cancel_focus = self.board_new_task_cancel_focus.clone();
        let title_input = self.board_new_task_title.clone();
        let desc_input = self.board_new_task_description.clone();
        let project_id = self
            .board_new_task_project_id
            .or_else(|| self.current_project_id())
            .unwrap_or_else(Uuid::nil);
        let agent = self.board_new_task_agent;

        let project_modal_handle = self.menu_handle("board-new-task-project", cx);
        let current_modal_project_id = self
            .board_new_task_project_id
            .or_else(|| self.current_project_id());
        let current_modal_project_name = current_modal_project_id
            .and_then(|id| self.state.projects.iter().find(|p| p.id == id))
            .map(|p| p.display_name())
            .unwrap_or_else(|| tr!("board.all_projects"));
        let modal_project_options = self
            .state
            .projects
            .iter()
            .filter(|p| !p.is_projectless())
            .map(|p| (p.id, p.display_name()))
            .collect::<Vec<_>>();
        let weak_modal_project = cx.entity().downgrade();
        let modal_project_selector = dropdown_menu(
            MenuChip::new("modal-project-chip")
                .icon("icons/folder.svg", theme.text_tertiary)
                .label(current_modal_project_name)
                .outlined()
                .background(theme.inset)
                .height(px(28.0))
                .selected(project_modal_handle.is_open())
                .w_full()
                .justify_between(),
            "modal-project-menu",
            &project_modal_handle,
            MenuAlign::BelowLeft,
            move |_| {
                let mut items = Vec::new();
                for (pid, pname) in modal_project_options.clone() {
                    let weak = weak_modal_project.clone();
                    let is_sel = current_modal_project_id == Some(pid);
                    items.push(
                        MenuItem::new(pname, move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.board_new_task_project_id = Some(pid);
                                cx.notify();
                            });
                        })
                        .icon("icons/folder.svg")
                        .selected(is_sel),
                    );
                }
                items
            },
        );

        let card =
            dialog_card("board-new-task-dialog", theme, px(480.0))
                .child(
                    div()
                        .text_size(sp(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text)
                        .child(tr!("board.create_task")),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.0))
                        // Project selector
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_size(sp(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text_secondary)
                                        .child(tr!("board.filter_project")),
                                )
                                .child(modal_project_selector),
                        )
                        // Title field
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_size(sp(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text_secondary)
                                        .child(tr!("board.task_title")),
                                )
                                .child(
                                    div().h(px(32.0)).rounded(px(6.0)).bg(theme.inset).child(
                                        TextField::new("new-task-title", title_input.clone()),
                                    ),
                                ),
                        )
                        // Description field
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_size(sp(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text_secondary)
                                        .child(tr!("board.task_description")),
                                )
                                .child(div().h(px(100.0)).rounded(px(6.0)).bg(theme.inset).child(
                                    TextField::new("new-task-description", desc_input.clone()),
                                )),
                        )
                        // Agent selector chips
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_size(sp(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text_secondary)
                                        .child(tr!("board.assigned_agent")),
                                )
                                .child({
                                    let mut row = div().flex().flex_wrap().gap(px(6.0));
                                    // Unassigned chip
                                    row = row.child(
                                        div()
                                            .id("board-new-task-agent-none")
                                            .h(px(24.0))
                                            .px(px(8.0))
                                            .rounded(px(4.0))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .text_size(sp(11.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .when(agent.is_none(), |chip| {
                                                chip.bg(theme.accent.opacity(0.18))
                                                    .text_color(theme.accent)
                                                    .border_1()
                                                    .border_color(theme.accent.opacity(0.4))
                                            })
                                            .when(agent.is_some(), |chip| {
                                                chip.bg(theme.inset)
                                                    .text_color(theme.text_secondary)
                                                    .border_1()
                                                    .border_color(theme.border)
                                            })
                                            .child(tr!("board.no_agent"))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.board_new_task_agent = None;
                                                this.board_new_task_model = None;
                                                cx.notify();
                                            })),
                                    );
                                    for p in [
                                        ProviderKind::Agy,
                                        ProviderKind::Claude,
                                        ProviderKind::Codex,
                                        ProviderKind::Cursor,
                                        ProviderKind::DeepSeek,
                                        ProviderKind::OpenCode,
                                    ] {
                                        let selected = agent == Some(p);
                                        let p_id = p.id();
                                        row = row.child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "board-new-task-agent-{}",
                                                    p_id
                                                )))
                                                .h(px(24.0))
                                                .px(px(8.0))
                                                .rounded(px(4.0))
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .gap(px(4.0))
                                                .text_size(sp(11.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .when(selected, |chip| {
                                                    chip.bg(theme.accent.opacity(0.18))
                                                        .text_color(theme.accent)
                                                        .border_1()
                                                        .border_color(theme.accent.opacity(0.4))
                                                })
                                                .when(!selected, |chip| {
                                                    chip.bg(theme.inset)
                                                        .text_color(theme.text_secondary)
                                                        .border_1()
                                                        .border_color(theme.border)
                                                })
                                                .child(icon(
                                                    provider_icon(p),
                                                    11.0,
                                                    if selected {
                                                        theme.accent
                                                    } else {
                                                        theme.text_secondary
                                                    },
                                                ))
                                                .child(p.short_name())
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.board_new_task_agent = Some(p);
                                                    this.board_new_task_model = None;
                                                    cx.notify();
                                                })),
                                        );
                                    }
                                    row
                                }),
                        )
                        // Model selector chips (only when agent is selected)
                        .when_some(agent, |element, p| {
                            let models = self.board_models_for_provider(p);
                            if models.is_empty() {
                                return element;
                            }
                            let current_model = self.board_new_task_model.clone();
                            let default_selected = current_model.is_none();
                            let mut model_chips = div().flex().flex_wrap().gap(px(6.0));

                            // Default model chip
                            model_chips = model_chips.child(
                                div()
                                    .id("board-new-task-model-default")
                                    .h(px(24.0))
                                    .px(px(8.0))
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .text_size(sp(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .when(default_selected, |chip| {
                                        chip.bg(theme.accent.opacity(0.18))
                                            .text_color(theme.accent)
                                            .border_1()
                                            .border_color(theme.accent.opacity(0.4))
                                    })
                                    .when(!default_selected, |chip| {
                                        chip.bg(theme.inset)
                                            .text_color(theme.text_secondary)
                                            .border_1()
                                            .border_color(theme.border)
                                    })
                                    .child(tr!("board.default_model"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.board_new_task_model = None;
                                        cx.notify();
                                    })),
                            );

                            for (m_id, m_name) in models {
                                let is_selected = current_model.as_deref() == Some(&m_id);
                                let m_id_for_click = m_id.clone();
                                model_chips = model_chips.child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "board-new-task-model-{}",
                                            m_id
                                        )))
                                        .h(px(24.0))
                                        .px(px(8.0))
                                        .rounded(px(4.0))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .text_size(sp(11.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .when(is_selected, |chip| {
                                            chip.bg(theme.accent.opacity(0.18))
                                                .text_color(theme.accent)
                                                .border_1()
                                                .border_color(theme.accent.opacity(0.4))
                                        })
                                        .when(!is_selected, |chip| {
                                            chip.bg(theme.inset)
                                                .text_color(theme.text_secondary)
                                                .border_1()
                                                .border_color(theme.border)
                                        })
                                        .child(m_name)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.board_new_task_model =
                                                Some(m_id_for_click.clone());
                                            cx.notify();
                                        })),
                                );
                            }

                            element.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .text_size(sp(12.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.text_secondary)
                                            .child(tr!("board.model")),
                                    )
                                    .child(model_chips),
                            )
                        }),
                )
                // Footer
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(8.0))
                        .mt(px(8.0))
                        .child(dialog_cancel_button(
                            "cancel-new-task",
                            tr!("board.cancel"),
                            &cancel_focus,
                            theme,
                            cx,
                            |this, _, cx| {
                                this.board_new_task_modal_open = false;
                                cx.notify();
                            },
                        ))
                        .child({
                            let title_str = title_input.read(cx).content().to_owned();
                            let desc_str = desc_input.read(cx).content().to_owned();
                            div()
                                .id("create-new-task-submit")
                                .h(px(32.0))
                                .px(px(14.0))
                                .rounded(px(7.0))
                                .bg(theme.inverse)
                                .text_color(theme.on_inverse)
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(sp(13.0))
                                .font_weight(FontWeight::MEDIUM)
                                .hover(|s| s.opacity(0.92))
                                .child(tr!("board.create_task"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let model = this.board_new_task_model.clone();
                                    this.create_board_task(
                                        project_id,
                                        title_str.clone(),
                                        desc_str.clone(),
                                        agent,
                                        model,
                                        cx,
                                    );
                                }))
                        }),
                );

        dialog_backdrop(
            "new-task-dialog-backdrop",
            theme,
            cx,
            |this, _, cx| {
                this.board_new_task_modal_open = false;
                cx.notify();
            },
            card,
        )
    }
}
