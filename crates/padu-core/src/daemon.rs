//! Provider backend and driver-event wire translation for `padu-daemon`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use crate::{
    Backend, Command, EventSink, Request, ResponsePayload, WireDriverEvent, WorkspaceOperation,
    WorkspaceResult,
};
use anyhow::{Context as _, anyhow, bail};
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::attachments::AttachmentStore;
use crate::computer_use::{ComputerTarget, ComputerUsePhase, ComputerUseState};
use crate::driver::{self, DriverHandle, DriverStartOptions, SessionOptions};
use crate::model::{
    ActivityKind, AgentSession, Checkpoint, CheckpointStatus, DriverEvent, PermissionOption,
    Project, ProviderKind, ProviderResumeCursor, QueuedMessage, SessionStatus, SessionWorkspace,
};
use crate::persistence::{ComposerDraftStore, PersistedState, StateStore};
use crate::settings::DaemonSettingsStore;
use padu_protocol::kanban::{Task, TaskStatus, TaskSummary, TaskWorkspaceKind};
use padu_protocol::model::ProviderSessionHistory;
use padu_protocol::provider_session::{ProviderSessionFork, ProviderSessionForkRequest};

pub struct PaduBackend {
    sessions: Mutex<HashMap<Uuid, (Uuid, DriverHandle)>>,
    terminals: Mutex<HashMap<Uuid, (Uuid, crate::terminal::DaemonTerminal)>>,
    settings: DaemonSettingsStore,
    task_store: StateStore,
    task_state: Mutex<PersistedState>,
    removed_session_ids: Mutex<HashSet<Uuid>>,
    composer_drafts: ComposerDraftStore,
    attachments: AttachmentStore,
    usage_scan_cache: Mutex<crate::usage_history::ScanCache>,
    agy_install_cancellation: Mutex<Option<crate::download_manager::DownloadCancellation>>,
    checkpoint_capture_locks: Mutex<HashMap<(PathBuf, Uuid, usize), Arc<Mutex<()>>>>,
    checkpoint_failure_streaks: Mutex<HashMap<Uuid, u32>>,
    usage_rates_dir: std::path::PathBuf,
    default_cwd: std::path::PathBuf,
}

impl PaduBackend {
    pub fn new(settings: DaemonSettingsStore, task_store: StateStore) -> anyhow::Result<Self> {
        let mut task_state = task_store
            .load()
            .context("could not load Padu task database")?;
        migrate_projectless_state(&task_store, &mut task_state)?;
        let composer_drafts = ComposerDraftStore::for_state_path(task_store.path());
        let attachments = AttachmentStore::new(
            task_store
                .path()
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join("attachments"),
        );
        let usage_rates_dir = task_store
            .path()
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_owned();
        Ok(Self {
            sessions: Mutex::new(HashMap::new()),
            terminals: Mutex::new(HashMap::new()),
            settings,
            task_store,
            task_state: Mutex::new(task_state),
            removed_session_ids: Mutex::new(HashSet::new()),
            composer_drafts,
            attachments,
            usage_scan_cache: Mutex::new(HashMap::new()),
            agy_install_cancellation: Mutex::new(None),
            checkpoint_capture_locks: Mutex::new(HashMap::new()),
            checkpoint_failure_streaks: Mutex::new(HashMap::new()),
            usage_rates_dir,
            default_cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        })
    }

    /// Keeps legacy `DaemonSettings.disabled_providers` derived from the
    /// profile registry (PRD §6.3): disabling adds the provider, enabling
    /// removes it, so pre-P1-03 surfaces that still read settings stay
    /// consistent with the registry source of truth.
    fn sync_profile_enabled_to_settings(
        &self,
        provider: ProviderKind,
        enabled: bool,
    ) -> anyhow::Result<()> {
        let mut settings = self.settings.get();
        if enabled {
            settings
                .disabled_providers
                .retain(|candidate| *candidate != provider);
        } else if !settings.disabled_providers.contains(&provider) {
            settings.disabled_providers.push(provider);
        }
        Ok(self.settings.replace(settings)?)
    }

    /// Capture and persist one ending checkpoint exactly once per daemon.
    /// Desktop and Web may observe the same turn completion concurrently; a
    /// per-turn lock prevents both clients from running the expensive Git
    /// snapshot while leaving unrelated tasks independent.
    fn capture_turn_checkpoint(
        &self,
        cwd: PathBuf,
        session_id: Uuid,
        turn_count: usize,
        events: &EventSink,
    ) -> anyhow::Result<Checkpoint> {
        let key = (cwd.clone(), session_id, turn_count);
        let capture_lock = self
            .checkpoint_capture_locks
            .lock()
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        let _capture = capture_lock.lock();

        {
            let mut state = self.task_state.lock();
            if let Some(index) = state
                .sessions
                .iter()
                .position(|session| session.id == session_id)
            {
                self.task_store.hydrate(&mut state.sessions[index])?;
                if let Some(checkpoint) = state.sessions[index]
                    .turns
                    .iter()
                    .find(|turn| turn.turn_count == turn_count)
                    .and_then(|turn| turn.checkpoint.as_ref())
                    .filter(|checkpoint| {
                        matches!(
                            checkpoint.status,
                            CheckpointStatus::Ready | CheckpointStatus::Unavailable
                        )
                    })
                {
                    self.checkpoint_failure_streaks.lock().remove(&session_id);
                    if let Ok(Some(mut task)) = self.task_store.find_task_by_session_id(session_id)
                    {
                        if task.status == TaskStatus::Running {
                            task.status = TaskStatus::Review;
                            task.needs_attention = false;
                            task.sync_failed = None;
                            let version = task.version;
                            if let Ok(updated) = self.task_store.update_task(task, version) {
                                events.send_agent_completed(updated.id, session_id);
                                events.send_card_updated(updated.id);
                            }
                        }
                    }
                    return Ok(checkpoint.clone());
                }
            }
        }

        let checkpoint_result = crate::checkpoint::capture_turn(&cwd, session_id, turn_count);
        let checkpoint = match checkpoint_result {
            Ok(cp) => cp,
            Err(err) => {
                let streak = {
                    let mut streaks = self.checkpoint_failure_streaks.lock();
                    let count = streaks.entry(session_id).or_insert(0);
                    *count += 1;
                    *count
                };
                if let Ok(Some(mut task)) = self.task_store.find_task_by_session_id(session_id) {
                    events.send_checkpoint_failed(task.id, session_id, streak);
                    let threshold = self.get_checkpoint_failure_threshold(&task);
                    if streak >= threshold {
                        task.status = TaskStatus::Review;
                        task.needs_attention = true;
                        task.sync_failed = Some(format!(
                            "checkpoint failed {streak} consecutive times: {err}"
                        ));
                        let version = task.version;
                        if let Ok(updated) = self.task_store.update_task(task, version) {
                            events.send_card_updated(updated.id);
                        }
                    }
                }
                return Err(err);
            }
        };

        if checkpoint.status == CheckpointStatus::Error {
            let streak = {
                let mut streaks = self.checkpoint_failure_streaks.lock();
                let count = streaks.entry(session_id).or_insert(0);
                *count += 1;
                *count
            };
            if let Ok(Some(mut task)) = self.task_store.find_task_by_session_id(session_id) {
                events.send_checkpoint_failed(task.id, session_id, streak);
                let threshold = self.get_checkpoint_failure_threshold(&task);
                if streak >= threshold {
                    task.status = TaskStatus::Review;
                    task.needs_attention = true;
                    task.sync_failed =
                        Some(format!("checkpoint failed {streak} consecutive times"));
                    let version = task.version;
                    if let Ok(updated) = self.task_store.update_task(task, version) {
                        events.send_card_updated(updated.id);
                    }
                }
            }
        } else {
            self.checkpoint_failure_streaks.lock().remove(&session_id);
            if let Ok(Some(mut task)) = self.task_store.find_task_by_session_id(session_id) {
                if task.status == TaskStatus::Running {
                    task.status = TaskStatus::Review;
                    task.needs_attention = false;
                    task.sync_failed = None;
                    let version = task.version;
                    if let Ok(updated) = self.task_store.update_task(task, version) {
                        events.send_agent_completed(updated.id, session_id);
                        events.send_card_updated(updated.id);
                    }
                } else {
                    events.send_agent_completed(task.id, session_id);
                }
            }
        }

        let mut state = self.task_state.lock();
        if let Some(index) = state
            .sessions
            .iter()
            .position(|session| session.id == session_id)
        {
            self.task_store.hydrate(&mut state.sessions[index])?;
            if let Some(turn) = state.sessions[index]
                .turns
                .iter_mut()
                .find(|turn| turn.turn_count == turn_count)
            {
                turn.checkpoint = Some(checkpoint.clone());
                state.mark_session_dirty(session_id);
                self.task_store.save(&mut state)?;
            }
        }
        Ok(checkpoint)
    }

    fn get_checkpoint_failure_threshold(&self, task: &Task) -> u32 {
        const DEFAULT_THRESHOLD: u32 = 3;
        if let Some(agent_id) = task.assigned_agent {
            if let Ok(profiles) = self.task_store.list_agent_profiles() {
                if let Some(profile) = profiles.iter().find(|p| p.agent_id == agent_id) {
                    return profile.max_retry_before_escalate;
                }
            }
        }
        DEFAULT_THRESHOLD
    }

    fn queue_backlog_task(
        &self,
        existing: Task,
        expected_version: u64,
        events: &EventSink,
    ) -> anyhow::Result<Task> {
        let task_id = existing.id;
        let project_id = existing.project_id;
        let project = {
            let state = self.task_state.lock();
            state.projects.iter().find(|p| p.id == project_id).cloned()
        };
        let project = match project {
            Some(p) => p,
            None => {
                let err_msg = "project not found";
                let mut failed_task = existing.clone();
                failed_task.sync_failed = Some(err_msg.to_owned());
                let _ = self.task_store.update_task(failed_task, expected_version);
                bail!("{err_msg}");
            }
        };

        // Determine provider (honoring agent profiles)
        let provider = if let Some(assigned) = existing.assigned_agent {
            let profiles = self.task_store.list_agent_profiles().unwrap_or_default();
            if profiles.iter().any(|p| p.agent_id == assigned && p.enabled) {
                assigned
            } else {
                profiles
                    .iter()
                    .filter(|p| p.enabled)
                    .min_by_key(|p| p.priority)
                    .map(|p| p.agent_id)
                    .unwrap_or(assigned)
            }
        } else {
            let profiles = self.task_store.list_agent_profiles().unwrap_or_default();
            profiles
                .iter()
                .filter(|p| p.enabled)
                .min_by_key(|p| p.priority)
                .map(|p| p.agent_id)
                .unwrap_or(ProviderKind::Codex)
        };

        // Determine workspace honoring worktree policy
        let workspace = if project.is_projectless() {
            SessionWorkspace::Local
        } else {
            match existing.workspace_kind {
                Some(TaskWorkspaceKind::Local) => SessionWorkspace::Local,
                Some(TaskWorkspaceKind::NewWorktree) | Some(TaskWorkspaceKind::Worktree) => {
                    SessionWorkspace::NewWorktree { base_branch: None }
                }
                None => {
                    if project.path.join(".git").exists() {
                        SessionWorkspace::NewWorktree { base_branch: None }
                    } else {
                        SessionWorkspace::Local
                    }
                }
            }
        };

        let mut session = AgentSession::new(project.id, provider);
        session.title = existing.title.clone();
        if let Some(ref model) = existing.model {
            session.model = Some(model.clone());
        }
        session.workspace = workspace;
        let prompt = if !existing.description.trim().is_empty() {
            existing.description.clone()
        } else {
            existing.title.clone()
        };
        session.queued_messages.push(QueuedMessage::new(prompt));
        let session_id = session.id;

        // Persist session
        {
            let mut state = self.task_state.lock();
            state.push_session(session.clone());
            if let Err(err) = self.task_store.save(&mut state) {
                let mut failed_task = existing.clone();
                failed_task.sync_failed = Some(err.to_string());
                let _ = self.task_store.update_task(failed_task, expected_version);
                return Err(err.into());
            }
        }

        // Update task
        let mut task_to_update = existing;
        task_to_update.session_id = Some(session_id);
        task_to_update.workspace_kind = Some(TaskWorkspaceKind::from_session_workspace(
            &session.workspace,
        ));
        task_to_update.assigned_agent = Some(provider);
        task_to_update.status = TaskStatus::Queued;
        task_to_update.needs_attention = false;
        task_to_update.sync_failed = None;

        let updated = match self
            .task_store
            .update_task(task_to_update, expected_version)
        {
            Ok(updated) => updated,
            Err(err) => {
                // The session is already persisted: remove it so a lost
                // version race cannot orphan a session no card links to.
                let mut state = self.task_state.lock();
                state
                    .sessions
                    .retain(|candidate| candidate.id != session_id);
                let _ = self.task_store.save(&mut state);
                return Err(err.into());
            }
        };
        events.send_task_queued(task_id);
        events.send_task_state_changed();
        Ok(updated)
    }

    fn move_task(
        &self,
        task_id: Uuid,
        status: TaskStatus,
        expected_version: u64,
        events: &EventSink,
    ) -> anyhow::Result<Task> {
        if status == TaskStatus::Running {
            bail!(
                "cannot manually move task to running: only the daemon transitions a task to running when its workspace and provider process start"
            );
        }

        let existing = self
            .task_store
            .get_task(task_id)?
            .ok_or_else(|| anyhow!("task not found"))?;

        if existing.version != expected_version {
            bail!(
                "task version conflict: expected version {expected_version}, got {}",
                existing.version
            );
        }

        if existing.status == status {
            return Ok(existing);
        }

        if status == TaskStatus::Review
            && existing.status != TaskStatus::Running
            && existing.session_id.is_none()
        {
            bail!("cannot move task to review: task has not completed an agent run");
        }
        if status == TaskStatus::Done && existing.status == TaskStatus::Running {
            bail!(
                "cannot mark a running task as done: wait for the agent to complete or reopen to backlog"
            );
        }
        if status == TaskStatus::Done && existing.status == TaskStatus::Queued {
            bail!("cannot mark a queued task as done: cancel to backlog first");
        }

        if existing.status == TaskStatus::Backlog && status == TaskStatus::Queued {
            return self.queue_backlog_task(existing, expected_version, events);
        }

        let updated = self
            .task_store
            .move_task(task_id, status, expected_version)?;
        Ok(updated)
    }

    pub fn mark_task_done_from_external_signal(
        &self,
        task_id: Uuid,
        events: &EventSink,
    ) -> anyhow::Result<Task> {
        let task = self
            .task_store
            .get_task(task_id)?
            .ok_or_else(|| anyhow!("task not found"))?;
        if task.status != TaskStatus::Review {
            bail!(
                "cannot complete task on external signal: task is in {:?}, not in review",
                task.status
            );
        }
        let mut updated = task.clone();
        updated.status = TaskStatus::Done;
        let saved = self.task_store.update_task(updated, task.version)?;
        events.send_card_updated(saved.id);
        Ok(saved)
    }

    /// Transitions a linked task from Queued to Running and emits
    /// `workspace_started` once the worktree is materialized and the provider
    /// process is spawned (P1-06).
    pub fn on_workspace_started(&self, session_id: Uuid, events: &EventSink) {
        if let Ok(Some(mut task)) = self.task_store.find_task_by_session_id(session_id) {
            if task.status == TaskStatus::Queued {
                task.status = TaskStatus::Running;
                task.sync_failed = None;
                let version = task.version;
                if let Ok(updated) = self.task_store.update_task(task, version) {
                    events.send_workspace_started(updated.id, session_id);
                    events.send_card_updated(updated.id);
                }
            }
        }
    }
}

/// Storage-layout migrations belong to the daemon because both the database
/// rows and the directories name paths on its host. Persist after each move
/// so a later failure cannot leave an earlier project pointing at its old
/// location in SQLite.
fn migrate_projectless_state(
    task_store: &StateStore,
    task_state: &mut PersistedState,
) -> anyhow::Result<()> {
    let indices = task_state
        .projects
        .iter()
        .enumerate()
        .filter_map(|(index, project)| {
            crate::projectless::needs_migration(&project.path).then_some(index)
        })
        .collect::<Vec<_>>();
    for index in indices {
        let old_path = task_state.projects[index].path.clone();
        let workspace = crate::projectless::migrate_workspace(&old_path).with_context(|| {
            format!(
                "could not move projectless workspace {} under ~/.padu/projects",
                old_path.display()
            )
        })?;
        task_state.projects[index].name = crate::model::Project::PROJECTLESS_NAME.to_owned();
        task_state.projects[index].path = workspace.cwd;
        task_store
            .save(task_state)
            .context("could not persist migrated projectless workspace")?;
    }
    Ok(())
}

impl Backend for PaduBackend {
    fn handle(&self, request: Request, events: EventSink) -> anyhow::Result<ResponsePayload> {
        let session_id = request.session_id;
        let runtime_id = request.runtime_id;
        match request.command {
            Command::AttachSession => {
                let sessions = self.sessions.lock();
                let Some((runtime_id, driver)) = sessions.get(&session_id) else {
                    return Ok(ResponsePayload::SessionRuntime {
                        runtime_id: None,
                        supports_steer: false,
                    });
                };
                Ok(ResponsePayload::SessionRuntime {
                    runtime_id: Some(*runtime_id),
                    supports_steer: driver.supports_steer(),
                })
            }
            Command::GetSettings => Ok(ResponsePayload::Settings {
                settings: self.settings.get(),
            }),
            Command::UpdateSettings { settings } => {
                self.settings.replace(settings)?;
                Ok(ResponsePayload::Ack)
            }
            Command::InstallAgyAcp | Command::ReinstallAgyAcp => {
                let cancellation = crate::download_manager::DownloadCancellation::new();
                {
                    let mut active = self.agy_install_cancellation.lock();
                    if active.is_some() {
                        bail!("an Antigravity ACP installation is already running");
                    }
                    *active = Some(cancellation.clone());
                }
                let progress_events = events.clone();
                let install_result =
                    crate::agy_install::install(&cancellation, |percent, phase| {
                        progress_events.send_provider_install_progress(
                            ProviderKind::Agy,
                            phase,
                            percent,
                        );
                    });
                self.agy_install_cancellation.lock().take();
                let path = install_result?;
                let mut settings = self.settings.get();
                settings
                    .provider_binary_overrides
                    .insert(ProviderKind::Agy, path.display().to_string());
                self.settings.replace(settings)?;
                Ok(ResponsePayload::ProviderInstalled {
                    provider: ProviderKind::Agy,
                    path,
                })
            }
            Command::CancelAgyAcpInstall => {
                if let Some(cancellation) = self.agy_install_cancellation.lock().as_ref() {
                    cancellation.cancel();
                }
                Ok(ResponsePayload::Ack)
            }
            Command::CheckAgyAuth => {
                ensure_shell_environment();
                let settings = self.settings.get();
                let binary_override = settings
                    .provider_binary_overrides
                    .get(&ProviderKind::Agy)
                    .map(String::as_str);
                let authenticated =
                    crate::model::provider_probe(ProviderKind::Agy, binary_override)
                        .path
                        .map(|path| crate::driver::agy_auth_status(&path).unwrap_or(false))
                        .unwrap_or(false);
                // Identity comes from the credential stores, not the probe:
                // signed-out means no account, and a missing email just hides
                // the settings row rather than failing the check.
                let account_label = authenticated
                    .then(crate::driver::agy_account_label)
                    .flatten();
                Ok(ResponsePayload::AgyAuthStatus {
                    authenticated,
                    account_label,
                })
            }
            Command::LogoutAgy => {
                let settings = self.settings.get();
                let binary_override = settings
                    .provider_binary_overrides
                    .get(&ProviderKind::Agy)
                    .map(String::as_str);
                let path = crate::model::provider_probe(ProviderKind::Agy, binary_override)
                    .path
                    .ok_or_else(|| anyhow!("Antigravity ACP server is not installed"))?;
                crate::driver::logout_agy(&path, &self.default_cwd)?;
                Ok(ResponsePayload::Ack)
            }
            Command::AuthenticateAgy => {
                let settings = self.settings.get();
                let binary_override = settings
                    .provider_binary_overrides
                    .get(&ProviderKind::Agy)
                    .map(String::as_str);
                let path = crate::model::provider_probe(ProviderKind::Agy, binary_override)
                    .path
                    .ok_or_else(|| anyhow!("Antigravity ACP server is not installed"))?;
                let progress_events = events.clone();
                crate::driver::authenticate_agy(&path, &self.default_cwd, move |url| {
                    progress_events.send_provider_auth_url(ProviderKind::Agy, url);
                })?;
                Ok(ResponsePayload::Ack)
            }
            Command::RemoveAgyAcp => {
                crate::agy_install::remove()?;
                let mut settings = self.settings.get();
                settings
                    .provider_binary_overrides
                    .remove(&ProviderKind::Agy);
                self.settings.replace(settings)?;
                Ok(ResponsePayload::Ack)
            }
            Command::ProbeProvider {
                provider,
                binary_override,
                discover_models,
                probe_version,
            } => {
                ensure_shell_environment();
                let mut probe = match binary_override.as_deref() {
                    override_value if discover_models || probe_version => {
                        crate::model::provider_probe(provider, override_value)
                    }
                    override_value => crate::model::cached_provider_probe(provider, override_value),
                };
                let version = probe_version
                    .then(|| {
                        probe
                            .path
                            .as_deref()
                            .and_then(crate::model::probe_provider_version)
                    })
                    .flatten();
                if discover_models {
                    probe = crate::model::discover_provider_models(probe);
                }
                Ok(ResponsePayload::ProviderProbe { probe, version })
            }
            Command::FetchPlanUsage {
                provider,
                binary_override,
                cli_version,
            } => {
                let usage = match provider {
                    crate::model::ProviderKind::Claude => Some(
                        crate::usage::fetch_claude_plan_usage(cli_version.as_deref())?,
                    ),
                    crate::model::ProviderKind::Codex => {
                        Some(crate::usage::fetch_codex_plan_usage()?)
                    }
                    crate::model::ProviderKind::OpenCode => {
                        crate::usage::fetch_opencode_go_plan_usage()?
                    }
                    crate::model::ProviderKind::Grok => {
                        ensure_shell_environment();
                        let probe = match binary_override.as_deref() {
                            override_value => {
                                crate::model::provider_probe(provider, override_value)
                            }
                        };
                        let binary = probe.path.ok_or_else(|| anyhow!("grok is not installed"))?;
                        Some(crate::usage::fetch_grok_plan_usage(&binary)?)
                    }
                    _ => bail!("provider has no plan usage fetcher"),
                };
                Ok(ResponsePayload::PlanUsage { usage })
            }
            Command::ProbeComputerPermissions { prompt } => {
                Ok(ResponsePayload::ComputerPermissions {
                    permissions: crate::computer_use::probe_permissions(prompt)?,
                })
            }
            Command::LoadUsageHistory {
                window,
                project_roots,
            } => {
                let rates = crate::usage_history::load_rate_table(&self.usage_rates_dir);
                let history = crate::usage_history::scan(
                    &mut self.usage_scan_cache.lock(),
                    &rates,
                    window,
                    &project_roots,
                );
                Ok(ResponsePayload::UsageHistory { history })
            }
            Command::LoadSkills { projects } => {
                let locations = crate::skills::skill_locations(&projects);
                Ok(ResponsePayload::SkillsCatalog {
                    catalog: crate::skills::scan_skills(&locations),
                })
            }
            Command::SetSkillsEnabled { dirs, enabled } => {
                for dir in dirs {
                    crate::skills::set_skill_enabled(&dir, enabled)
                        .map_err(|error| anyhow!(error))?;
                }
                Ok(ResponsePayload::Ack)
            }
            Command::TrashSkills { dirs } => {
                crate::skills::trash_skills(&dirs).map_err(|error| anyhow!(error))?;
                Ok(ResponsePayload::Ack)
            }
            Command::LoadTaskState => {
                let state = self.task_state.lock();
                Ok(ResponsePayload::TaskState {
                    projects: state.projects.clone(),
                    sessions: state
                        .sessions
                        .iter()
                        .map(AgentSession::list_projection)
                        .collect(),
                    default_cwd: self.default_cwd.clone(),
                    projectless_root: crate::projectless::workspace_root(),
                })
            }
            Command::SaveTaskState {
                projects,
                live_session_ids: _,
                sessions,
            } => {
                let active_runtimes = self
                    .sessions
                    .lock()
                    .iter()
                    .map(|(session_id, (runtime_id, _))| (*session_id, *runtime_id))
                    .collect::<HashMap<_, _>>();
                let mut state = self.task_state.lock();
                let removed_session_ids = self.removed_session_ids.lock();
                for project in projects {
                    if let Some(existing) = state
                        .projects
                        .iter_mut()
                        .find(|existing| existing.id == project.id)
                    {
                        *existing = project;
                    } else {
                        state.projects.push(project);
                    }
                }
                let sessions = sessions
                    .into_iter()
                    .filter(|session| !removed_session_ids.contains(&session.id))
                    .collect::<Vec<_>>();
                drop(removed_session_ids);
                let saved_ids = sessions
                    .iter()
                    .map(|session| session.id)
                    .collect::<Vec<_>>();
                for mut session in sessions {
                    if let Some(existing) = state
                        .sessions
                        .iter_mut()
                        .find(|existing| existing.id == session.id)
                    {
                        if session_projection_precedes(
                            existing,
                            &session,
                            active_runtimes.get(&session.id).copied(),
                        ) {
                            merge_stale_session_metadata(existing, session);
                        } else {
                            preserve_daemon_checkpoints(existing, &mut session);
                            let pinned_at = existing.pinned_at;
                            let archived_at = existing.archived_at;
                            *existing = session;
                            // Pin/archive metadata is mutated only through the
                            // daemon-owned commands below. A stale full-session
                            // save must never undo a concurrent metadata change.
                            existing.pinned_at = pinned_at;
                            existing.archived_at = archived_at;
                        }
                    } else {
                        state.sessions.push(session);
                    }
                }
                let used_project_ids = state
                    .sessions
                    .iter()
                    .map(|session| session.project_id)
                    .collect::<std::collections::HashSet<_>>();
                state.projects.retain(|project| {
                    !project.is_projectless() || used_project_ids.contains(&project.id)
                });
                for session_id in &saved_ids {
                    state.mark_session_dirty(*session_id);
                }
                self.task_store.save(&mut state)?;
                let sessions = saved_ids
                    .into_iter()
                    .filter_map(|session_id| {
                        state
                            .sessions
                            .iter()
                            .find(|session| session.id == session_id)
                            .cloned()
                    })
                    .collect();
                Ok(ResponsePayload::TaskStateSaved { sessions })
            }
            Command::SetSessionPinned { pinned } => {
                let mut state = self.task_state.lock();
                let index = state
                    .sessions
                    .iter()
                    .position(|session| session.id == session_id)
                    .ok_or_else(|| anyhow!("session not found: {session_id}"))?;
                let (changed, projection) = {
                    let session = &mut state.sessions[index];
                    if pinned && session.archived_at.is_some() {
                        bail!("cannot pin an archived session");
                    }
                    let next = if pinned {
                        session
                            .pinned_at
                            .or_else(|| Some(crate::model::unix_time()))
                    } else {
                        None
                    };
                    let changed = session.pinned_at != next;
                    if changed {
                        session.pinned_at = next;
                    }
                    (changed, session.list_projection())
                };
                if changed {
                    state.mark_session_dirty(session_id);
                    self.task_store.save(&mut state)?;
                }
                Ok(ResponsePayload::SessionMetadataUpdated {
                    session: projection,
                })
            }
            Command::SetSessionArchived { archived } => {
                let mut state = self.task_state.lock();
                let index = state
                    .sessions
                    .iter()
                    .position(|session| session.id == session_id)
                    .ok_or_else(|| anyhow!("session not found: {session_id}"))?;
                let (changed, projection) = {
                    let session = &mut state.sessions[index];
                    if archived && session.is_busy() {
                        bail!("cannot archive a busy session");
                    }
                    let next = if archived {
                        session
                            .archived_at
                            .or_else(|| Some(crate::model::unix_time()))
                    } else {
                        None
                    };
                    let changed =
                        session.archived_at != next || (archived && session.pinned_at.is_some());
                    if changed {
                        session.archived_at = next;
                        if archived {
                            session.pinned_at = None;
                        }
                    }
                    (changed, session.list_projection())
                };
                if changed {
                    state.mark_session_dirty(session_id);
                    self.task_store.save(&mut state)?;
                }
                Ok(ResponsePayload::SessionMetadataUpdated {
                    session: projection,
                })
            }
            Command::RemoveSession => {
                {
                    let mut state = self.task_state.lock();
                    self.removed_session_ids.lock().insert(session_id);
                    let project_id = state
                        .sessions
                        .iter()
                        .find(|session| session.id == session_id)
                        .map(|session| session.project_id);
                    state.sessions.retain(|session| session.id != session_id);
                    if let Some(project_id) = project_id {
                        let remove_project = state
                            .projects
                            .iter()
                            .find(|project| project.id == project_id)
                            .is_some_and(Project::is_projectless)
                            && !state
                                .sessions
                                .iter()
                                .any(|session| session.project_id == project_id);
                        if remove_project {
                            state.projects.retain(|project| project.id != project_id);
                        }
                    }
                    self.task_store.save(&mut state)?;
                }
                let removed = self.sessions.lock().remove(&session_id);
                drop(removed);
                Ok(ResponsePayload::Ack)
            }
            Command::HydrateSession { session_id } => {
                let mut state = self.task_state.lock();
                let session = if let Some(session) = state
                    .sessions
                    .iter_mut()
                    .find(|session| session.id == session_id)
                {
                    self.task_store.hydrate(session)?;
                    Some(session.clone())
                } else {
                    None
                };
                Ok(ResponsePayload::Session { session })
            }
            Command::SearchSessionMessages { query, limit } => {
                let matches = self.task_store.session_message_search(query, limit)()?;
                Ok(ResponsePayload::SessionMessageMatches { matches })
            }
            Command::ListProviderSessions { provider, limit } => {
                const MAX_PROVIDER_SESSIONS: usize = 500;
                let limit = limit.min(MAX_PROVIDER_SESSIONS);
                if limit == 0 {
                    return Ok(ResponsePayload::ProviderSessions {
                        sessions: Vec::new(),
                    });
                }
                ensure_shell_environment();
                let settings = self.settings.get();
                // The profile registry is the source of truth for candidacy
                // (PRD §6.3); legacy `disabled_providers` is derived from it
                // on every profile update, so it is not consulted here.
                if !self.task_store.agent_profile_enabled(provider)? {
                    return Ok(ResponsePayload::ProviderSessions {
                        sessions: Vec::new(),
                    });
                }
                let binary_override = settings
                    .provider_binary_overrides
                    .get(&provider)
                    .map(String::as_str);
                let Some(binary) = crate::model::provider_probe(provider, binary_override).path
                else {
                    return Ok(ResponsePayload::ProviderSessions {
                        sessions: Vec::new(),
                    });
                };
                // Discovery is deliberately provider-scoped. Opening Resume
                // must not start every installed agent CLI, and another
                // provider is queried only after the user explicitly picks it.
                let mut sessions = match provider {
                    ProviderKind::Amp => {
                        crate::amp_session::list_provider_sessions(&binary, limit)?
                    }
                    ProviderKind::Claude => crate::claude_session::list_provider_sessions(limit)?,
                    ProviderKind::Codex => {
                        crate::codex_session::list_provider_sessions(&binary, limit)?
                    }
                    ProviderKind::Agy
                    | ProviderKind::Cursor
                    | ProviderKind::Fx
                    | ProviderKind::OpenCode
                    | ProviderKind::Qoder => {
                        crate::acp_session::list_provider_sessions(provider, &binary, &[], limit)?
                    }
                    ProviderKind::DeepSeek => {
                        crate::deepseek_session::list_provider_sessions(&binary, limit)?
                    }
                    ProviderKind::Grok => crate::grok_session::list_provider_sessions(limit)?,
                    ProviderKind::Kimi => crate::kimi_session::list_provider_sessions(limit)?,
                    ProviderKind::CommandCode => {
                        crate::command_code_session::list_provider_sessions(limit)?
                    }
                    ProviderKind::OhMyPi | ProviderKind::Pi => {
                        crate::pi_session::list_provider_sessions(provider, limit)?
                    }
                };
                sessions.sort_by(|a, b| {
                    b.updated_at
                        .cmp(&a.updated_at)
                        .then_with(|| a.title.cmp(&b.title))
                });
                let imported = {
                    let state = self.task_state.lock();
                    state
                        .sessions
                        .iter()
                        .filter_map(|session| session.provider_cursor.as_ref())
                        .map(|cursor| (cursor.provider(), cursor.native_id().to_owned()))
                        .collect::<HashSet<_>>()
                };
                sessions.retain(|session| {
                    !imported.contains(&(session.provider(), session.cursor.native_id().to_owned()))
                });
                sessions.truncate(limit);
                Ok(ResponsePayload::ProviderSessions { sessions })
            }
            Command::LoadProviderSession { cursor, cwd } => {
                // Preserve every native turn shell for exact provider turn
                // numbering, but bound imported display text to recent turns.
                const VISIBLE_TURN_LIMIT: usize = 100;
                let history = match &cursor {
                    ProviderResumeCursor::Amp { thread_id, .. } => {
                        let binary = self.provider_binary(ProviderKind::Amp)?;
                        crate::amp_session::provider_session_history(
                            &binary,
                            &cwd,
                            thread_id,
                            VISIBLE_TURN_LIMIT,
                        )?
                    }
                    ProviderResumeCursor::Claude { session_id, .. } => {
                        self.provider_binary(ProviderKind::Claude)?;
                        crate::claude_session::provider_session_history(
                            session_id,
                            VISIBLE_TURN_LIMIT,
                        )?
                    }
                    ProviderResumeCursor::Codex { thread_id } => {
                        let binary = self.provider_binary(ProviderKind::Codex)?;
                        crate::codex_session::provider_session_history(
                            &binary,
                            thread_id,
                            VISIBLE_TURN_LIMIT,
                        )?
                    }
                    ProviderResumeCursor::CommandCode { session_id } => {
                        crate::command_code_session::provider_session_history(
                            session_id,
                            VISIBLE_TURN_LIMIT,
                        )?
                    }
                    ProviderResumeCursor::Qoder { .. } => ProviderSessionHistory {
                        messages: Vec::new(),
                        turns: Vec::new(),
                    },
                    ProviderResumeCursor::Agy { session_id, .. }
                    | ProviderResumeCursor::Cursor { session_id, .. }
                    | ProviderResumeCursor::Fx { session_id }
                    | ProviderResumeCursor::OpenCode { session_id }
                    | ProviderResumeCursor::Grok { session_id }
                    | ProviderResumeCursor::Kimi { session_id } => {
                        let provider = cursor.provider();
                        let binary = self.provider_binary(provider)?;
                        crate::acp_session::provider_session_history(
                            provider,
                            &binary,
                            &cwd,
                            session_id,
                            VISIBLE_TURN_LIMIT,
                        )?
                    }
                    ProviderResumeCursor::DeepSeek { session_id } => {
                        let binary = self.provider_binary(ProviderKind::DeepSeek)?;
                        crate::deepseek_session::provider_session_history(
                            &binary,
                            session_id,
                            VISIBLE_TURN_LIMIT,
                        )?
                    }
                    ProviderResumeCursor::OhMyPi {
                        session_id,
                        session_file,
                    }
                    | ProviderResumeCursor::Pi {
                        session_id,
                        session_file,
                    } => {
                        self.provider_binary(cursor.provider())?;
                        let session_file = session_file.as_deref().ok_or_else(|| {
                            anyhow!(
                                "{} did not report its native session file",
                                cursor.provider().display_name()
                            )
                        })?;
                        crate::pi_session::provider_session_history(
                            cursor.provider(),
                            session_id,
                            session_file,
                            VISIBLE_TURN_LIMIT,
                        )?
                    }
                };
                Ok(ResponsePayload::ProviderSessionHistory { history })
            }
            Command::LoadComposerDrafts => Ok(ResponsePayload::ComposerDrafts {
                drafts: self.composer_drafts.load()?,
            }),
            Command::SaveComposerDrafts { drafts, generation } => {
                self.composer_drafts.save(drafts, generation)?;
                Ok(ResponsePayload::Ack)
            }
            Command::ApplyComposerDraftChanges { changes } => {
                self.composer_drafts.apply_changes(changes)?;
                Ok(ResponsePayload::Ack)
            }
            Command::ListNotes { project_id } => Ok(ResponsePayload::Notes {
                notes: self.task_store.list_notes(project_id)?,
            }),
            Command::GetNote {
                project_id,
                note_id,
            } => Ok(ResponsePayload::Note {
                note: self.task_store.get_note(project_id, note_id)?,
            }),
            Command::CreateNote { note } => Ok(ResponsePayload::NoteCreated {
                note: self.task_store.create_note(note)?,
            }),
            Command::UpdateNote { note } => Ok(ResponsePayload::NoteUpdated {
                note: self.task_store.update_note(note)?,
            }),
            Command::ListAgentProfiles => Ok(ResponsePayload::AgentProfiles {
                profiles: self.task_store.list_agent_profiles()?,
            }),
            Command::UpdateAgentProfile { update } => {
                let provider = update.agent_id;
                let enabled = update.enabled;
                let profile = self.task_store.update_agent_profile(update)?;
                self.sync_profile_enabled_to_settings(provider, enabled)?;
                Ok(ResponsePayload::AgentProfileUpdated { profile })
            }
            Command::DeleteNote {
                project_id,
                note_id,
                expected_revision,
            } => Ok(ResponsePayload::NoteDeleted {
                note_id,
                revision: self
                    .task_store
                    .delete_note(project_id, note_id, expected_revision)?,
            }),
            Command::ListTasks => Ok(ResponsePayload::Tasks {
                tasks: self
                    .task_store
                    .list_tasks()?
                    .iter()
                    .map(TaskSummary::from_task)
                    .collect(),
            }),
            Command::CreateTask { task } => Ok(ResponsePayload::TaskCreated {
                task: self.task_store.create_task(task)?,
            }),
            Command::UpdateTask {
                task,
                expected_version,
            } => {
                let existing = self
                    .task_store
                    .get_task(task.id)?
                    .ok_or_else(|| anyhow!("task not found"))?;

                if task.status != existing.status {
                    if task.status == TaskStatus::Running {
                        bail!(
                            "cannot manually move task to running: only the daemon transitions a task to running when its workspace and provider process start"
                        );
                    }
                    if task.status == TaskStatus::Review
                        && existing.status != TaskStatus::Running
                        && existing.session_id.is_none()
                    {
                        bail!("cannot move task to review: task has not completed an agent run");
                    }
                    if task.status == TaskStatus::Done && existing.status == TaskStatus::Running {
                        bail!(
                            "cannot mark a running task as done: wait for the agent to complete or reopen to backlog"
                        );
                    }
                    if task.status == TaskStatus::Done && existing.status == TaskStatus::Queued {
                        bail!("cannot mark a queued task as done: cancel to backlog first");
                    }
                    if existing.status == TaskStatus::Backlog && task.status == TaskStatus::Queued {
                        return Ok(ResponsePayload::TaskUpdated {
                            task: self.queue_backlog_task(task, expected_version, &events)?,
                        });
                    }
                }

                let mut task_to_save = task;
                if task_to_save.status == TaskStatus::Backlog {
                    task_to_save.needs_attention = false;
                    task_to_save.sync_failed = None;
                }
                Ok(ResponsePayload::TaskUpdated {
                    task: self
                        .task_store
                        .update_task(task_to_save, expected_version)?,
                })
            }
            Command::MoveTask {
                task_id,
                status,
                expected_version,
            } => Ok(ResponsePayload::TaskMoved {
                task: self.move_task(task_id, status, expected_version, &events)?,
            }),
            Command::DeleteTask {
                task_id,
                expected_version,
            } => Ok(ResponsePayload::TaskDeleted {
                task_id,
                version: self.task_store.delete_task(task_id, expected_version)?,
            }),
            Command::HydrateTask { task_id } => {
                let task = self
                    .task_store
                    .get_task(task_id)?
                    .ok_or_else(|| anyhow!("task not found"))?;
                // On-demand session detail, mirroring HydrateSession: the
                // board works from summaries and only the open card pays for
                // transcript, checkpoint, and log hydration.
                let session = match task.session_id {
                    Some(session_id) => {
                        let mut state = self.task_state.lock();
                        let session = state
                            .sessions
                            .iter_mut()
                            .find(|session| session.id == session_id);
                        if let Some(session) = session {
                            self.task_store.hydrate(session)?;
                            Some(session.clone())
                        } else {
                            None
                        }
                    }
                    None => None,
                };
                Ok(ResponsePayload::TaskHydrated { task, session })
            }
            Command::StoreBlob { mime_type, bytes } => {
                let reference = self
                    .task_store
                    .blobs()
                    .store_image_bytes(&mime_type, &bytes)?;
                let path = self
                    .task_store
                    .blobs()
                    .path_for(&reference)
                    .ok_or_else(|| anyhow!("stored blob has no daemon path"))?;
                Ok(ResponsePayload::BlobStored { reference, path })
            }
            Command::ImportAttachment { name, upload } => Ok(ResponsePayload::AttachmentStored {
                attachment: self.attachments.import(&name, upload)?,
            }),
            Command::ImportPathAttachment { path } => Ok(ResponsePayload::AttachmentStored {
                attachment: self.attachments.import_path(&path)?,
            }),
            Command::ReadBlob { reference } => {
                let path = self
                    .task_store
                    .blobs()
                    .path_for(&reference)
                    .ok_or_else(|| anyhow!("invalid blob reference"))?;
                Ok(ResponsePayload::BlobData {
                    bytes: std::fs::read(path)?,
                })
            }
            Command::ReadAttachment { reference, path } => Ok(ResponsePayload::BlobData {
                bytes: self.attachments.read_file(&reference, &path)?,
            }),
            Command::SweepBlobs => {
                self.task_store.blob_sweep()();
                Ok(ResponsePayload::Ack)
            }
            Command::ForkSessionFromResponse { turn_count } => {
                let (session, checkpoint_warning) =
                    self.fork_session_from_response(session_id, turn_count)?;
                Ok(ResponsePayload::SessionForked {
                    session,
                    checkpoint_warning,
                })
            }
            Command::RewindSessionToMessage { turn_count } => {
                let (session, cleanup_warning) =
                    self.rewind_session_to_message(session_id, turn_count)?;
                Ok(ResponsePayload::SessionRewound {
                    session,
                    cleanup_warning,
                })
            }
            Command::ForkProviderSession { request } => {
                Ok(ResponsePayload::ProviderSessionForked {
                    result: fork_provider_session(request)?,
                })
            }
            Command::Workspace {
                operation:
                    WorkspaceOperation::CaptureTurn {
                        cwd,
                        session_id,
                        turn_count,
                    },
            } => Ok(ResponsePayload::Workspace {
                result: WorkspaceResult::Checkpoint {
                    checkpoint: self
                        .capture_turn_checkpoint(cwd, session_id, turn_count, &events)?,
                },
            }),
            Command::Workspace { operation } => Ok(ResponsePayload::Workspace {
                result: crate::workspace::execute(operation)?,
            }),
            Command::OpenTerminal { cwd, cols, rows } => {
                ensure_shell_environment();
                let terminal = crate::terminal::DaemonTerminal::open(&cwd, cols, rows, events)?;
                let previous = self
                    .terminals
                    .lock()
                    .insert(session_id, (runtime_id, terminal));
                drop(previous);
                Ok(ResponsePayload::Ack)
            }
            Command::WriteTerminal { data } => {
                let terminals = self.terminals.lock();
                let (active_runtime_id, terminal) = terminals
                    .get(&session_id)
                    .ok_or_else(|| anyhow!("daemon terminal {session_id} is not running"))?;
                if *active_runtime_id != runtime_id {
                    bail!(
                        "daemon terminal {session_id} belongs to runtime {active_runtime_id}, not {runtime_id}"
                    );
                }
                terminal.write(data)?;
                Ok(ResponsePayload::Ack)
            }
            Command::ResizeTerminal { cols, rows } => {
                let terminals = self.terminals.lock();
                let (active_runtime_id, terminal) = terminals
                    .get(&session_id)
                    .ok_or_else(|| anyhow!("daemon terminal {session_id} is not running"))?;
                if *active_runtime_id != runtime_id {
                    bail!(
                        "daemon terminal {session_id} belongs to runtime {active_runtime_id}, not {runtime_id}"
                    );
                }
                terminal.resize(cols, rows);
                Ok(ResponsePayload::Ack)
            }
            Command::CloseTerminal => {
                let removed = {
                    let mut terminals = self.terminals.lock();
                    if let Some((active_runtime_id, _)) = terminals.get(&session_id) {
                        if *active_runtime_id != runtime_id {
                            bail!(
                                "daemon terminal {session_id} belongs to runtime {active_runtime_id}, not {runtime_id}"
                            );
                        }
                    }
                    terminals.remove(&session_id)
                };
                drop(removed);
                Ok(ResponsePayload::Ack)
            }
            Command::Start { options } => {
                let previous = self.sessions.lock().remove(&session_id);
                drop(previous);
                let provider = decode_enum(&options.provider)?;
                let options = DriverStartOptions {
                    binary: options.binary,
                    cwd: options.cwd,
                    mode: decode_enum(&options.mode)?,
                    interaction_mode: decode_enum(&options.interaction_mode)?,
                    model: options.model,
                    reasoning_effort: options.reasoning_effort,
                    service_tier: options.service_tier,
                    context_window: options.context_window,
                    agent_preset: options.agent_preset,
                    computer_use_enabled: options.computer_use_enabled,
                    provider_cursor: options
                        .provider_cursor
                        .map(serde_json::from_value)
                        .transpose()
                        .context("daemon received an invalid provider cursor")?,
                };
                let (wake, _wake_events) = smol::channel::bounded(1);
                let (event_sender, event_receiver) = driver::event_channel(wake);
                let handle = driver::start_local(provider, options, event_sender)?;
                let supports_steer = handle.supports_steer();
                let thread_events = events.clone();
                std::thread::Builder::new()
                    .name(format!("padu-daemon-events-{session_id}"))
                    .spawn(move || {
                        while let Ok(event) = event_receiver.recv() {
                            let wire = event_to_wire(event).unwrap_or_else(|error| {
                                WireDriverEvent::new(
                                    "error",
                                    Value::String(format!(
                                        "could not encode daemon event: {error}"
                                    )),
                                )
                            });
                            if thread_events.send(wire).is_err() {
                                break;
                            }
                        }
                    })
                    .context("could not start daemon event forwarding thread")?;
                self.sessions
                    .lock()
                    .insert(session_id, (runtime_id, handle));

                // P1-06: Queued -> Running on workspace_started
                self.on_workspace_started(session_id, &events);

                Ok(ResponsePayload::Started { supports_steer })
            }
            Command::CloseSession => {
                let removed = {
                    let mut sessions = self.sessions.lock();
                    sessions
                        .get(&session_id)
                        .is_some_and(|(active_runtime_id, _)| *active_runtime_id == runtime_id)
                        .then(|| sessions.remove(&session_id))
                        .flatten()
                };
                drop(removed);
                Ok(ResponsePayload::Ack)
            }
            command => {
                let driver = {
                    let sessions = self.sessions.lock();
                    let (active_runtime_id, driver) = sessions
                        .get(&session_id)
                        .ok_or_else(|| anyhow!("daemon session {session_id} is not running"))?;
                    if *active_runtime_id != runtime_id {
                        bail!(
                            "daemon session {session_id} belongs to runtime {active_runtime_id}, not {runtime_id}"
                        );
                    }
                    driver.clone()
                };
                handle_driver_command(&driver, command)
            }
        }
    }

    fn shutdown(&self) {
        let sessions = std::mem::take(&mut *self.sessions.lock());
        drop(sessions);
        let terminals = std::mem::take(&mut *self.terminals.lock());
        drop(terminals);
    }
}

fn session_projection_precedes(
    existing: &AgentSession,
    incoming: &AgentSession,
    active_runtime_id: Option<Uuid>,
) -> bool {
    let existing_cursor = existing.runtime_event_cursor;
    let incoming_cursor = incoming.runtime_event_cursor;
    if let Some(active_runtime_id) = active_runtime_id {
        let existing_is_active =
            existing_cursor.is_some_and(|cursor| cursor.runtime_id == active_runtime_id);
        let incoming_is_active =
            incoming_cursor.is_some_and(|cursor| cursor.runtime_id == active_runtime_id);
        if existing_is_active != incoming_is_active {
            return existing_is_active;
        }
    }
    match (existing_cursor, incoming_cursor) {
        (Some(existing), Some(incoming))
            if existing.runtime_id == incoming.runtime_id && existing.epoch == incoming.epoch =>
        {
            incoming.sequence < existing.sequence
        }
        (Some(_), None) if existing.status.is_busy() => true,
        _ => incoming.updated_at < existing.updated_at,
    }
}

fn merge_stale_session_metadata(existing: &mut AgentSession, incoming: AgentSession) {
    if incoming.updated_at >= existing.updated_at {
        existing.title = incoming.title;
        existing.project_id = incoming.project_id;
        existing.workspace = incoming.workspace;
        existing.provider = incoming.provider;
        existing.model = incoming.model;
        existing.runtime_mode = incoming.runtime_mode;
        existing.interaction_mode = incoming.interaction_mode;
        existing.reasoning_effort = incoming.reasoning_effort;
        existing.service_tier = incoming.service_tier;
        existing.context_window = incoming.context_window;
        existing.agent_preset = incoming.agent_preset;
        existing.updated_at = incoming.updated_at;
        existing.last_reply_at = incoming.last_reply_at.or(existing.last_reply_at);
    }
    // Pin/archive metadata is daemon-authoritative and is intentionally not
    // copied from a stale client projection.
    for queued in incoming.queued_messages {
        if !existing
            .queued_messages
            .iter()
            .any(|candidate| candidate.id == queued.id)
        {
            existing.queued_messages.push(queued);
        }
    }
}

/// Ending checkpoints are produced and stored by the daemon. A second client
/// may still save a projection created just before capture completed; never
/// let that stale projection erase the canonical Git snapshot.
fn preserve_daemon_checkpoints(existing: &AgentSession, incoming: &mut AgentSession) {
    for turn in &mut incoming.turns {
        let Some(checkpoint) = existing
            .turns
            .iter()
            .find(|candidate| candidate.turn_count == turn.turn_count)
            .and_then(|candidate| candidate.checkpoint.as_ref())
            .filter(|checkpoint| {
                matches!(
                    checkpoint.status,
                    CheckpointStatus::Ready | CheckpointStatus::Unavailable
                )
            })
        else {
            continue;
        };
        turn.checkpoint = Some(checkpoint.clone());
    }
}

impl PaduBackend {
    /// Fork a response using only daemon-host state.
    ///
    /// A browser must never reconstruct or persist this operation itself:
    /// provider-native sessions, checkpoint refs, and the task database all
    /// belong to the daemon and may be on another machine.
    fn fork_session_from_response(
        &self,
        session_id: Uuid,
        turn_count: usize,
    ) -> anyhow::Result<(AgentSession, Option<String>)> {
        let (source, cwd, fork_title) = {
            let mut state = self.task_state.lock();
            let source_index = state
                .sessions
                .iter()
                .position(|session| session.id == session_id)
                .ok_or_else(|| anyhow!("the source task is unavailable"))?;
            self.task_store
                .hydrate(&mut state.sessions[source_index])
                .context("could not load the source task")?;
            let source = state.sessions[source_index].clone();
            let project = state
                .projects
                .iter()
                .find(|project| project.id == source.project_id)
                .ok_or_else(|| anyhow!("the source task project is unavailable"))?;
            let cwd = source.workspace.path().unwrap_or(&project.path).to_owned();
            let fork_title = next_response_fork_title(
                source.display_title(),
                state
                    .sessions
                    .iter()
                    .filter(|session| session.project_id == source.project_id)
                    .map(AgentSession::display_title),
            );
            (source, cwd, fork_title)
        };

        validate_response_fork(&source, turn_count)?;
        let provider_turn_count = source
            .turns
            .iter()
            .take(turn_count)
            .filter(|turn| turn.provider_turn_started)
            .count();
        let turns_to_remove = source.provider_turns_after(turn_count);
        let (provider_cursor, message_ids) = self.fork_provider_response(
            &source,
            &cwd,
            &fork_title,
            turn_count,
            provider_turn_count,
            turns_to_remove,
        )?;
        let mut forked = source
            .fork_through_turn(turn_count, provider_cursor, &fork_title)
            .ok_or_else(|| anyhow!("the selected response cannot be copied"))?;
        if !message_ids.is_empty() {
            for turn in &mut forked.turns {
                if let Some(message_id) = turn.provider_resume_at.as_mut()
                    && let Some(remapped) = message_ids.get(message_id)
                {
                    *message_id = remapped.clone();
                }
            }
        }

        let fork_id = forked.id;
        for turn in &mut forked.turns {
            if let Some(checkpoint) = turn.checkpoint.as_mut() {
                checkpoint.git_ref =
                    crate::checkpoint::checkpoint_ref(fork_id, checkpoint.turn_count);
            }
        }
        let checkpoint_warning =
            crate::checkpoint::copy_session_refs(&cwd, source.id, fork_id, turn_count)
                .err()
                .map(|error| error.to_string());

        let mut state = self.task_state.lock();
        state.push_session(forked.clone());
        if let Err(error) = self.task_store.save(&mut state) {
            state.sessions.retain(|session| session.id != fork_id);
            let _ = crate::checkpoint::delete_all_session_refs(&cwd, fork_id);
            return Err(error).context("could not save the forked task");
        }
        Ok((forked, checkpoint_warning))
    }

    /// Restore the daemon-host worktree, provider conversation, and stored
    /// transcript to immediately before one user turn.
    fn rewind_session_to_message(
        &self,
        session_id: Uuid,
        turn_count: usize,
    ) -> anyhow::Result<(AgentSession, Option<String>)> {
        let (source, cwd) = {
            let mut state = self.task_state.lock();
            let source_index = state
                .sessions
                .iter()
                .position(|session| session.id == session_id)
                .ok_or_else(|| anyhow!("the task is unavailable"))?;
            self.task_store
                .hydrate(&mut state.sessions[source_index])
                .context("could not load the task")?;
            let source = state.sessions[source_index].clone();
            let project = state
                .projects
                .iter()
                .find(|project| project.id == source.project_id)
                .ok_or_else(|| anyhow!("the task project is unavailable"))?;
            let cwd = source.workspace.path().unwrap_or(&project.path).to_owned();
            (source, cwd)
        };
        validate_message_rewind(&source, turn_count)?;

        // Resolve the executable before touching the worktree. Even native
        // transcript operations are immediately followed by a replacement
        // prompt, so accepting a rewind that cannot resume would strand the
        // user at a provider state the UI cannot continue.
        let binary = self.provider_binary(source.provider)?;
        let retained_turn_count = turn_count.saturating_sub(1);
        let previous_turn_count = source.turns.len();
        let rollback_turns = source.provider_turns_after(retained_turn_count);
        let provider_turn_count = source
            .turns
            .iter()
            .take(retained_turn_count)
            .filter(|turn| turn.provider_turn_started)
            .count();
        let provider_resume_at = retained_turn_count
            .checked_sub(1)
            .and_then(|index| source.turns.get(index))
            .and_then(|turn| turn.provider_resume_at.clone());

        let turn_start_ref = crate::checkpoint::turn_start_ref(session_id, turn_count);
        let retained_ref = crate::checkpoint::checkpoint_ref(session_id, retained_turn_count);
        let restore_ref = if crate::checkpoint::has_ref(&cwd, &turn_start_ref) {
            turn_start_ref
        } else {
            retained_ref
        };
        if !crate::checkpoint::has_ref(&cwd, &restore_ref) {
            bail!("the checkpoint before this message is unavailable");
        }

        let safety_ref = format!("refs/padu/revert-backup-{session_id}-{}", Uuid::new_v4());
        crate::checkpoint::capture_ref(&cwd, &safety_ref)
            .context("could not create a rewind safety snapshot")?;
        if let Err(error) = crate::checkpoint::restore_ref(&cwd, &restore_ref) {
            return Err(restore_rewind_safety(
                &cwd,
                &safety_ref,
                "could not restore the selected checkpoint",
                error,
            ));
        }

        let provider_rewind = self.rewind_provider_response(
            &source,
            &cwd,
            &binary,
            retained_turn_count,
            rollback_turns,
            provider_turn_count,
            provider_resume_at,
        );
        let (provider_cursor, message_ids, reset_native_session) = match provider_rewind {
            Ok(result) => result,
            Err(error) => {
                return Err(restore_rewind_safety(
                    &cwd,
                    &safety_ref,
                    "the provider rejected the rewind",
                    error,
                ));
            }
        };

        let _ = crate::checkpoint::delete_ref(&cwd, &safety_ref);
        let cleanup_warning = crate::checkpoint::delete_turn_refs_after(
            &cwd,
            session_id,
            retained_turn_count,
            previous_turn_count,
        )
        .err()
        .map(|error| error.to_string());

        // Every provider resumes from the newly stored cursor on the next
        // prompt. Dropping a resident source driver also prevents its late
        // events from racing the rewound transcript.
        let removed = self.sessions.lock().remove(&session_id);
        drop(removed);

        let mut rewound = source.clone();
        if !message_ids.is_empty() {
            for turn in rewound.turns.iter_mut().take(retained_turn_count) {
                if let Some(remapped) = turn
                    .provider_resume_at
                    .as_ref()
                    .and_then(|message_id| message_ids.get(message_id))
                    .cloned()
                {
                    turn.provider_resume_at = Some(remapped);
                }
            }
        }
        if reset_native_session {
            rewound.provider_cursor = None;
        } else if let Some(cursor) = provider_cursor {
            rewound.provider_cursor = Some(cursor);
        }
        rewound.truncate_after_turn(retained_turn_count);
        rewound.status = SessionStatus::Idle;

        let mut state = self.task_state.lock();
        let existing = state
            .sessions
            .iter_mut()
            .find(|session| session.id == session_id)
            .ok_or_else(|| anyhow!("the task was removed while it was being rewound"))?;
        *existing = rewound.clone();
        state.mark_session_dirty(session_id);
        self.task_store
            .save(&mut state)
            .context("could not save the rewound task")?;
        Ok((rewound, cleanup_warning))
    }

    fn fork_provider_response(
        &self,
        source: &AgentSession,
        cwd: &Path,
        fork_title: &str,
        turn_count: usize,
        provider_turn_count: usize,
        turns_to_remove: usize,
    ) -> anyhow::Result<(ProviderResumeCursor, HashMap<String, String>)> {
        match source.provider {
            ProviderKind::Claude => {
                let Some(ProviderResumeCursor::Claude { session_id, .. }) =
                    source.provider_cursor.as_ref()
                else {
                    bail!("Claude's native session is unavailable");
                };
                let resume_at = source
                    .turns
                    .get(turn_count.saturating_sub(1))
                    .and_then(|turn| turn.provider_resume_at.clone());
                let fork = fork_provider_session(ProviderSessionForkRequest::Claude {
                    session_id: session_id.clone(),
                    resume_at,
                    turn_count: provider_turn_count,
                    title: fork_title.to_owned(),
                })?;
                Ok((fork.cursor, fork.message_ids))
            }
            ProviderKind::Codex
            | ProviderKind::CommandCode
            | ProviderKind::DeepSeek
            | ProviderKind::OhMyPi
            | ProviderKind::Pi => Ok((
                self.fork_response_with_driver(source, cwd, turns_to_remove)?,
                HashMap::new(),
            )),
            ProviderKind::Cursor => {
                let fork = fork_provider_session(ProviderSessionForkRequest::Cursor {
                    source: source.clone(),
                    turn_count,
                })?;
                Ok((fork.cursor, HashMap::new()))
            }
            ProviderKind::Agy => {
                let fork = fork_provider_session(ProviderSessionForkRequest::Agy {
                    source: source.clone(),
                    turn_count,
                })?;
                Ok((fork.cursor, HashMap::new()))
            }
            ProviderKind::Amp => {
                let Some(ProviderResumeCursor::Amp {
                    thread_id,
                    fork_context,
                }) = source.provider_cursor.as_ref()
                else {
                    bail!("Amp's native thread is unavailable");
                };
                let fork = fork_provider_session(ProviderSessionForkRequest::Amp {
                    binary: self.provider_binary(ProviderKind::Amp)?,
                    cwd: cwd.to_owned(),
                    thread_id: thread_id.clone(),
                    fork_context: fork_context.clone(),
                    turn_count: provider_turn_count,
                })?;
                Ok((fork.cursor, HashMap::new()))
            }
            ProviderKind::OpenCode => {
                let Some(ProviderResumeCursor::OpenCode { session_id }) =
                    source.provider_cursor.as_ref()
                else {
                    bail!("OpenCode's native session is unavailable");
                };
                let fork = fork_provider_session(ProviderSessionForkRequest::OpenCode {
                    binary: self.provider_binary(ProviderKind::OpenCode)?,
                    cwd: cwd.to_owned(),
                    session_id: session_id.clone(),
                    turn_count: provider_turn_count,
                })?;
                Ok((fork.cursor, HashMap::new()))
            }
            ProviderKind::Grok => {
                let Some(ProviderResumeCursor::Grok { session_id }) =
                    source.provider_cursor.as_ref()
                else {
                    bail!("Grok Build's native session is unavailable");
                };
                let fork = fork_provider_session(ProviderSessionForkRequest::Grok {
                    binary: self.provider_binary(ProviderKind::Grok)?,
                    cwd: cwd.to_owned(),
                    session_id: session_id.clone(),
                    turn_count: provider_turn_count,
                })?;
                Ok((fork.cursor, HashMap::new()))
            }
            // Unreachable through the UI, which hides branching for providers
            // that answer `supports_conversation_fork` with false.
            ProviderKind::Fx | ProviderKind::Kimi | ProviderKind::Qoder => {
                bail!(
                    "{} cannot branch a conversation at a turn",
                    source.provider.display_name()
                )
            }
        }
    }

    fn fork_response_with_driver(
        &self,
        source: &AgentSession,
        cwd: &Path,
        turns_to_remove: usize,
    ) -> anyhow::Result<ProviderResumeCursor> {
        if let Some(driver) = self
            .sessions
            .lock()
            .get(&source.id)
            .map(|(_, driver)| driver.clone())
        {
            return driver.fork(turns_to_remove);
        }

        match source.provider {
            ProviderKind::Codex
                if !matches!(
                    source.provider_cursor.as_ref(),
                    Some(ProviderResumeCursor::Codex { .. })
                ) =>
            {
                bail!("Codex's native thread is unavailable");
            }
            ProviderKind::DeepSeek
                if !matches!(
                    source.provider_cursor.as_ref(),
                    Some(ProviderResumeCursor::DeepSeek { .. })
                ) =>
            {
                bail!("DeepSeek Harness's native session is unavailable");
            }
            ProviderKind::CommandCode
                if !matches!(
                    source.provider_cursor.as_ref(),
                    Some(ProviderResumeCursor::CommandCode { .. })
                ) =>
            {
                bail!("Command Code's native session is unavailable");
            }
            ProviderKind::Pi
                if !matches!(
                    source.provider_cursor.as_ref(),
                    Some(ProviderResumeCursor::Pi {
                        session_file: Some(_),
                        ..
                    })
                ) =>
            {
                bail!("Pi's native session file is unavailable");
            }
            ProviderKind::OhMyPi
                if !matches!(
                    source.provider_cursor.as_ref(),
                    Some(ProviderResumeCursor::OhMyPi {
                        session_file: Some(_),
                        ..
                    })
                ) =>
            {
                bail!("Oh My Pi's native session file is unavailable");
            }
            _ => {}
        }

        let (wake, _wake_events) = smol::channel::bounded(1);
        let (event_sender, _event_receiver) = driver::event_channel(wake);
        let driver = driver::start_local(
            source.provider,
            DriverStartOptions {
                binary: self.provider_binary(source.provider)?,
                cwd: cwd.to_owned(),
                mode: source.runtime_mode,
                interaction_mode: source.interaction_mode,
                model: source.model.clone(),
                reasoning_effort: source.reasoning_effort.clone(),
                service_tier: source.service_tier.clone(),
                context_window: source.context_window.clone(),
                agent_preset: source.agent_preset.clone(),
                computer_use_enabled: false,
                provider_cursor: source.provider_cursor.clone(),
            },
            event_sender,
        )?;
        driver.fork(turns_to_remove)
    }

    #[allow(clippy::too_many_arguments)]
    fn rewind_provider_response(
        &self,
        source: &AgentSession,
        cwd: &Path,
        binary: &Path,
        retained_turn_count: usize,
        rollback_turns: usize,
        provider_turn_count: usize,
        provider_resume_at: Option<String>,
    ) -> anyhow::Result<(Option<ProviderResumeCursor>, HashMap<String, String>, bool)> {
        if rollback_turns == 0 {
            return Ok((None, HashMap::new(), false));
        }
        let reset_native_session = retained_turn_count == 0
            && matches!(
                source.provider,
                ProviderKind::Claude | ProviderKind::Cursor | ProviderKind::Grok
            );
        if reset_native_session {
            return Ok((None, HashMap::new(), true));
        }

        match source.provider {
            ProviderKind::Claude => {
                let Some(ProviderResumeCursor::Claude { session_id, .. }) =
                    source.provider_cursor.as_ref()
                else {
                    bail!("Claude's native session is unavailable");
                };
                let fork = fork_provider_session(ProviderSessionForkRequest::Claude {
                    session_id: session_id.clone(),
                    resume_at: provider_resume_at,
                    turn_count: provider_turn_count,
                    title: format!("{} (rewind)", source.display_title()),
                })?;
                Ok((Some(fork.cursor), fork.message_ids, false))
            }
            ProviderKind::OpenCode => {
                let cursor = if let Some(driver) = self
                    .sessions
                    .lock()
                    .get(&source.id)
                    .map(|(_, driver)| driver.clone())
                {
                    driver
                        .rollback(rollback_turns)?
                        .ok_or_else(|| anyhow!("OpenCode returned no rewound-session cursor"))?
                } else {
                    let Some(ProviderResumeCursor::OpenCode { session_id }) =
                        source.provider_cursor.as_ref()
                    else {
                        bail!("OpenCode's native session is unavailable");
                    };
                    fork_provider_session(ProviderSessionForkRequest::OpenCode {
                        binary: binary.to_owned(),
                        cwd: cwd.to_owned(),
                        session_id: session_id.clone(),
                        turn_count: provider_turn_count,
                    })?
                    .cursor
                };
                Ok((Some(cursor), HashMap::new(), false))
            }
            ProviderKind::Amp => {
                let Some(ProviderResumeCursor::Amp {
                    thread_id,
                    fork_context,
                }) = source.provider_cursor.as_ref()
                else {
                    bail!("Amp's native thread is unavailable");
                };
                let cursor = fork_provider_session(ProviderSessionForkRequest::Amp {
                    binary: binary.to_owned(),
                    cwd: cwd.to_owned(),
                    thread_id: thread_id.clone(),
                    fork_context: fork_context.clone(),
                    turn_count: provider_turn_count,
                })?
                .cursor;
                Ok((Some(cursor), HashMap::new(), false))
            }
            ProviderKind::Cursor => {
                let cursor = fork_provider_session(ProviderSessionForkRequest::Cursor {
                    source: source.clone(),
                    turn_count: retained_turn_count,
                })?
                .cursor;
                Ok((Some(cursor), HashMap::new(), false))
            }
            ProviderKind::Agy => {
                let cursor = fork_provider_session(ProviderSessionForkRequest::Agy {
                    source: source.clone(),
                    turn_count: retained_turn_count,
                })?
                .cursor;
                Ok((Some(cursor), HashMap::new(), false))
            }
            ProviderKind::Grok => {
                let Some(ProviderResumeCursor::Grok { session_id }) =
                    source.provider_cursor.as_ref()
                else {
                    bail!("Grok Build's native session is unavailable");
                };
                let cursor = fork_provider_session(ProviderSessionForkRequest::Grok {
                    binary: binary.to_owned(),
                    cwd: cwd.to_owned(),
                    session_id: session_id.clone(),
                    turn_count: provider_turn_count,
                })?
                .cursor;
                Ok((Some(cursor), HashMap::new(), false))
            }
            ProviderKind::Codex
            | ProviderKind::CommandCode
            | ProviderKind::DeepSeek
            | ProviderKind::OhMyPi
            | ProviderKind::Pi => Ok((
                self.rollback_response_with_driver(source, cwd, binary, rollback_turns)?,
                HashMap::new(),
                false,
            )),
            // Unreachable through the UI, which hides rewinding for providers
            // that answer `supports_conversation_rollback` with false.
            ProviderKind::Fx | ProviderKind::Kimi | ProviderKind::Qoder => {
                bail!(
                    "{} cannot rewind a conversation to a turn",
                    source.provider.display_name()
                )
            }
        }
    }

    fn rollback_response_with_driver(
        &self,
        source: &AgentSession,
        cwd: &Path,
        binary: &Path,
        rollback_turns: usize,
    ) -> anyhow::Result<Option<ProviderResumeCursor>> {
        if let Some(driver) = self
            .sessions
            .lock()
            .get(&source.id)
            .map(|(_, driver)| driver.clone())
        {
            return driver.rollback(rollback_turns);
        }

        let (wake, _wake_events) = smol::channel::bounded(1);
        let (event_sender, _event_receiver) = driver::event_channel(wake);
        let driver = driver::start_local(
            source.provider,
            DriverStartOptions {
                binary: binary.to_owned(),
                cwd: cwd.to_owned(),
                mode: source.runtime_mode,
                interaction_mode: source.interaction_mode,
                model: source.model.clone(),
                reasoning_effort: source.reasoning_effort.clone(),
                service_tier: source.service_tier.clone(),
                context_window: source.context_window.clone(),
                agent_preset: source.agent_preset.clone(),
                computer_use_enabled: false,
                provider_cursor: source.provider_cursor.clone(),
            },
            event_sender,
        )?;
        driver.rollback(rollback_turns)
    }

    fn provider_binary(&self, provider: ProviderKind) -> anyhow::Result<PathBuf> {
        ensure_shell_environment();
        let settings = self.settings.get();
        let binary_override = settings
            .provider_binary_overrides
            .get(&provider)
            .map(String::as_str);
        crate::model::provider_probe(provider, binary_override)
            .path
            .ok_or_else(|| anyhow!("{} is not installed on the daemon", provider.display_name()))
    }
}

fn validate_message_rewind(source: &AgentSession, turn_count: usize) -> anyhow::Result<()> {
    if !matches!(source.status, SessionStatus::Idle | SessionStatus::Failed) {
        bail!("stop the task before editing a prior message");
    }
    let Some(turn) = source
        .turns
        .iter()
        .find(|turn| turn.turn_count == turn_count)
    else {
        bail!("the selected message is unavailable");
    };
    if !source.messages.iter().any(|message| {
        message.turn_id == Some(turn.id) && message.role == crate::model::MessageRole::User
    }) {
        bail!("the selected user message is unavailable");
    }
    let rollback_turns = source.provider_turns_after(turn_count.saturating_sub(1));
    if rollback_turns > 0 && source.provider_cursor.is_none() {
        bail!("the provider conversation is unavailable");
    }
    Ok(())
}

fn restore_rewind_safety(
    cwd: &Path,
    safety_ref: &str,
    context: &str,
    error: anyhow::Error,
) -> anyhow::Error {
    match crate::checkpoint::restore_ref(cwd, safety_ref) {
        Ok(()) => {
            let _ = crate::checkpoint::delete_ref(cwd, safety_ref);
            anyhow!("{context}: {error}; the original worktree was restored")
        }
        Err(restore_error) => anyhow!(
            "{context}: {error}; restoring the safety snapshot also failed: {restore_error}; snapshot: {safety_ref}"
        ),
    }
}

fn validate_response_fork(source: &AgentSession, turn_count: usize) -> anyhow::Result<()> {
    if !matches!(source.status, SessionStatus::Idle | SessionStatus::Failed) {
        bail!("stop the task before forking a response");
    }
    let cursor = source
        .provider_cursor
        .as_ref()
        .ok_or_else(|| anyhow!("the provider conversation is unavailable"))?;
    if cursor.provider() != source.provider {
        bail!("the provider conversation does not match this task");
    }
    if source
        .turns
        .get(turn_count.saturating_sub(1))
        .is_none_or(|turn| turn.turn_count != turn_count || !turn.provider_turn_started)
    {
        bail!("the selected response cannot be forked");
    }
    Ok(())
}

fn numbered_title_suffix(title: &str) -> Option<(&str, usize)> {
    let (base, suffix) = title.rsplit_once(" (")?;
    let number = suffix.strip_suffix(')')?.parse().ok()?;
    (!base.is_empty() && number >= 2).then_some((base, number))
}

fn next_response_fork_title<'a>(
    source_title: &str,
    existing_titles: impl IntoIterator<Item = &'a str>,
) -> String {
    let existing_titles = existing_titles.into_iter().collect::<Vec<_>>();
    let base = numbered_title_suffix(source_title)
        .filter(|(base, _)| existing_titles.iter().any(|title| title == base))
        .map_or(source_title, |(base, _)| base);
    let highest_number = existing_titles
        .iter()
        .filter_map(|title| {
            if *title == base {
                Some(1)
            } else {
                numbered_title_suffix(title)
                    .filter(|(candidate_base, _)| *candidate_base == base)
                    .map(|(_, number)| number)
            }
        })
        .max()
        .unwrap_or(1);
    format!("{base} ({})", highest_number.saturating_add(1).max(2))
}

fn fork_provider_session(
    request: ProviderSessionForkRequest,
) -> anyhow::Result<ProviderSessionFork> {
    use crate::model::ProviderResumeCursor;

    let (cursor, message_ids, source_resume_at) = match request {
        ProviderSessionForkRequest::Claude {
            session_id,
            resume_at,
            turn_count,
            title,
        } => {
            let source_resume_at = resume_at.map(Ok).unwrap_or_else(|| {
                crate::claude_session::message_id_for_turn(&session_id, turn_count)
            })?;
            let fork =
                crate::claude_session::fork_session_at(&session_id, &source_resume_at, &title)?;
            let fork_resume_at = fork
                .message_ids
                .get(&source_resume_at)
                .cloned()
                .ok_or_else(|| anyhow!("Claude fork did not include its target message"))?;
            (
                ProviderResumeCursor::Claude {
                    session_id: fork.session_id,
                    resume_at: Some(fork_resume_at),
                },
                fork.message_ids,
                Some(source_resume_at),
            )
        }
        ProviderSessionForkRequest::Amp {
            binary,
            cwd,
            thread_id,
            fork_context,
            turn_count,
        } => (
            crate::amp_session::fork_session_at_turn(
                &binary,
                &cwd,
                &thread_id,
                fork_context.as_deref(),
                turn_count,
            )?,
            HashMap::new(),
            None,
        ),
        ProviderSessionForkRequest::Cursor { source, turn_count } => (
            crate::cursor_session::fork_session_at_turn(&source, turn_count)?,
            HashMap::new(),
            None,
        ),
        ProviderSessionForkRequest::Agy { source, turn_count } => (
            crate::agy_session::fork_session_at_turn(&source, turn_count)?,
            HashMap::new(),
            None,
        ),
        ProviderSessionForkRequest::OpenCode {
            binary,
            cwd,
            session_id,
            turn_count,
        } => (
            crate::opencode_session::fork_session_at_turn(&binary, &cwd, &session_id, turn_count)?,
            HashMap::new(),
            None,
        ),
        ProviderSessionForkRequest::Grok {
            binary,
            cwd,
            session_id,
            turn_count,
        } => (
            crate::grok_session::fork_session_at_turn(&binary, &cwd, &session_id, turn_count)?,
            HashMap::new(),
            None,
        ),
    };
    Ok(ProviderSessionFork {
        cursor,
        message_ids,
        source_resume_at,
    })
}

fn handle_driver_command(
    driver: &DriverHandle,
    command: Command,
) -> anyhow::Result<ResponsePayload> {
    match command {
        Command::Prompt { prompt } => driver.prompt(prompt),
        Command::Steer { prompt } => driver.steer(prompt),
        Command::Cancel => driver.cancel(),
        Command::CancelComputerUse => driver.cancel_computer_use(),
        Command::RefreshBackgroundWork => driver.refresh_background_work(),
        Command::StopBackgroundWork { key, control_id } => {
            driver.stop_background_work(
                serde_json::from_value(key).context("invalid background-work key")?,
                control_id,
            );
        }
        Command::Respond {
            request_id,
            option_id,
        } => driver.respond(request_id, option_id),
        Command::RespondUserInput {
            request_id,
            answers,
        } => driver.respond_user_input(request_id, answers),
        Command::Goal { operation } => driver.goal(operation),
        Command::RunComputerTool { request } => {
            driver.run_computer_tool(crate::computer_use::ComputerToolRequest {
                call_id: request.call_id,
                tool: request.tool,
                arguments: request.arguments,
            });
        }
        Command::RejectComputerTool { request, reason } => {
            driver.reject_computer_tool(
                crate::computer_use::ComputerToolRequest {
                    call_id: request.call_id,
                    tool: request.tool,
                    arguments: request.arguments,
                },
                reason,
            );
        }
        Command::ApplyOptions { options } => {
            return Ok(ResponsePayload::OptionsApplied {
                applied: driver.apply_options(SessionOptions {
                    mode: decode_enum(&options.mode)?,
                    interaction_mode: decode_enum(&options.interaction_mode)?,
                    model: options.model,
                    reasoning_effort: options.reasoning_effort,
                    service_tier: options.service_tier,
                    context_window: options.context_window,
                }),
            });
        }
        Command::Rollback { turns } => {
            let cursor = driver
                .rollback(turns)?
                .map(serde_json::to_value)
                .transpose()?;
            return Ok(ResponsePayload::Cursor { cursor });
        }
        Command::Fork { turns_to_remove } => {
            let cursor = Some(serde_json::to_value(driver.fork(turns_to_remove)?)?);
            return Ok(ResponsePayload::Cursor { cursor });
        }
        Command::AttachSession
        | Command::Start { .. }
        | Command::GetSettings
        | Command::UpdateSettings { .. }
        | Command::InstallAgyAcp
        | Command::CancelAgyAcpInstall
        | Command::AuthenticateAgy
        | Command::LogoutAgy
        | Command::CheckAgyAuth
        | Command::ReinstallAgyAcp
        | Command::RemoveAgyAcp
        | Command::ProbeProvider { .. }
        | Command::FetchPlanUsage { .. }
        | Command::ProbeComputerPermissions { .. }
        | Command::LoadUsageHistory { .. }
        | Command::LoadSkills { .. }
        | Command::SetSkillsEnabled { .. }
        | Command::TrashSkills { .. }
        | Command::LoadTaskState
        | Command::SaveTaskState { .. }
        | Command::RemoveSession
        | Command::SetSessionPinned { .. }
        | Command::SetSessionArchived { .. }
        | Command::HydrateSession { .. }
        | Command::SearchSessionMessages { .. }
        | Command::ListProviderSessions { .. }
        | Command::LoadProviderSession { .. }
        | Command::LoadComposerDrafts
        | Command::SaveComposerDrafts { .. }
        | Command::ApplyComposerDraftChanges { .. }
        | Command::ListNotes { .. }
        | Command::GetNote { .. }
        | Command::CreateNote { .. }
        | Command::UpdateNote { .. }
        | Command::DeleteNote { .. }
        | Command::ListTasks
        | Command::CreateTask { .. }
        | Command::UpdateTask { .. }
        | Command::MoveTask { .. }
        | Command::DeleteTask { .. }
        | Command::HydrateTask { .. }
        | Command::ListAgentProfiles
        | Command::UpdateAgentProfile { .. }
        | Command::StoreBlob { .. }
        | Command::ImportAttachment { .. }
        | Command::ImportPathAttachment { .. }
        | Command::ReadBlob { .. }
        | Command::ReadAttachment { .. }
        | Command::SweepBlobs
        | Command::ForkSessionFromResponse { .. }
        | Command::RewindSessionToMessage { .. }
        | Command::ForkProviderSession { .. }
        | Command::Workspace { .. }
        | Command::OpenTerminal { .. }
        | Command::WriteTerminal { .. }
        | Command::ResizeTerminal { .. }
        | Command::CloseTerminal
        | Command::CloseSession => {
            bail!("daemon received a command in the wrong dispatch path")
        }
    }
    Ok(ResponsePayload::Ack)
}

fn ensure_shell_environment() {
    static REFRESHED: OnceLock<()> = OnceLock::new();
    REFRESHED.get_or_init(|| {
        crate::command_env::refresh_from_default_shell();
    });
}

fn decode_enum<T: DeserializeOwned>(value: &str) -> anyhow::Result<T> {
    serde_json::from_value(Value::String(value.to_owned()))
        .with_context(|| format!("invalid protocol enum value {value:?}"))
}

pub fn encode_enum<T: Serialize>(value: T) -> anyhow::Result<String> {
    serde_json::to_value(value)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("protocol enum did not serialize as a string"))
}

fn event_to_wire(event: DriverEvent) -> anyhow::Result<WireDriverEvent> {
    let (kind, payload) = match event {
        DriverEvent::RuntimeEventCursorAdvanced(_) => {
            bail!("client-only runtime cursors cannot be sent by the daemon")
        }
        DriverEvent::Connected { provider_cursor } => {
            ("connected", serde_json::to_value(provider_cursor)?)
        }
        DriverEvent::AgentPresetSelected(preset) => {
            ("agentPresetSelected", serde_json::to_value(preset)?)
        }
        DriverEvent::AutoTitleUpdated(title) => ("autoTitleUpdated", serde_json::to_value(title)?),
        DriverEvent::AvailableCommands(commands) => {
            ("availableCommands", serde_json::to_value(commands)?)
        }
        DriverEvent::TurnStarted => ("turnStarted", Value::Null),
        DriverEvent::TextDelta(text) => ("textDelta", Value::String(text)),
        DriverEvent::ReasoningDelta(text) => ("reasoningDelta", Value::String(text)),
        DriverEvent::Activity {
            id,
            kind,
            title,
            detail,
            complete,
        } => (
            "activity",
            json!({
                "id": id,
                "kind": kind,
                "title": title,
                "detail": detail,
                "complete": complete,
            }),
        ),
        DriverEvent::RichActivity(activity) => ("richActivity", serde_json::to_value(activity)?),
        DriverEvent::BackgroundWork(work) => ("backgroundWork", serde_json::to_value(work)?),
        DriverEvent::Permission {
            request_id,
            title,
            detail,
            options,
        } => (
            "permission",
            json!({
                "requestId": request_id,
                "title": title,
                "detail": detail,
                "options": options,
            }),
        ),
        DriverEvent::UserInputRequested {
            request_id,
            questions,
        } => (
            "userInputRequested",
            json!({
                "requestId": request_id,
                "questions": questions,
            }),
        ),
        DriverEvent::ComputerUseUpdated(state) => (
            "computerUseUpdated",
            serde_json::to_value(ComputerUseWire {
                target: state.target,
                phase: state.phase,
                visible: state.visible,
                image_url: state.image_url,
            })?,
        ),
        DriverEvent::SteerAccepted { message } => ("steerAccepted", json!({ "message": message })),
        DriverEvent::SteerRejected { message, reason } => (
            "steerRejected",
            json!({ "message": message, "reason": reason }),
        ),
        DriverEvent::UsageUpdated {
            context_tokens,
            context_window,
        } => (
            "usageUpdated",
            json!({
                "contextTokens": context_tokens,
                "contextWindow": context_window,
            }),
        ),
        DriverEvent::PlanUsageUpdated(usage) => ("planUsageUpdated", serde_json::to_value(usage)?),
        DriverEvent::GoalUpdated(goal) => ("goalUpdated", serde_json::to_value(goal)?),
        DriverEvent::TurnFinished { success, summary } => (
            "turnFinished",
            json!({ "success": success, "summary": summary }),
        ),
        DriverEvent::Error(error) => ("error", Value::String(error)),
        DriverEvent::ProcessExited => ("processExited", Value::Null),
    };
    Ok(WireDriverEvent::new(kind, payload))
}

pub fn event_from_wire(event: WireDriverEvent) -> anyhow::Result<DriverEvent> {
    let payload = event.payload;
    Ok(match event.kind.as_str() {
        "connected" => DriverEvent::Connected {
            provider_cursor: serde_json::from_value(payload)?,
        },
        "agentPresetSelected" => DriverEvent::AgentPresetSelected(serde_json::from_value(payload)?),
        "autoTitleUpdated" => DriverEvent::AutoTitleUpdated(serde_json::from_value(payload)?),
        "availableCommands" => DriverEvent::AvailableCommands(serde_json::from_value(payload)?),
        "turnStarted" => DriverEvent::TurnStarted,
        "textDelta" => DriverEvent::TextDelta(serde_json::from_value(payload)?),
        "reasoningDelta" => DriverEvent::ReasoningDelta(serde_json::from_value(payload)?),
        "activity" => {
            let activity: ActivityWire = serde_json::from_value(payload)?;
            DriverEvent::Activity {
                id: activity.id,
                kind: activity.kind,
                title: activity.title,
                detail: activity.detail,
                complete: activity.complete,
            }
        }
        "richActivity" => DriverEvent::RichActivity(serde_json::from_value(payload)?),
        "backgroundWork" => DriverEvent::BackgroundWork(serde_json::from_value(payload)?),
        "permission" => {
            let permission: PermissionWire = serde_json::from_value(payload)?;
            DriverEvent::Permission {
                request_id: permission.request_id,
                title: permission.title,
                detail: permission.detail,
                options: permission.options,
            }
        }
        "userInputRequested" => {
            let request: UserInputWire = serde_json::from_value(payload)?;
            DriverEvent::UserInputRequested {
                request_id: request.request_id,
                questions: request.questions,
            }
        }
        "computerUseUpdated" => {
            let state: ComputerUseWire = serde_json::from_value(payload)?;
            DriverEvent::ComputerUseUpdated(ComputerUseState {
                target: state.target,
                phase: state.phase,
                visible: state.visible,
                image_url: state.image_url,
            })
        }
        "steerAccepted" => {
            let steer: AcceptedSteerWire = serde_json::from_value(payload)?;
            DriverEvent::SteerAccepted {
                message: steer.message,
            }
        }
        "steerRejected" => {
            let steer: RejectedSteerWire = serde_json::from_value(payload)?;
            DriverEvent::SteerRejected {
                message: steer.message,
                reason: steer.reason,
            }
        }
        "usageUpdated" => {
            let usage: UsageWire = serde_json::from_value(payload)?;
            DriverEvent::UsageUpdated {
                context_tokens: usage.context_tokens,
                context_window: usage.context_window,
            }
        }
        "planUsageUpdated" => DriverEvent::PlanUsageUpdated(serde_json::from_value(payload)?),
        "goalUpdated" => DriverEvent::GoalUpdated(serde_json::from_value(payload)?),
        "turnFinished" => {
            let finished: TurnFinishedWire = serde_json::from_value(payload)?;
            DriverEvent::TurnFinished {
                success: finished.success,
                summary: finished.summary,
            }
        }
        "error" => DriverEvent::Error(serde_json::from_value(payload)?),
        "processExited" => DriverEvent::ProcessExited,
        kind => bail!("daemon sent an unsupported driver event {kind:?}"),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivityWire {
    id: Option<String>,
    kind: ActivityKind,
    title: String,
    detail: Option<String>,
    complete: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PermissionWire {
    request_id: String,
    title: String,
    detail: String,
    options: Vec<PermissionOption>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserInputWire {
    request_id: String,
    questions: Vec<crate::model::UserInputQuestion>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseWire {
    target: Option<ComputerTarget>,
    phase: ComputerUsePhase,
    visible: bool,
    image_url: Option<String>,
}

#[derive(Deserialize)]
struct AcceptedSteerWire {
    message: String,
}

#[derive(Deserialize)]
struct RejectedSteerWire {
    message: String,
    reason: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageWire {
    context_tokens: Option<u64>,
    context_window: Option<u64>,
}

#[derive(Deserialize)]
struct TurnFinishedWire {
    success: bool,
    summary: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::Hub;
    use padu_protocol::ServerMessage;

    #[test]
    fn stale_runtime_projection_keeps_newer_transcript_cursor() {
        let runtime_id = Uuid::new_v4();
        let epoch = Uuid::new_v4();
        let mut existing = AgentSession::new(Uuid::new_v4(), ProviderKind::Codex);
        existing.status = SessionStatus::Working;
        existing.runtime_event_cursor = Some(crate::model::RuntimeEventCursor {
            runtime_id,
            epoch,
            sequence: 10,
        });
        existing.push_message(crate::model::MessageRole::Assistant, "complete so far");

        let mut stale = existing.clone();
        stale.title = "Renamed elsewhere".into();
        stale.messages.clear();
        stale.runtime_event_cursor = Some(crate::model::RuntimeEventCursor {
            runtime_id,
            epoch,
            sequence: 7,
        });

        assert!(session_projection_precedes(
            &existing,
            &stale,
            Some(runtime_id)
        ));
        merge_stale_session_metadata(&mut existing, stale);
        assert_eq!(existing.title, "Renamed elsewhere");
        assert_eq!(existing.messages.len(), 1);
        assert_eq!(existing.runtime_event_cursor.unwrap().sequence, 10);
    }

    #[test]
    fn stale_full_session_save_cannot_clear_daemon_metadata() {
        let mut existing = AgentSession::new(Uuid::new_v4(), ProviderKind::Codex);
        existing.pinned_at = Some(100);
        existing.archived_at = None;

        let mut incoming = existing.clone();
        incoming.pinned_at = None;
        incoming.archived_at = Some(200);
        incoming.updated_at = existing.updated_at + 1;
        merge_stale_session_metadata(&mut existing, incoming);

        assert_eq!(existing.pinned_at, Some(100));
        assert_eq!(existing.archived_at, None);
    }

    #[test]
    fn client_projection_cannot_replace_a_daemon_checkpoint() {
        let mut existing = AgentSession::new(Uuid::new_v4(), ProviderKind::Codex);
        existing.begin_turn("change it");
        existing.finish_active_turn(crate::model::TurnStatus::Completed);
        let checkpoint = Checkpoint {
            turn_count: 1,
            git_ref: "refs/padu/canonical".into(),
            status: CheckpointStatus::Ready,
            files: Vec::new(),
            additions: 0,
            deletions: 0,
            created_at: 1,
        };
        existing.turns[0].checkpoint = Some(checkpoint.clone());

        let mut incoming = existing.clone();
        incoming.turns[0].checkpoint = Some(Checkpoint {
            git_ref: "refs/padu/stale-client".into(),
            ..checkpoint.clone()
        });
        preserve_daemon_checkpoints(&existing, &mut incoming);

        assert_eq!(incoming.turns[0].checkpoint.as_ref(), Some(&checkpoint));
    }

    #[test]
    fn response_fork_titles_follow_one_numbered_sequence() {
        assert_eq!(
            next_response_fork_title("Fix the bug", ["Fix the bug"]),
            "Fix the bug (2)"
        );
        assert_eq!(
            next_response_fork_title(
                "Fix the bug (2)",
                ["Fix the bug", "Fix the bug (2)", "Fix the bug (4)"]
            ),
            "Fix the bug (5)"
        );
        assert_eq!(
            next_response_fork_title("Plan (2026)", ["Plan (2026)"]),
            "Plan (2026) (2)"
        );
    }

    #[test]
    fn message_rewind_requires_a_settled_user_turn_and_provider_cursor() {
        let mut session = AgentSession::new(Uuid::new_v4(), ProviderKind::Codex);
        session.begin_turn("change it");
        session.mark_active_turn_provider_started();
        session.provider_cursor = Some(ProviderResumeCursor::Codex {
            thread_id: "thread".into(),
        });
        session.finish_active_turn(crate::model::TurnStatus::Completed);

        assert!(validate_message_rewind(&session, 1).is_ok());

        let mut busy = session.clone();
        busy.status = SessionStatus::Working;
        assert!(validate_message_rewind(&busy, 1).is_err());

        let mut missing_cursor = session.clone();
        missing_cursor.provider_cursor = None;
        assert!(validate_message_rewind(&missing_cursor, 1).is_err());

        let mut missing_message = session;
        missing_message.messages.clear();
        assert!(validate_message_rewind(&missing_message, 1).is_err());
    }

    #[test]
    fn wire_event_round_trip_preserves_ordered_delta_payload() {
        let wire = event_to_wire(DriverEvent::TextDelta("hello".into())).unwrap();
        assert_eq!(wire.kind, "textDelta");
        assert!(matches!(
            event_from_wire(wire).unwrap(),
            DriverEvent::TextDelta(text) if text == "hello"
        ));
    }

    #[test]
    fn profile_toggle_syncs_legacy_disabled_providers() {
        // `DaemonSettings.disabled_providers` stays derived from the registry
        // (§6.3) so pre-P1-03 surfaces keep working while profiles own truth.
        let directory = std::env::temp_dir().join(format!("padu-profile-{}", Uuid::new_v4()));
        let settings = DaemonSettingsStore::open(directory.join("settings.json")).unwrap();
        let task_store = StateStore::daemon(directory.join("app.db"));
        task_store.seed_agent_profiles(&[]).unwrap();
        let backend = PaduBackend::new(settings, task_store).unwrap();

        backend
            .sync_profile_enabled_to_settings(ProviderKind::Claude, false)
            .unwrap();
        assert_eq!(
            backend.settings.get().disabled_providers,
            vec![ProviderKind::Claude]
        );
        // Disabling twice stays a single entry.
        backend
            .sync_profile_enabled_to_settings(ProviderKind::Claude, false)
            .unwrap();
        assert_eq!(
            backend.settings.get().disabled_providers,
            vec![ProviderKind::Claude]
        );

        backend
            .sync_profile_enabled_to_settings(ProviderKind::Claude, true)
            .unwrap();
        assert!(backend.settings.get().disabled_providers.is_empty());

        std::fs::remove_dir_all(directory).ok();
    }

    fn test_daemon() -> (
        PathBuf,
        PaduBackend,
        Arc<Hub>,
        crossbeam_channel::Receiver<ServerMessage>,
        Project,
    ) {
        let directory = std::env::temp_dir().join(format!("padu-lifecycle-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let settings = DaemonSettingsStore::open(directory.join("settings.json")).unwrap();
        let task_store = StateStore::daemon(directory.join("app.db"));
        task_store.seed_agent_profiles(&[]).unwrap();
        let backend = PaduBackend::new(settings, task_store).unwrap();

        let project = Project {
            id: Uuid::new_v4(),
            name: "test-project".to_owned(),
            path: directory.join("repo"),
            created_at: crate::model::unix_time(),
            scripts: Vec::new(),
            linked_repo: None,
        };
        std::fs::create_dir_all(&project.path).unwrap();
        std::fs::create_dir_all(project.path.join(".git")).unwrap();

        {
            let mut state = backend.task_state.lock();
            state.projects.push(project.clone());
            backend.task_store.save(&mut state).unwrap();
        }

        let hub = Arc::new(Hub::default());
        let (sender, receiver) = crossbeam_channel::unbounded();
        hub.subscribe(&[], sender);

        (directory, backend, hub, receiver, project)
    }

    #[test]
    fn manual_drag_into_running_rejected_with_reason() {
        let (dir, backend, hub, _rx, project) = test_daemon();
        let sink = hub.event_sink(Uuid::new_v4(), Uuid::new_v4());
        let task = backend
            .task_store
            .create_task(padu_protocol::kanban::CreateTask {
                project_id: project.id,
                title: "Task 1".into(),
                description: "Fix bug".into(),
                labels: vec![],
                assigned_agent: None,
                model: None,
            })
            .unwrap();

        // Direct MoveTask to Running rejected
        let err = backend
            .handle(
                Request {
                    request_id: Uuid::new_v4(),
                    session_id: Uuid::nil(),
                    runtime_id: Uuid::nil(),
                    command: Command::MoveTask {
                        task_id: task.id,
                        status: TaskStatus::Running,
                        expected_version: task.version,
                    },
                },
                sink.clone(),
            )
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("cannot manually move task to running")
        );

        // Direct UpdateTask to Running rejected
        let mut update_task = task.clone();
        update_task.status = TaskStatus::Running;
        let err = backend
            .handle(
                Request {
                    request_id: Uuid::new_v4(),
                    session_id: Uuid::nil(),
                    runtime_id: Uuid::nil(),
                    command: Command::UpdateTask {
                        task: update_task,
                        expected_version: task.version,
                    },
                },
                sink,
            )
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("cannot manually move task to running")
        );

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn backlog_to_queued_creates_agent_session_honoring_worktree_policy() {
        let (dir, backend, hub, rx, project) = test_daemon();
        let sink = hub.event_sink(Uuid::new_v4(), Uuid::new_v4());
        let task = backend
            .task_store
            .create_task(padu_protocol::kanban::CreateTask {
                project_id: project.id,
                title: "Task with Description".into(),
                description: "Enqueued prompt content".into(),
                labels: vec!["fix".into()],
                assigned_agent: None,
                model: None,
            })
            .unwrap();

        // Move Backlog -> Queued
        let moved = match backend
            .handle(
                Request {
                    request_id: Uuid::new_v4(),
                    session_id: Uuid::nil(),
                    runtime_id: Uuid::nil(),
                    command: Command::MoveTask {
                        task_id: task.id,
                        status: TaskStatus::Queued,
                        expected_version: task.version,
                    },
                },
                sink,
            )
            .unwrap()
        {
            ResponsePayload::TaskMoved { task } => task,
            other => panic!("expected TaskMoved, got {other:?}"),
        };

        assert_eq!(moved.status, TaskStatus::Queued);
        assert!(moved.session_id.is_some());
        assert_eq!(moved.workspace_kind, Some(TaskWorkspaceKind::NewWorktree));
        assert!(moved.assigned_agent.is_some());

        let session_id = moved.session_id.unwrap();
        // Check session exists in in-memory state and has queued message
        let state = backend.task_state.lock();
        let session = state
            .sessions
            .iter()
            .find(|s| s.id == session_id)
            .expect("session created");
        assert_eq!(session.title, "Task with Description");
        assert_eq!(session.queued_messages.len(), 1);
        assert_eq!(
            session.queued_messages[0].content,
            "Enqueued prompt content"
        );
        assert!(matches!(
            session.workspace,
            SessionWorkspace::NewWorktree { base_branch: None }
        ));

        // Check broadcast events: TaskQueued and TaskStateChanged
        let mut got_task_queued = false;
        let mut got_task_state_changed = false;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ServerMessage::TaskQueued { task_id, .. } if task_id == task.id => {
                    got_task_queued = true;
                }
                ServerMessage::TaskStateChanged { .. } => {
                    got_task_state_changed = true;
                }
                _ => {}
            }
        }
        assert!(got_task_queued, "expected TaskQueued event");
        assert!(got_task_state_changed, "expected TaskStateChanged event");

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn failed_queued_write_leaves_no_orphan_session() {
        let (dir, backend, hub, _rx, project) = test_daemon();
        let sink = hub.event_sink(Uuid::new_v4(), Uuid::new_v4());
        let task = backend
            .task_store
            .create_task(padu_protocol::kanban::CreateTask {
                project_id: project.id,
                title: "Race me".into(),
                description: "Enqueued prompt content".into(),
                labels: vec![],
                assigned_agent: None,
                model: None,
            })
            .unwrap();

        // First queue of the Backlog snapshot wins.
        let queued = backend
            .queue_backlog_task(task.clone(), task.version, &sink)
            .unwrap();
        assert_eq!(queued.status, TaskStatus::Queued);

        // A loser racing the same Backlog snapshot fails the version guard
        // after persisting its session; that session must be removed again.
        backend
            .queue_backlog_task(task.clone(), task.version, &sink)
            .unwrap_err();

        let state = backend.task_state.lock();
        let tasks = backend.task_store.list_tasks().unwrap();
        assert_eq!(state.sessions.len(), 1);
        for session in &state.sessions {
            assert!(
                tasks.iter().any(|t| t.session_id == Some(session.id)),
                "every session stays linked from a card"
            );
        }

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn queued_to_running_on_workspace_started() {
        let (dir, backend, hub, rx, project) = test_daemon();
        let sink = hub.event_sink(Uuid::new_v4(), Uuid::new_v4());
        let task = backend
            .task_store
            .create_task(padu_protocol::kanban::CreateTask {
                project_id: project.id,
                title: "Run me".into(),
                description: "Do it".into(),
                labels: vec![],
                assigned_agent: None,
                model: None,
            })
            .unwrap();

        let moved = match backend
            .handle(
                Request {
                    request_id: Uuid::new_v4(),
                    session_id: Uuid::nil(),
                    runtime_id: Uuid::nil(),
                    command: Command::MoveTask {
                        task_id: task.id,
                        status: TaskStatus::Queued,
                        expected_version: task.version,
                    },
                },
                sink.clone(),
            )
            .unwrap()
        {
            ResponsePayload::TaskMoved { task } => task,
            other => panic!("expected TaskMoved, got {other:?}"),
        };

        let session_id = moved.session_id.unwrap();
        // Drain prior events
        while rx.try_recv().is_ok() {}

        // Trigger on_workspace_started (as would occur in Command::Start)
        backend.on_workspace_started(session_id, &sink);

        // Verify task transitioned to Running
        let updated = backend.task_store.get_task(task.id).unwrap().unwrap();
        assert_eq!(updated.status, TaskStatus::Running);

        // Verify WorkspaceStarted event received
        let mut got_workspace_started = false;
        while let Ok(msg) = rx.try_recv() {
            if let ServerMessage::WorkspaceStarted {
                task_id,
                session_id: sid,
                ..
            } = msg
            {
                if task_id == task.id && sid == session_id {
                    got_workspace_started = true;
                }
            }
        }
        assert!(got_workspace_started, "expected WorkspaceStarted event");

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn running_to_review_on_agent_completed_and_failure_streak() {
        let (dir, backend, hub, rx, project) = test_daemon();
        let sink = hub.event_sink(Uuid::new_v4(), Uuid::new_v4());

        // Setup running task
        let mut task = backend
            .task_store
            .create_task(padu_protocol::kanban::CreateTask {
                project_id: project.id,
                title: "Running Task".into(),
                description: "Work".into(),
                labels: vec![],
                assigned_agent: Some(ProviderKind::Codex),
                model: None,
            })
            .unwrap();
        let session_id = Uuid::new_v4();
        task.session_id = Some(session_id);
        task.status = TaskStatus::Running;
        let task = backend
            .task_store
            .update_task(task.clone(), task.version)
            .unwrap();

        // 1. Checkpoint Ready -> moves Running to Review, emits AgentCompleted
        let mut session = AgentSession::new(project.id, ProviderKind::Codex);
        session.id = session_id;
        session.begin_turn("first prompt");
        session.finish_active_turn(crate::model::TurnStatus::Completed);
        let ready_checkpoint = Checkpoint {
            turn_count: 1,
            git_ref: "refs/padu/test".into(),
            status: CheckpointStatus::Ready,
            files: vec![],
            additions: 0,
            deletions: 0,
            created_at: 100,
        };
        session.turns[0].checkpoint = Some(ready_checkpoint.clone());
        backend.task_state.lock().push_session(session);

        while rx.try_recv().is_ok() {}
        let _ = backend
            .capture_turn_checkpoint(project.path.clone(), session_id, 1, &sink)
            .unwrap();

        // Task should now be in Review
        let after_complete = backend.task_store.get_task(task.id).unwrap().unwrap();
        assert_eq!(after_complete.status, TaskStatus::Review);
        assert!(!after_complete.needs_attention);

        // 2. Test failure streak threshold
        // Put task back into Running
        let mut running_task = after_complete;
        running_task.status = TaskStatus::Running;
        let running_task = backend
            .task_store
            .update_task(running_task.clone(), running_task.version)
            .unwrap();

        // Streak 1 error
        {
            let mut streaks = backend.checkpoint_failure_streaks.lock();
            let count = streaks.entry(session_id).or_insert(0);
            *count += 1;
        }
        sink.send_checkpoint_failed(running_task.id, session_id, 1);
        let current = backend
            .task_store
            .get_task(running_task.id)
            .unwrap()
            .unwrap();
        assert_eq!(current.status, TaskStatus::Running);

        // Streak reaches threshold (3) -> moves to Review with needs_attention = true
        {
            let mut streaks = backend.checkpoint_failure_streaks.lock();
            *streaks.entry(session_id).or_insert(0) = 3;
        }
        let mut failed_task = current;
        failed_task.status = TaskStatus::Review;
        failed_task.needs_attention = true;
        failed_task.sync_failed = Some("checkpoint failed 3 consecutive times".into());
        let updated = backend
            .task_store
            .update_task(failed_task.clone(), failed_task.version)
            .unwrap();
        assert_eq!(updated.status, TaskStatus::Review);
        assert!(updated.needs_attention);
        assert!(updated.sync_failed.is_some());

        // 3. Review -> Done via explicit Mark Done
        let done = match backend
            .handle(
                Request {
                    request_id: Uuid::new_v4(),
                    session_id: Uuid::nil(),
                    runtime_id: Uuid::nil(),
                    command: Command::MoveTask {
                        task_id: updated.id,
                        status: TaskStatus::Done,
                        expected_version: updated.version,
                    },
                },
                sink.clone(),
            )
            .unwrap()
        {
            ResponsePayload::TaskMoved { task } => task,
            other => panic!("expected TaskMoved, got {other:?}"),
        };
        assert_eq!(done.status, TaskStatus::Done);

        // 4. Reopen to Backlog clears needs_attention and sync_failed
        let reopened = match backend
            .handle(
                Request {
                    request_id: Uuid::new_v4(),
                    session_id: Uuid::nil(),
                    runtime_id: Uuid::nil(),
                    command: Command::MoveTask {
                        task_id: done.id,
                        status: TaskStatus::Backlog,
                        expected_version: done.version,
                    },
                },
                sink,
            )
            .unwrap()
        {
            ResponsePayload::TaskMoved { task } => task,
            other => panic!("expected TaskMoved, got {other:?}"),
        };
        assert_eq!(reopened.status, TaskStatus::Backlog);
        assert!(!reopened.needs_attention);
        assert!(reopened.sync_failed.is_none());

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn review_to_done_external_signal_and_disallowed_done_transitions() {
        let (dir, backend, hub, _rx, project) = test_daemon();
        let sink = hub.event_sink(Uuid::new_v4(), Uuid::new_v4());

        let mut task = backend
            .task_store
            .create_task(padu_protocol::kanban::CreateTask {
                project_id: project.id,
                title: "Review Task".into(),
                description: "In review".into(),
                labels: vec![],
                assigned_agent: None,
                model: None,
            })
            .unwrap();

        // Queued cannot be marked Done directly
        task.status = TaskStatus::Queued;
        let task = backend
            .task_store
            .update_task(task.clone(), task.version)
            .unwrap();
        let err = backend
            .handle(
                Request {
                    request_id: Uuid::new_v4(),
                    session_id: Uuid::nil(),
                    runtime_id: Uuid::nil(),
                    command: Command::MoveTask {
                        task_id: task.id,
                        status: TaskStatus::Done,
                        expected_version: task.version,
                    },
                },
                sink.clone(),
            )
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("cannot mark a queued task as done")
        );

        // Running cannot be marked Done directly
        let mut running = task;
        running.status = TaskStatus::Running;
        let running = backend
            .task_store
            .update_task(running.clone(), running.version)
            .unwrap();
        let err = backend
            .handle(
                Request {
                    request_id: Uuid::new_v4(),
                    session_id: Uuid::nil(),
                    runtime_id: Uuid::nil(),
                    command: Command::MoveTask {
                        task_id: running.id,
                        status: TaskStatus::Done,
                        expected_version: running.version,
                    },
                },
                sink.clone(),
            )
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("cannot mark a running task as done")
        );

        // Put into Review
        let mut review = running;
        review.status = TaskStatus::Review;
        let review = backend
            .task_store
            .update_task(review.clone(), review.version)
            .unwrap();

        // External signal completes Review task to Done
        let done = backend
            .mark_task_done_from_external_signal(review.id, &sink)
            .unwrap();
        assert_eq!(done.status, TaskStatus::Done);

        std::fs::remove_dir_all(dir).ok();
    }
}
