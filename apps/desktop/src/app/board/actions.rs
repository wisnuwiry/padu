use super::types::split_board_labels;
use crate::app::{Padu, WorkspacePage};
use crate::model::ProviderKind;
use gpui::Context;
use padu_client::kanban::{CreateTask, Task, TaskStatus, TaskSummary};
use uuid::Uuid;

impl Padu {
    pub(crate) fn open_board(&mut self, cx: &mut Context<Self>) {
        self.navigate_workspace_page(WorkspacePage::Board, cx);
        self.ensure_board_tasks_loaded(cx);
        // Assignee pickers mark disabled profiles, so have the registry ready.
        self.ensure_agent_profiles(false, cx);
    }

    pub(crate) fn ensure_board_tasks_loaded(&mut self, cx: &mut Context<Self>) {
        if self.board_loaded || self.board_load_pending {
            return;
        }
        self.load_board_tasks_from_daemon(cx);
    }

    pub(crate) fn load_board_tasks_from_daemon(&mut self, cx: &mut Context<Self>) {
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

    pub(crate) fn move_board_task(
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

    pub(crate) fn select_board_task(&mut self, task_id: Option<Uuid>, cx: &mut Context<Self>) {
        self.board_selected_task_id = task_id;
        self.board_hydrated_task = None;
        self.board_edit_agent = None;
        self.board_edit_saving = false;
        self.board_edit_preview = false;
        self.board_edit_title
            .update(cx, |field, cx| field.set_content("", cx));
        self.board_edit_description
            .update(cx, |field, cx| field.set_content("", cx));
        self.board_edit_labels_list.clear();
        self.board_edit_labels
            .update(cx, |field, cx| field.set_content("", cx));
        if let Some(id) = task_id {
            self.hydrate_selected_task(id, cx);
        }
        cx.notify();
    }

    pub(crate) fn hydrate_selected_task(&mut self, task_id: Uuid, cx: &mut Context<Self>) {
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

    pub(crate) fn delete_board_task(
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
    pub(crate) fn board_profile_disabled(&self, provider: ProviderKind) -> bool {
        self.agent_profiles
            .iter()
            .find(|profile| profile.agent_id == provider)
            .is_some_and(|profile| !profile.enabled)
    }

    /// Fill the drawer's edit inputs from the hydrated task. Runs only on
    /// hydrate (never while typing), so in-flight edits are never clobbered.
    pub(crate) fn sync_board_edit_fields(&mut self, task: &Task, cx: &mut Context<Self>) {
        self.board_edit_agent = task.assigned_agent;
        self.board_edit_saving = false;
        self.board_edit_title
            .update(cx, |field, cx| field.set_content(&task.title, cx));
        self.board_edit_description
            .update(cx, |field, cx| field.set_content(&task.description, cx));
        self.board_edit_labels_list = task.labels.clone();
        self.board_edit_labels
            .update(cx, |field, cx| field.set_content("", cx));
    }

    pub(crate) fn commit_board_edit_label(&mut self, cx: &mut Context<Self>) {
        let raw = self.board_edit_labels.read(cx).content();
        let parts = split_board_labels(&raw);
        let mut added = false;
        for part in parts {
            if !self.board_edit_labels_list.contains(&part) {
                self.board_edit_labels_list.push(part);
                added = true;
            }
        }
        self.board_edit_labels
            .update(cx, |field, cx| field.set_content("", cx));
        if added {
            cx.notify();
        }
    }

    pub(crate) fn save_board_task_edits(&mut self, cx: &mut Context<Self>) {
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
        self.commit_board_edit_label(cx);
        let description = self
            .board_edit_description
            .read(cx)
            .content()
            .trim()
            .to_owned();
        let labels = self.board_edit_labels_list.clone();
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

    pub(crate) fn board_models_for_provider(
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

    pub(crate) fn open_new_task_modal(&mut self, cx: &mut Context<Self>) {
        self.board_new_task_modal_open = true;
        self.board_new_task_preview = false;
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

    pub(crate) fn create_board_task(
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
}
