use super::*;

const COMPOSER_DRAFT_SAVE_DELAY: Duration = Duration::from_millis(250);

impl From<&ComposerAttachment> for crate::persistence::ComposerDraftAttachment {
    fn from(attachment: &ComposerAttachment) -> Self {
        Self {
            path: attachment.path.clone(),
            mention: attachment.mention.clone(),
            name: attachment.name.to_string(),
            is_dir: attachment.is_dir,
            is_image: attachment.is_image,
            blob_reference: attachment.blob_reference.clone(),
        }
    }
}

impl From<crate::persistence::ComposerDraftAttachment> for ComposerAttachment {
    fn from(attachment: crate::persistence::ComposerDraftAttachment) -> Self {
        Self {
            path: attachment.path,
            client_preview_image: None,
            mention: attachment.mention,
            name: SharedString::from(attachment.name),
            is_dir: attachment.is_dir,
            is_image: attachment.is_image,
            blob_reference: attachment.blob_reference,
        }
    }
}

impl From<ComposerAttachment> for MessageAttachment {
    fn from(attachment: ComposerAttachment) -> Self {
        Self {
            path: attachment.path,
            mention: attachment.mention,
            name: attachment.name.to_string(),
            is_dir: attachment.is_dir,
            is_image: attachment.is_image,
            blob_reference: attachment.blob_reference,
        }
    }
}

impl From<MessageAttachment> for ComposerAttachment {
    fn from(attachment: MessageAttachment) -> Self {
        Self {
            path: attachment.path,
            client_preview_image: None,
            mention: attachment.mention,
            name: SharedString::from(attachment.name),
            is_dir: attachment.is_dir,
            is_image: attachment.is_image,
            blob_reference: attachment.blob_reference,
        }
    }
}

fn recovered_composer_draft(submission: &ComposerSubmission) -> crate::persistence::ComposerDraft {
    crate::persistence::ComposerDraft {
        text: submission
            .display_content
            .clone()
            .unwrap_or_else(|| submission.prompt.clone()),
        attachments: submission
            .attachments
            .iter()
            .map(|attachment| crate::persistence::ComposerDraftAttachment {
                path: attachment.path.clone(),
                mention: attachment.mention.clone(),
                name: attachment.name.clone(),
                is_dir: attachment.is_dir,
                is_image: attachment.is_image,
                blob_reference: attachment.blob_reference.clone(),
            })
            .collect(),
        embedded_notes: submission.embedded_notes.clone(),
    }
}

pub(super) fn merge_recovered_text(current: String, recovered: String) -> String {
    if recovered.is_empty()
        || current == recovered
        || current.ends_with(&format!("\n\n{recovered}"))
    {
        return current;
    }
    if current.is_empty() {
        return recovered;
    }
    format!("{current}\n\n{recovered}")
}

fn merge_recovered_composer_draft(
    mut current: crate::persistence::ComposerDraft,
    recovered: crate::persistence::ComposerDraft,
) -> crate::persistence::ComposerDraft {
    current.text = merge_recovered_text(current.text, recovered.text);
    for attachment in recovered.attachments {
        if !current.attachments.iter().any(|existing| {
            existing.path == attachment.path && existing.blob_reference == attachment.blob_reference
        }) {
            current.attachments.push(attachment);
        }
    }
    for note in recovered.embedded_notes {
        if !current.embedded_notes.contains(&note) {
            current.embedded_notes.push(note);
        }
    }
    current
}

fn queue_pending_composer_recovery(
    recoveries: &mut Vec<PendingComposerRecovery>,
    recovery: PendingComposerRecovery,
) {
    // Two rejected prompts can legitimately belong to the same captured task.
    // Keep both in arrival order; de-duplicating by session would silently
    // discard the first one.
    recoveries.push(recovery);
}

impl Padu {
    pub(super) fn selected_composer_draft_key(
        &self,
    ) -> Option<crate::persistence::ComposerDraftKey> {
        self.state
            .selected_session
            .and_then(|selected| {
                self.state
                    .sessions
                    .iter()
                    .find(|session| session.id == selected)
            })
            .map(crate::persistence::ComposerDraftKey::for_session)
    }

    fn current_composer_draft(&self, cx: &App) -> crate::persistence::ComposerDraft {
        crate::persistence::ComposerDraft {
            text: self.composer.read(cx).content(cx).to_owned(),
            attachments: self
                .composer_attachments
                .iter()
                .map(crate::persistence::ComposerDraftAttachment::from)
                .collect(),
            embedded_notes: self.composer_embedded_notes.clone(),
        }
    }

    /// Return an accepted submission to the draft that owned it. This is used
    /// when a background skill refresh is invalidated before dispatch. Merge
    /// into any newer text instead of replacing it, and keep the captured
    /// attachments and embedded notes with the recovered prompt.
    pub(super) fn recover_composer_submission(
        &mut self,
        captured_session_id: Uuid,
        captured_project_id: Uuid,
        submission: ComposerSubmission,
        cx: &mut Context<Self>,
    ) {
        let fallback = self.selected_composer_draft_key().or_else(|| {
            self.state
                .selected_project
                .filter(|project_id| {
                    self.state
                        .projects
                        .iter()
                        .any(|project| project.id == *project_id)
                })
                .map(crate::persistence::ComposerDraftKey::NewSession)
        });
        let key = super::runtime::composer_draft_key_for_recovery(
            captured_session_id,
            captured_project_id,
            &self.state.sessions,
            &self.state.projects,
            fallback,
            self.host_switch_pending,
        );
        let Some(key) = key else {
            queue_pending_composer_recovery(
                &mut self.pending_composer_recoveries,
                PendingComposerRecovery {
                    captured_session_id,
                    captured_project_id,
                    submission,
                },
            );
            self.ensure_composer_recovery_target(cx);
            cx.notify();
            return;
        };
        let selected_key = self.selected_composer_draft_key();
        let current = if selected_key == Some(key) {
            self.current_composer_draft(cx)
        } else {
            self.composer_drafts.get(key).cloned().unwrap_or_default()
        };
        let recovered =
            merge_recovered_composer_draft(current, recovered_composer_draft(&submission));
        let changed = self.composer_drafts.set(key, recovered);
        if selected_key == Some(key) {
            self.restore_selected_composer_draft(cx);
        }
        if changed {
            self.schedule_composer_draft_save(cx);
        }
        cx.notify();
    }

    pub(super) fn flush_pending_composer_recoveries(&mut self, cx: &mut Context<Self>) {
        if self.pending_composer_recoveries.is_empty() {
            return;
        }
        if self.selected_composer_draft_key().is_none() {
            self.ensure_composer_recovery_target(cx);
            return;
        }
        let pending = std::mem::take(&mut self.pending_composer_recoveries);
        for recovery in pending {
            self.recover_composer_submission(
                recovery.captured_session_id,
                recovery.captured_project_id,
                recovery.submission,
                cx,
            );
        }
    }

    fn ensure_composer_recovery_target(&mut self, cx: &mut Context<Self>) {
        if self.pending_composer_recoveries.is_empty()
            || self.selected_composer_draft_key().is_some()
            || self.composer_recovery_target_pending
        {
            return;
        }
        if let Some(project_id) = self.state.projects.first().map(|project| project.id) {
            self.state.selected_project = Some(project_id);
            self.create_session_for(project_id, self.state.last_provider, cx);
            return;
        }
        self.composer_recovery_target_pending = true;
        self.create_projectless_session(cx);
    }

    /// Copy the visible composer into its in-memory slot. No I/O happens here;
    /// callers can use this on every real edit and on navigation boundaries.
    pub(super) fn capture_current_composer_draft(&mut self, cx: &App) -> bool {
        let Some(key) = self.selected_composer_draft_key() else {
            return false;
        };
        let draft = self.current_composer_draft(cx);
        self.composer_drafts.set(key, draft)
    }

    pub(super) fn capture_and_save_current_composer_draft(&mut self, cx: &mut Context<Self>) {
        if self.capture_current_composer_draft(cx) {
            self.schedule_composer_draft_save(cx);
        }
    }

    /// A project choice in the composer changes where the current unsent task
    /// will run; it is not ordinary task navigation. Carry its draft into a
    /// blank destination instead of letting session activation clear it.
    fn move_composer_draft_after_project_change(
        &mut self,
        source: Option<crate::persistence::ComposerDraftKey>,
        cx: &mut Context<Self>,
    ) {
        let Some(source) = source else {
            return;
        };
        let Some(destination) = self.selected_composer_draft_key() else {
            return;
        };
        if self.composer_drafts.move_to_empty(source, destination) {
            self.restore_selected_composer_draft(cx);
            self.schedule_composer_draft_save(cx);
        }
    }

    pub(super) fn select_project_from_composer(
        &mut self,
        project_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        let source = self.selected_composer_draft_key();
        self.select_project(project_id, cx);
        self.move_composer_draft_after_project_change(source, cx);
    }

    pub(super) fn create_projectless_session_from_composer(&mut self, cx: &mut Context<Self>) {
        let source = self.selected_composer_draft_key();
        self.create_projectless_session(cx);
        self.move_composer_draft_after_project_change(source, cx);
    }

    /// Submission consumes the active draft before a blank session gains a
    /// durable session identity, so its project-scoped text cannot reappear
    /// the next time the user opens New Task.
    pub(super) fn discard_current_composer_draft(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.selected_composer_draft_key() else {
            return;
        };
        if self.composer_drafts.remove(key) {
            self.schedule_composer_draft_save(cx);
        }
    }

    pub(super) fn remove_composer_draft(
        &mut self,
        key: crate::persistence::ComposerDraftKey,
        cx: &mut Context<Self>,
    ) {
        if self.composer_drafts.remove(key) {
            self.schedule_composer_draft_save(cx);
        }
    }

    /// Replace the global input entity with the newly selected task's draft.
    /// The lookup is entirely in memory; attachments carry their cached file
    /// metadata so a session switch never stats their paths.
    pub(super) fn restore_selected_composer_draft(&mut self, cx: &mut Context<Self>) {
        let draft = self
            .selected_composer_draft_key()
            .and_then(|key| self.composer_drafts.get(key))
            .cloned()
            .unwrap_or_default();
        self.composer_attachments = draft
            .attachments
            .into_iter()
            .map(ComposerAttachment::from)
            .collect();
        self.composer_embedded_notes = draft.embedded_notes;
        self.composer
            .update(cx, |input, cx| input.set_content(draft.text, cx));
        cx.notify();
    }

    /// Debounce disk traffic while keeping serialization and filesystem I/O on
    /// the background executor. Generation checks also make detached timers
    /// and out-of-order writes harmless.
    pub(super) fn schedule_composer_draft_save(&mut self, cx: &mut Context<Self>) {
        if self.host_switch_pending {
            return;
        }
        self.composer_draft_save_generation = self.composer_draft_save_generation.saturating_add(1);
        let generation = self.composer_draft_save_generation;
        let store = self.composer_draft_store.clone();
        let host_generation = self.host_switch_generation;
        cx.spawn(async move |padu, cx| {
            cx.background_executor()
                .timer(COMPOSER_DRAFT_SAVE_DELAY)
                .await;
            let Some(drafts) = padu
                .update(cx, |padu, cx| {
                    if padu.composer_draft_save_generation != generation
                        || padu.host_switch_generation != host_generation
                    {
                        return None;
                    }
                    padu.capture_current_composer_draft(cx);
                    Some(padu.composer_drafts.clone())
                })
                .ok()
                .flatten()
            else {
                return;
            };
            let result = cx
                .background_executor()
                .spawn(async move { store.save(drafts, generation) })
                .await;
            if let Err(error) = result {
                let _ = padu.update(cx, |padu, cx| {
                    padu.show_toast(tr!("errors.save_local_state", error = error));
                    cx.notify();
                });
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    #[test]
    fn invalidated_submission_merges_without_losing_newer_text_or_metadata() {
        let attachment = |path: &str| MessageAttachment {
            path: PathBuf::from(path),
            mention: path.into(),
            name: path.into(),
            is_dir: false,
            is_image: false,
            blob_reference: Some(format!("{path}-reference")),
        };
        let note = |title: &str| padu_protocol::notes::EmbeddedNote {
            id: Uuid::new_v4(),
            title: title.into(),
            content: format!("{title} content"),
            revision: 1,
        };
        let current_attachment = attachment("/current.txt");
        let accepted_attachment = attachment("/accepted.txt");
        let current_note = note("Current");
        let accepted_note = note("Accepted");
        let current = crate::persistence::ComposerDraft {
            text: "newer composer text".into(),
            attachments: vec![crate::persistence::ComposerDraftAttachment {
                path: current_attachment.path.clone(),
                mention: current_attachment.mention.clone(),
                name: current_attachment.name.clone(),
                is_dir: current_attachment.is_dir,
                is_image: current_attachment.is_image,
                blob_reference: current_attachment.blob_reference.clone(),
            }],
            embedded_notes: vec![current_note.clone()],
        };
        let submission = ComposerSubmission {
            prompt: "[mandatory! open skill: /remote/SKILL.md] accepted".into(),
            display_content: Some("$brainstroming accepted".into()),
            attachments: vec![accepted_attachment.clone()],
            embedded_notes: vec![accepted_note.clone()],
            needs_skill_refresh: false,
            skill_warnings: Vec::new(),
        };

        let recovered = recovered_composer_draft(&submission);
        assert_eq!(
            recovered.text, "$brainstroming accepted",
            "recovery stores the visible prompt, not the provider payload"
        );
        assert_eq!(recovered.attachments.len(), 1);
        assert_eq!(recovered.embedded_notes, vec![accepted_note.clone()]);

        let merged = merge_recovered_composer_draft(current, recovered.clone());
        assert_eq!(
            merged.text,
            "newer composer text\n\n$brainstroming accepted"
        );
        assert_eq!(merged.attachments.len(), 2);
        assert_eq!(
            merged.attachments[0].path, current_attachment.path,
            "the newer draft keeps its original attachment order"
        );
        assert_eq!(merged.attachments[1].path, accepted_attachment.path);
        assert_eq!(
            merged.embedded_notes,
            vec![current_note, accepted_note],
            "both the newer note and accepted submission note survive"
        );

        let merged_again = merge_recovered_composer_draft(merged.clone(), recovered);
        assert_eq!(
            merged_again, merged,
            "replaying recovery must not duplicate text or metadata"
        );
    }

    #[test]
    fn repeated_recovery_for_one_task_preserves_both_prompts_in_order() {
        let session_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let mut recoveries = Vec::new();
        for prompt in ["first rejected prompt", "second rejected prompt"] {
            queue_pending_composer_recovery(
                &mut recoveries,
                PendingComposerRecovery {
                    captured_session_id: session_id,
                    captured_project_id: project_id,
                    submission: ComposerSubmission::plain(prompt.into()),
                },
            );
        }

        assert_eq!(recoveries.len(), 2);
        assert_eq!(recoveries[0].submission.prompt, "first rejected prompt");
        assert_eq!(recoveries[1].submission.prompt, "second rejected prompt");
    }
}
