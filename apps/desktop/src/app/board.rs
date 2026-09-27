use super::*;
use crate::model::ProviderKind;
use crate::theme::{Theme, sp};
use crate::ui::dialog::{dialog_backdrop, dialog_cancel_button, dialog_card};
use crate::ui::text_field::TextField;
use crate::ui::{icon, provider_icon};
use padu_client::kanban::{CreateTask, Task, TaskStatus, TaskSummary};

const BOARD_COLUMNS: [TaskStatus; 5] = [
    TaskStatus::Backlog,
    TaskStatus::Queued,
    TaskStatus::Running,
    TaskStatus::Review,
    TaskStatus::Done,
];

/// Curated assignee shortlist for the new-task modal. The detail drawer
/// offers the full `ProviderKind::ALL` registry like the web client.
const BOARD_MODAL_PROVIDERS: [ProviderKind; 6] = [
    ProviderKind::Agy,
    ProviderKind::Claude,
    ProviderKind::Codex,
    ProviderKind::Cursor,
    ProviderKind::DeepSeek,
    ProviderKind::OpenCode,
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

/// Split the drawer's comma-separated labels input, mirroring the web
/// client's label parsing in `board-page.tsx`.
fn split_board_labels(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Live usage badges for a board card (P1-08).
///
/// The board joins each card's `session_id` to the in-memory `AgentSession`
/// catalog, which already streams `UsageUpdated` / turn events at commit
/// cadence (≤ ~8.3 Hz). Badges therefore refresh with the transcript: no UI
/// polling, no I/O in `render` — a missing session simply means "not known
/// yet" and renders no badges.
///
/// Tokens prefer `context_usage.tokens`, falling back to the Codex
/// `thread_goal` ledger (whichever is larger). Duration sums settled turn
/// spans plus the live running turn via `now`, falling back to the goal's
/// `time_used_seconds` and finally the session wall clock. There is no live
/// USD cost on the wire (costs only exist in the scanned `usage_history`),
/// so tokens are the cost proxy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TaskLiveBadges {
    pub tokens: Option<u64>,
    pub duration_secs: Option<u64>,
}

pub fn task_live_badges(session: Option<&AgentSession>, now: u64) -> TaskLiveBadges {
    let Some(session) = session else {
        return TaskLiveBadges::default();
    };
    let mut tokens = session.context_usage.map(|usage| usage.tokens);
    if let Some(goal) = session.thread_goal.as_ref() {
        let goal_tokens = goal.tokens_used.max(0) as u64;
        tokens = Some(match tokens {
            Some(current) => current.max(goal_tokens),
            None => goal_tokens,
        });
    }
    let tokens = tokens.filter(|count| *count > 0);

    let mut duration_secs: Option<u64> = if session.turns.is_empty() {
        None
    } else {
        Some(
            session
                .turns
                .iter()
                .map(|turn| {
                    turn.completed_at
                        .unwrap_or(now)
                        .saturating_sub(turn.started_at)
                })
                .fold(0u64, |total, span| total.saturating_add(span)),
        )
    };
    if duration_secs.is_none()
        && let Some(goal) = session.thread_goal.as_ref()
        && goal.time_used_seconds > 0
    {
        duration_secs = Some(goal.time_used_seconds.max(0) as u64);
    }
    if duration_secs.is_none() {
        let end = if session.status.is_busy() {
            now
        } else {
            session.last_reply_at.unwrap_or(session.updated_at)
        };
        duration_secs = Some(end.saturating_sub(session.created_at));
    }
    let duration_secs = duration_secs.filter(|secs| *secs > 0);

    TaskLiveBadges {
        tokens,
        duration_secs,
    }
}

/// Which edit surface an assignee chip writes back to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BoardAgentPickTarget {
    NewTask,
    Edit,
}

impl Padu {
    pub(super) fn open_board(&mut self, cx: &mut Context<Self>) {
        self.navigate_workspace_page(WorkspacePage::Board, cx);
        self.ensure_board_tasks_loaded(cx);
        // Assignee pickers mark disabled profiles, so have the registry ready.
        self.ensure_agent_profiles(false, cx);
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
        self.board_edit_agent = None;
        self.board_edit_saving = false;
        self.board_edit_title
            .update(cx, |field, cx| field.set_content("", cx));
        self.board_edit_description
            .update(cx, |field, cx| field.set_content("", cx));
        self.board_edit_labels
            .update(cx, |field, cx| field.set_content("", cx));
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
                        this.board_hydrated_task = Some(task.clone());
                        this.sync_board_edit_fields(&task, cx);
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

    /// Whether the profile registry disables this provider for new work.
    /// Unknown (not yet loaded) reads as enabled here — the daemon remains
    /// the enforcing source of truth and re-resolves at queue time.
    pub(super) fn board_profile_disabled(&self, provider: ProviderKind) -> bool {
        self.agent_profiles
            .iter()
            .find(|profile| profile.agent_id == provider)
            .is_some_and(|profile| !profile.enabled)
    }

    /// Fill the drawer's edit inputs from the hydrated task. Runs only on
    /// hydrate (never while typing), so in-flight edits are never clobbered.
    fn sync_board_edit_fields(&mut self, task: &Task, cx: &mut Context<Self>) {
        self.board_edit_agent = task.assigned_agent;
        self.board_edit_saving = false;
        self.board_edit_title
            .update(cx, |field, cx| field.set_content(&task.title, cx));
        self.board_edit_description
            .update(cx, |field, cx| field.set_content(&task.description, cx));
        self.board_edit_labels.update(cx, |field, cx| {
            field.set_content(&task.labels.join(", "), cx)
        });
    }

    pub(super) fn save_board_task_edits(&mut self, cx: &mut Context<Self>) {
        let Some(hydrated) = self.board_hydrated_task.clone() else {
            return;
        };
        if self.board_edit_saving {
            return;
        }
        let title = self.board_edit_title.read(cx).content().trim().to_owned();
        if title.is_empty() {
            return;
        }
        let description = self
            .board_edit_description
            .read(cx)
            .content()
            .trim()
            .to_owned();
        let labels = split_board_labels(&self.board_edit_labels.read(cx).content());
        let mut updated = hydrated.clone();
        updated.title = title;
        updated.description = description;
        updated.labels = labels;
        updated.assigned_agent = self.board_edit_agent;
        let expected_version = hydrated.version;
        self.board_edit_saving = true;
        cx.notify();

        let daemon = self.daemon.clone();
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let response = daemon.client().request(
                        Uuid::nil(),
                        Uuid::nil(),
                        padu_client::Command::UpdateTask {
                            task: updated,
                            expected_version,
                        },
                    )?;
                    let padu_client::ResponsePayload::TaskUpdated { task } = response else {
                        anyhow::bail!("daemon returned an invalid TaskUpdated response");
                    };
                    Ok::<_, anyhow::Error>(task)
                })
                .await;
            let _ = padu.update(cx, |this, cx| {
                this.board_edit_saving = false;
                match result {
                    Ok(task) => {
                        if let Some(summary) = this.board_tasks.iter_mut().find(|t| t.id == task.id)
                        {
                            *summary = TaskSummary::from_task(&task);
                        }
                        if this.board_selected_task_id == Some(task.id) {
                            this.board_hydrated_task = Some(task.clone());
                            this.sync_board_edit_fields(&task, cx);
                        }
                        this.show_success_toast(tr!("board.save"));
                        cx.notify();
                    }
                    Err(error) => {
                        this.show_toast(format!("Failed to save task: {error}"));
                        this.load_board_tasks_from_daemon(cx);
                    }
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

    fn open_new_task_modal(&mut self, cx: &mut Context<Self>) {
        self.board_new_task_modal_open = true;
        self.board_new_task_title
            .update(cx, |t, cx| t.set_content("", cx));
        self.board_new_task_description
            .update(cx, |t, cx| t.set_content("", cx));
        self.board_new_task_project_id = self
            .board_filter_project
            .or_else(|| self.current_project_id());
        self.board_new_task_agent = None;
        self.board_new_task_model = None;
        // Assignee chips mark disabled profiles, so have the registry ready.
        self.ensure_agent_profiles(false, cx);
        cx.notify();
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

        // Live usage badges, resolved once per frame from the in-memory
        // session catalog (P1-08). The catalog already streams usage/turn
        // events at commit cadence, so cards stay live with no polling and
        // row builders below only do O(1) map lookups — no I/O in `render`.
        let now = unix_time();
        let live_badges: HashMap<Uuid, TaskLiveBadges> = self
            .state
            .sessions
            .iter()
            .map(|session| (session.id, task_live_badges(Some(session), now)))
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
                &live_badges,
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
                            .tab_index(0)
                            .focus_visible(|style| style.border_color(theme.accent))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.board_filter_needs_attention =
                                    !this.board_filter_needs_attention;
                                cx.notify();
                            }))
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    this.board_filter_needs_attention =
                                        !this.board_filter_needs_attention;
                                    cx.notify();
                                    cx.stop_propagation();
                                }
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
                            .tab_index(0)
                            .focus_visible(|style| style.border_color(theme.accent))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.board_filter_sync_failed = !this.board_filter_sync_failed;
                                cx.notify();
                            }))
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    this.board_filter_sync_failed = !this.board_filter_sync_failed;
                                    cx.notify();
                                    cx.stop_propagation();
                                }
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
                                .tab_index(0)
                                .focus_visible(|style| style.border_color(theme.accent))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.board_filter_project = None;
                                    this.board_filter_agent = None;
                                    this.board_filter_needs_attention = false;
                                    this.board_filter_sync_failed = false;
                                    this.board_search.update(cx, |s, cx| s.set_content("", cx));
                                    cx.notify();
                                }))
                                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                        this.board_filter_project = None;
                                        this.board_filter_agent = None;
                                        this.board_filter_needs_attention = false;
                                        this.board_filter_sync_failed = false;
                                        this.board_search.update(cx, |s, cx| s.set_content("", cx));
                                        cx.notify();
                                        cx.stop_propagation();
                                    }
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
                            .tab_index(0)
                            .focus_visible(|style| style.border_color(theme.accent))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.load_board_tasks_from_daemon(cx);
                            }))
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    this.load_board_tasks_from_daemon(cx);
                                    cx.stop_propagation();
                                }
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
                            .tab_index(0)
                            .focus_visible(|style| style.border_color(theme.accent))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_new_task_modal(cx);
                            }))
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    this.open_new_task_modal(cx);
                                    cx.stop_propagation();
                                }
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
        live_badges: &HashMap<Uuid, TaskLiveBadges>,
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
                .tab_index(0)
                .focus_visible(|style| style.border_color(theme.accent))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.board_collapsed_columns.remove(&status_for_toggle);
                    cx.notify();
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        this.board_collapsed_columns.remove(&status_for_toggle);
                        cx.notify();
                        cx.stop_propagation();
                    }
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
                            .tab_index(0)
                            .focus_visible(|style| style.border_color(theme.accent))
                            .child(icon("icons/chevron-left.svg", 12.0, theme.text_tertiary))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.board_collapsed_columns.insert(status_for_collapse);
                                cx.notify();
                            }))
                            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    this.board_collapsed_columns.insert(status_for_collapse);
                                    cx.notify();
                                    cx.stop_propagation();
                                }
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
                let live = live_badges.get(&task.id).copied().unwrap_or_default();
                card_list = card_list.child(self.render_task_card(
                    task,
                    live,
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
        live: TaskLiveBadges,
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
            .tab_index(0)
            .focus_visible(|style| style.border_color(theme.accent))
            .flex()
            .flex_col()
            .gap(px(6.0))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_board_task(Some(task_id), cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    this.select_board_task(Some(task_id), cx);
                    cx.stop_propagation();
                }
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

        // Live usage badges (P1-08): tokens + duration from the linked
        // session's streamed usage events. Icon + text, never color alone.
        if let Some(tokens) = live.tokens {
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
                    .child(icon("icons/zap.svg", 10.0, theme.text_tertiary))
                    .child(
                        div()
                            .text_size(sp(10.0))
                            .text_color(theme.text_secondary)
                            .child(crate::usage::format_tokens(tokens)),
                    ),
            );
        }

        if let Some(duration) = live.duration_secs {
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
                    .child(
                        div()
                            .text_size(sp(10.0))
                            .text_color(theme.text_secondary)
                            .child(super::transcript::format_working_elapsed(duration)),
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
                        .tab_index(0)
                        .focus_visible(|style| style.border_color(theme.accent))
                        .child(icon(icon_path, 10.0, theme.text_secondary))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.move_board_task(task_id, next_status, task_version, cx);
                            cx.stop_propagation();
                        }))
                        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                this.move_board_task(task_id, next_status, task_version, cx);
                                cx.stop_propagation();
                            }
                        })),
                ),
            );
        }

        card
    }

    /// Assignee chip row shared by the new-task modal and the detail drawer.
    /// Disabled profiles render dimmed with a re-enable tooltip and are not
    /// selectable (no tab stop, no action). `id_prefix` keeps element ids
    /// unique per surface; `providers` lets the modal keep its curated six
    /// while the drawer offers the full registry like the web client.
    fn render_board_agent_chips(
        &self,
        selected: Option<ProviderKind>,
        id_prefix: &str,
        target: BoardAgentPickTarget,
        providers: &[ProviderKind],
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut row = div().flex().flex_wrap().gap(px(6.0));
        row = row.child(
            div()
                .id(SharedString::from(format!("{id_prefix}-agent-none")))
                .h(px(24.0))
                .px(px(8.0))
                .rounded(px(4.0))
                .cursor_pointer()
                .flex()
                .items_center()
                .text_size(sp(11.0))
                .font_weight(FontWeight::MEDIUM)
                .tab_index(0)
                .focus_visible(|style| style.border_color(theme.accent))
                .when(selected.is_none(), |chip| {
                    chip.bg(theme.accent.opacity(0.18))
                        .text_color(theme.accent)
                        .border_1()
                        .border_color(theme.accent.opacity(0.4))
                })
                .when(selected.is_some(), |chip| {
                    chip.bg(theme.inset)
                        .text_color(theme.text_secondary)
                        .border_1()
                        .border_color(theme.border)
                })
                .child(tr!("board.no_agent"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    match target {
                        BoardAgentPickTarget::NewTask => {
                            this.board_new_task_agent = None;
                            this.board_new_task_model = None;
                        }
                        BoardAgentPickTarget::Edit => {
                            this.board_edit_agent = None;
                        }
                    }
                    cx.notify();
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        match target {
                            BoardAgentPickTarget::NewTask => {
                                this.board_new_task_agent = None;
                                this.board_new_task_model = None;
                            }
                            BoardAgentPickTarget::Edit => {
                                this.board_edit_agent = None;
                            }
                        }
                        cx.notify();
                        cx.stop_propagation();
                    }
                })),
        );
        for provider in providers {
            let provider = *provider;
            let is_selected = selected == Some(provider);
            let is_disabled = self.board_profile_disabled(provider);
            let mut chip = div()
                .id(SharedString::from(format!(
                    "{id_prefix}-agent-{}",
                    provider.id()
                )))
                .h(px(24.0))
                .px(px(8.0))
                .rounded(px(4.0))
                .flex()
                .items_center()
                .gap(px(4.0))
                .text_size(sp(11.0))
                .font_weight(FontWeight::MEDIUM)
                .when(is_selected, |chip| {
                    chip.bg(theme.accent.opacity(0.18))
                        .text_color(theme.accent)
                        .border_1()
                        .border_color(theme.accent.opacity(0.4))
                })
                .when(!is_selected && !is_disabled, |chip| {
                    chip.bg(theme.inset)
                        .text_color(theme.text_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .cursor_pointer()
                })
                .when(is_disabled, |chip| {
                    chip.bg(theme.inset)
                        .text_color(theme.text_ghost)
                        .border_1()
                        .border_color(theme.border)
                        .opacity(0.55)
                        .tooltip(Tooltip::text(tr!("board.agent_disabled_tooltip")))
                })
                .child(icon(
                    provider_icon(provider),
                    11.0,
                    if is_selected {
                        theme.accent
                    } else {
                        theme.text_secondary
                    },
                ))
                .child(provider.short_name());
            if !is_disabled {
                chip = chip
                    .tab_index(0)
                    .focus_visible(|style| style.border_color(theme.accent))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        match target {
                            BoardAgentPickTarget::NewTask => {
                                this.board_new_task_agent = Some(provider);
                                this.board_new_task_model = None;
                            }
                            BoardAgentPickTarget::Edit => {
                                this.board_edit_agent = Some(provider);
                            }
                        }
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            match target {
                                BoardAgentPickTarget::NewTask => {
                                    this.board_new_task_agent = Some(provider);
                                    this.board_new_task_model = None;
                                }
                                BoardAgentPickTarget::Edit => {
                                    this.board_edit_agent = Some(provider);
                                }
                            }
                            cx.notify();
                            cx.stop_propagation();
                        }
                    }));
            }
            row = row.child(chip);
        }
        row
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
                    .tab_index(0)
                    .focus_visible(|style| style.border_color(theme.accent))
                    .child(icon("icons/slash.svg", 14.0, theme.text_secondary))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_board_task(None, cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space" | "escape") {
                            this.select_board_task(None, cx);
                            cx.stop_propagation();
                        }
                    })),
            );

        drawer = drawer.child(header);

        // Task Title (editable — Save persists via UpdateTask)
        let edit_title = self.board_edit_title.clone();
        drawer = drawer.child(
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
                    div()
                        .min_h(px(32.0))
                        .rounded(px(6.0))
                        .bg(theme.inset)
                        .border_1()
                        .border_color(theme.border)
                        .px(px(8.0))
                        .py(px(4.0))
                        .child(TextField::new("task-edit-title", edit_title)),
                ),
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
                            .tab_index(0)
                            .focus_visible(|style| style.border_color(theme.accent))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.move_board_task(task_id, s, version, cx);
                            }))
                            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    this.move_board_task(task_id, s, version, cx);
                                    cx.stop_propagation();
                                }
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

        // Live usage row (P1-08): same streamed session join as the cards.
        let live = session_id
            .and_then(|sid| self.state.sessions.iter().find(|s| s.id == sid))
            .map(|session| task_live_badges(Some(session), unix_time()))
            .unwrap_or_default();
        if live.tokens.is_some() || live.duration_secs.is_some() {
            let mut value = String::new();
            if let Some(tokens) = live.tokens {
                value.push_str(&crate::usage::format_tokens(tokens));
            }
            if let Some(duration) = live.duration_secs {
                if !value.is_empty() {
                    value.push_str(" · ");
                }
                value.push_str(&super::transcript::format_working_elapsed(duration));
            }
            info_section = info_section.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(sp(12.0))
                            .text_color(theme.text_secondary)
                            .child(tr!("board.live_usage")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(icon("icons/zap.svg", 11.0, theme.text_secondary))
                            .child(
                                div()
                                    .text_size(sp(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(value),
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
                            .tab_index(0)
                            .focus_visible(|style| style.border_color(theme.accent))
                            .child(icon("icons/external-link.svg", 11.0, theme.accent))
                            .child(tr!("board.open_in_chat"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_session(sid, cx);
                            }))
                            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    this.select_session(sid, cx);
                                    cx.stop_propagation();
                                }
                            })),
                    ),
            );
        }

        drawer = drawer.child(info_section);

        // Edit section: assignee chips + labels (title above, description below)
        drawer = drawer.child(
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
                .child(self.render_board_agent_chips(
                    self.board_edit_agent,
                    "task-edit",
                    BoardAgentPickTarget::Edit,
                    &ProviderKind::ALL,
                    theme,
                    cx,
                )),
        );
        let edit_labels = self.board_edit_labels.clone();
        drawer = drawer.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(sp(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_secondary)
                        .child(tr!("board.labels")),
                )
                .child(
                    div()
                        .h(px(32.0))
                        .rounded(px(6.0))
                        .bg(theme.inset)
                        .border_1()
                        .border_color(theme.border)
                        .px(px(8.0))
                        .py(px(4.0))
                        .child(TextField::new("task-edit-labels", edit_labels)),
                ),
        );

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

        // Description editor + Save row (UpdateTask with version guard)
        let edit_description = self.board_edit_description.clone();
        let can_save = hydrated.is_some()
            && !self.board_edit_saving
            && !self.board_edit_title.read(cx).content().trim().is_empty();
        drawer = drawer.child(
            div()
                .flex_1()
                .min_h_0()
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
                .child(
                    div()
                        .flex_1()
                        .min_h(px(96.0))
                        .rounded(px(8.0))
                        .bg(theme.inset)
                        .border_1()
                        .border_color(theme.border)
                        .px(px(8.0))
                        .py(px(6.0))
                        .child(TextField::new("task-edit-description", edit_description)),
                ),
        );

        // Footer: Delete on the left, Save on the right
        let title_for_delete_click = title.clone();
        let title_for_delete_key = title.clone();
        drawer = drawer.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
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
                        .tab_index(0)
                        .focus_visible(|style| style.border_color(theme.danger))
                        .child(icon("icons/trash.svg", 12.0, theme.danger))
                        .child(tr!("board.delete_task"))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.confirm_delete_task(
                                task_id,
                                title_for_delete_click.clone(),
                                version,
                                window,
                                cx,
                            );
                        }))
                        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                this.confirm_delete_task(
                                    task_id,
                                    title_for_delete_key.clone(),
                                    version,
                                    window,
                                    cx,
                                );
                                cx.stop_propagation();
                            }
                        })),
                )
                .child(
                    div()
                        .id("task-edit-save")
                        .h(px(28.0))
                        .px(px(14.0))
                        .rounded(px(6.0))
                        .bg(theme.inverse)
                        .text_color(theme.on_inverse)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(sp(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .when(can_save, |element| {
                            element
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.92))
                                .tab_index(0)
                                .focus_visible(|style| style.border_color(theme.accent))
                        })
                        .when(!can_save, |element| element.opacity(0.45))
                        .child(tr!("board.save"))
                        .when(can_save, |element| {
                            element
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.save_board_task_edits(cx);
                                }))
                                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                        this.save_board_task_edits(cx);
                                        cx.stop_propagation();
                                    }
                                }))
                        }),
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
                        // Agent selector chips (disabled profiles unselectable)
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
                                .child(self.render_board_agent_chips(
                                    agent,
                                    "board-new-task",
                                    BoardAgentPickTarget::NewTask,
                                    &BOARD_MODAL_PROVIDERS,
                                    theme,
                                    cx,
                                )),
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::model::{
        AgentTurn, ContextUsage, InteractionMode, RuntimeMode, SessionStatus, SessionWorkspace,
        ThreadGoal, ThreadGoalStatus, TurnStatus,
    };

    fn test_session() -> AgentSession {
        AgentSession {
            id: Uuid::new_v4(),
            title: String::from("test session"),
            auto_title: None,
            project_id: Uuid::new_v4(),
            workspace: SessionWorkspace::Local,
            provider: ProviderKind::Codex,
            model: None,
            runtime_mode: RuntimeMode::FullAccess,
            interaction_mode: InteractionMode::Build,
            reasoning_effort: None,
            service_tier: None,
            context_window: None,
            agent_preset: None,
            status: SessionStatus::Working,
            created_at: 1_000,
            updated_at: 2_000,
            last_reply_at: None,
            pinned_at: None,
            archived_at: None,
            provider_cursor: None,
            available_commands: Vec::new(),
            thread_goal: None,
            context_usage: None,
            runtime_event_cursor: None,
            provider_session_id: None,
            messages: Vec::new(),
            transcript_blocks: Vec::new(),
            turns: Vec::new(),
            queued_messages: Vec::new(),
            detail_loaded: true,
        }
    }

    fn test_turn(started_at: u64, completed_at: Option<u64>) -> AgentTurn {
        AgentTurn {
            id: Uuid::new_v4(),
            turn_count: 1,
            status: TurnStatus::Completed,
            provider_turn_started: false,
            provider_resume_at: None,
            started_at,
            completed_at,
            checkpoint: None,
        }
    }

    fn test_summary(index: u64, session_id: Option<Uuid>) -> TaskSummary {
        TaskSummary {
            id: Uuid::from_u128(index as u128),
            project_id: Uuid::nil(),
            title: format!("task {index}"),
            description_preview: String::new(),
            status: TaskStatus::Running,
            assigned_agent: None,
            model: None,
            session_id,
            labels: Vec::new(),
            needs_attention: false,
            sync_failed: None,
            updated_at: 2_000,
            version: 1,
            archived: false,
        }
    }

    #[test]
    fn live_badges_absent_without_session() {
        assert_eq!(task_live_badges(None, 5_000), TaskLiveBadges::default());
    }

    #[test]
    fn live_badges_hide_zero_usage_but_tick_busy_wall_clock() {
        let session = test_session();
        assert_eq!(
            task_live_badges(Some(&session), 5_000),
            TaskLiveBadges {
                tokens: None,
                duration_secs: Some(4_000),
            }
        );
    }

    #[test]
    fn live_badges_read_context_usage_and_turns() {
        let mut session = test_session();
        session.context_usage = Some(ContextUsage {
            tokens: 12_500,
            window: Some(200_000),
        });
        session.turns = vec![test_turn(1_000, Some(1_060)), test_turn(2_000, None)];
        // The open turn ticks against `now`.
        assert_eq!(
            task_live_badges(Some(&session), 2_030),
            TaskLiveBadges {
                tokens: Some(12_500),
                duration_secs: Some(90),
            }
        );
    }

    #[test]
    fn live_badges_prefer_larger_goal_ledger() {
        let mut session = test_session();
        session.status = SessionStatus::Idle;
        session.context_usage = Some(ContextUsage {
            tokens: 100,
            window: None,
        });
        session.thread_goal = Some(ThreadGoal {
            objective: String::from("goal"),
            status: ThreadGoalStatus::Active,
            token_budget: Some(50_000),
            tokens_used: 4_000,
            time_used_seconds: 0,
        });
        let badges = task_live_badges(Some(&session), 9_000);
        assert_eq!(badges.tokens, Some(4_000));
        // No turns and no goal time: idle wall clock from `updated_at`.
        assert_eq!(badges.duration_secs, Some(1_000));
    }

    #[test]
    fn drawer_labels_split_on_commas_and_trim() {
        assert!(split_board_labels("").is_empty());
        assert_eq!(
            split_board_labels("bug, feature ,,  refactor "),
            vec![
                "bug".to_owned(),
                "feature".to_owned(),
                "refactor".to_owned()
            ]
        );
    }

    #[test]
    fn live_badges_stay_interactive_with_500_cards() {
        let now = 1_000_000u64;
        let mut sessions = Vec::with_capacity(500);
        let mut summaries = Vec::with_capacity(500);
        for index in 0..500u64 {
            let session_id = Uuid::from_u128(1_000_000 + index as u128);
            let mut session = test_session();
            session.id = session_id;
            session.context_usage = Some(ContextUsage {
                tokens: 1_000 + index,
                window: None,
            });
            session.turns = vec![test_turn(now - 300, Some(now - 240))];
            sessions.push(session);
            summaries.push(test_summary(index, Some(session_id)));
        }

        let started = Instant::now();
        // One pass per frame, then O(1) lookups per visible row — the same
        // shape as `render_board_page`.
        let live: HashMap<Uuid, TaskLiveBadges> = sessions
            .iter()
            .map(|session| (session.id, task_live_badges(Some(session), now)))
            .collect();
        let mut rendered = 0u64;
        for summary in &summaries {
            if let Some(badges) = summary.session_id.and_then(|id| live.get(&id).copied()) {
                rendered += u64::from(badges.tokens.is_some());
            }
        }
        let elapsed = started.elapsed();

        assert_eq!(rendered, 500);
        assert!(
            elapsed < Duration::from_millis(500),
            "500-card badge join took {elapsed:?}"
        );
    }
}
