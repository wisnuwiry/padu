use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NotesLayout {
    Edit,
    Split,
    Preview,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Note {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub body: String,
    pub revision: u64,
    pub created_at: u64,
    pub updated_at: u64,
    /// Precomputed lowercase search key so per-frame filtering never
    /// allocates a copy of every note body.
    pub search_key: String,
}

impl Note {
    fn search_key(title: &str, body: &str) -> String {
        let mut key = title.to_lowercase();
        key.push('\0');
        key.push_str(&body.to_lowercase());
        key
    }

    fn from_protocol(note: padu_client::notes::Note) -> Self {
        let search_key = Self::search_key(&note.title, &note.content);
        Self {
            id: note.id,
            project_id: note.project_id,
            title: note.title,
            body: note.content,
            revision: note.revision,
            created_at: note.created_at,
            updated_at: note.updated_at,
            search_key,
        }
    }

    fn refresh_search_key(&mut self) {
        self.search_key = Self::search_key(&self.title, &self.body);
    }
}

impl Padu {
    pub(super) fn open_notes(&mut self, cx: &mut Context<Self>) {
        self.navigate_workspace_page(WorkspacePage::Notes, cx);
        self.ensure_notes_loaded(cx);
    }

    pub(super) fn ensure_notes_loaded(&mut self, cx: &mut Context<Self>) {
        if self.notes_loaded || self.notes_load_pending {
            return;
        }
        self.load_notes_from_daemon(Uuid::nil(), cx);
    }

    pub(super) fn load_notes_from_daemon(&mut self, project_id: Uuid, cx: &mut Context<Self>) {
        if self.notes_load_pending {
            return;
        }
        self.notes_load_pending = true;
        self.notes_load_generation = self.notes_load_generation.wrapping_add(1);
        let generation = self.notes_load_generation;
        let daemon = self.daemon.clone();
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let response = daemon.client().request(
                        Uuid::nil(),
                        Uuid::nil(),
                        padu_client::Command::ListNotes { project_id },
                    )?;
                    let padu_client::ResponsePayload::Notes { notes } = response else {
                        anyhow::bail!("daemon returned an invalid Notes response");
                    };
                    let mut loaded = Vec::with_capacity(notes.len());
                    for summary in notes {
                        let response = daemon.client().request(
                            Uuid::nil(),
                            Uuid::nil(),
                            padu_client::Command::GetNote {
                                project_id: summary.project_id,
                                note_id: summary.id,
                            },
                        )?;
                        let padu_client::ResponsePayload::Note { note: Some(note) } = response
                        else {
                            continue;
                        };
                        loaded.push(note);
                    }
                    Ok::<_, anyhow::Error>(loaded)
                })
                .await;
            let _ = padu.update(cx, |this, cx| {
                if this.notes_load_generation != generation {
                    return;
                }
                this.notes_load_pending = false;
                if let Ok(notes) = result {
                    this.notes_loaded = true;
                    this.notes = notes.into_iter().map(Note::from_protocol).collect();
                    this.notes_data_generation = this.notes_data_generation.wrapping_add(1);
                    this.notes_selected =
                        this.notes_selected.min(this.notes.len().saturating_sub(1));
                    this.sync_note_editors(cx);
                    if this.command_palette.open {
                        let query = this.command_palette.search.read(cx).content().to_owned();
                        this.refresh_command_palette_results(&query, true, cx);
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn sync_note_editors(&self, cx: &mut Context<Self>) {
        let Some(note) = self.notes.get(self.notes_selected) else {
            return;
        };
        self.notes_title.update(cx, |input, cx| {
            if input.content() != note.title {
                input.set_content(note.title.clone(), cx);
            }
        });
        self.notes_body.update(cx, |input, cx| {
            if input.content() != note.body {
                input.set_content(note.body.clone(), cx);
            }
        });
    }

    fn select_note(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.notes.len() {
            return;
        }
        self.save_note_edit(cx);
        self.notes_selected = index;
        self.notes_layout = NotesLayout::Edit;
        self.sync_note_editors(cx);
        cx.notify();
    }

    fn create_note(&mut self, cx: &mut Context<Self>) {
        self.save_note_edit(cx);
        let Some(project_id) = self.active_project().map(|project| project.id) else {
            return;
        };
        let daemon = self.daemon.clone();
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let response = daemon.client().request(
                        Uuid::nil(),
                        Uuid::nil(),
                        padu_client::Command::CreateNote {
                            note: padu_client::notes::CreateNote {
                                project_id,
                                title: tr!("notes.untitled"),
                                content: String::new(),
                            },
                        },
                    )?;
                    let padu_client::ResponsePayload::NoteCreated { note } = response else {
                        anyhow::bail!("daemon returned an invalid Notes create response");
                    };
                    Ok::<_, anyhow::Error>(note)
                })
                .await;
            let _ = padu.update(cx, |this, cx| {
                if let Ok(note) = result {
                    this.notes.push(Note::from_protocol(note));
                    this.notes_data_generation = this.notes_data_generation.wrapping_add(1);
                    this.notes_selected = this.notes.len() - 1;
                    this.sync_note_editors(cx);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn schedule_note_save(&mut self, cx: &mut Context<Self>) {
        let Some(note) = self.notes.get(self.notes_selected) else {
            return;
        };
        let title = self.notes_title.read(cx).content().trim().to_owned();
        let body = self.notes_body.read(cx).content().to_owned();
        let normalized_title = if title.is_empty() {
            tr!("notes.untitled")
        } else {
            title
        };
        if normalized_title == note.title && body == note.body {
            return;
        }
        cx.notify();
        self.notes_save_generation = self.notes_save_generation.wrapping_add(1);
        let generation = self.notes_save_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(650))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.notes_save_generation == generation {
                    this.save_note_edit(cx);
                }
            });
        })
        .detach();
    }

    fn save_note_edit(&mut self, cx: &mut Context<Self>) {
        self.notes_save_generation = self.notes_save_generation.wrapping_add(1);
        let Some(note) = self.notes.get_mut(self.notes_selected) else {
            return;
        };
        let title = self.notes_title.read(cx).content().trim().to_owned();
        let body = self.notes_body.read(cx).content().to_owned();
        let normalized_title = if title.is_empty() {
            tr!("notes.untitled")
        } else {
            title
        };
        if normalized_title == note.title && body == note.body {
            return;
        }
        note.title = normalized_title;
        note.body = body;
        note.refresh_search_key();
        note.updated_at = unix_time();
        self.notes_data_generation = self.notes_data_generation.wrapping_add(1);
        let note_snapshot = note.clone();
        self.persist_note_snapshot(note_snapshot, cx);
    }

    /// Sends an update for a note snapshot. On a revision conflict the pending
    /// edit is reconciled against the daemon and retried instead of being
    /// silently dropped.
    fn persist_note_snapshot(&mut self, note: Note, cx: &mut Context<Self>) {
        let note_snapshot = note;
        let retry_snapshot = note_snapshot.clone();
        let daemon = self.daemon.clone();
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let response = daemon.client().request(
                        Uuid::nil(),
                        Uuid::nil(),
                        padu_client::Command::UpdateNote {
                            note: padu_client::notes::UpdateNote {
                                project_id: note_snapshot.project_id,
                                note_id: note_snapshot.id,
                                title: note_snapshot.title,
                                content: note_snapshot.body,
                                expected_revision: note_snapshot.revision,
                            },
                        },
                    )?;
                    let padu_client::ResponsePayload::NoteUpdated { note } = response else {
                        anyhow::bail!("daemon returned an invalid Notes update response");
                    };
                    Ok::<_, anyhow::Error>(note)
                })
                .await;
            let _ = padu.update(cx, |this, cx| match result {
                Ok(note) => {
                    if let Some(current) =
                        this.notes.iter_mut().find(|current| current.id == note.id)
                    {
                        current.revision = note.revision;
                        current.updated_at = note.updated_at;
                    }
                    cx.notify();
                }
                Err(error) => this.reconcile_note_save(retry_snapshot, error, cx),
            });
        })
        .detach();
    }

    /// A revision conflict means the daemon accepted a different write. Fetch
    /// the current revision and retry the still-pending content against it so
    /// the editor's newer text is not silently abandoned.
    fn reconcile_note_save(&mut self, pending: Note, error: anyhow::Error, cx: &mut Context<Self>) {
        let daemon = self.daemon.clone();
        let project_id = pending.project_id;
        let note_id = pending.id;
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let response = daemon.client().request(
                        Uuid::nil(),
                        Uuid::nil(),
                        padu_client::Command::GetNote {
                            project_id,
                            note_id,
                        },
                    )?;
                    let padu_client::ResponsePayload::Note { note: Some(note) } = response else {
                        anyhow::bail!("daemon returned an invalid Notes response");
                    };
                    Ok::<_, anyhow::Error>(note)
                })
                .await;
            let _ = padu.update(cx, |this, cx| match result {
                Ok(note) => {
                    if let Some(current) =
                        this.notes.iter_mut().find(|current| current.id == note.id)
                    {
                        current.revision = note.revision;
                        current.updated_at = note.updated_at;
                    }
                    let mut retry = pending;
                    retry.revision = note.revision;
                    this.persist_note_snapshot(retry, cx);
                }
                Err(_) => {
                    this.show_toast(tr!("notes.save_failed", error = error));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn delete_note_at(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.notes.len() {
            return;
        }
        self.notes_save_generation = self.notes_save_generation.wrapping_add(1);
        let note = self.notes.remove(index);
        self.notes_data_generation = self.notes_data_generation.wrapping_add(1);
        let daemon = self.daemon.clone();
        cx.spawn(async move |_, cx| {
            let _ = cx
                .background_executor()
                .spawn(async move {
                    let _ = daemon.client().request(
                        Uuid::nil(),
                        Uuid::nil(),
                        padu_client::Command::DeleteNote {
                            project_id: note.project_id,
                            note_id: note.id,
                            expected_revision: note.revision,
                        },
                    );
                })
                .await;
        })
        .detach();
        self.notes_selected = self.notes_selected.min(self.notes.len().saturating_sub(1));
        self.sync_note_editors(cx);
        cx.notify();
    }

    /// Creates a new note containing the supplied content and selects it.
    pub(super) fn add_content_to_new_note(
        &mut self,
        content: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let content = content.trim();
        if content.is_empty() {
            return false;
        }
        let Some(project_id) = self.active_project().map(|project| project.id) else {
            self.show_toast(tr!("notes.add_failed"));
            cx.notify();
            return false;
        };
        let title = self
            .selected_session()
            .map(|session| session.display_title().to_owned())
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| tr!("notes.untitled"));
        let content = content.to_owned();
        let daemon = self.daemon.clone();
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let response = daemon.client().request(
                        Uuid::nil(),
                        Uuid::nil(),
                        padu_client::Command::CreateNote {
                            note: padu_client::notes::CreateNote {
                                project_id,
                                title,
                                content,
                            },
                        },
                    )?;
                    let padu_client::ResponsePayload::NoteCreated { note } = response else {
                        anyhow::bail!("daemon returned an invalid Notes create response");
                    };
                    Ok::<_, anyhow::Error>(note)
                })
                .await;
            let _ = padu.update(cx, |this, cx| match result {
                Ok(note) => {
                    this.notes.push(Note::from_protocol(note));
                    this.notes_data_generation = this.notes_data_generation.wrapping_add(1);
                    this.notes_selected = this.notes.len() - 1;
                    this.sync_note_editors(cx);
                    this.show_success_toast(tr!("notes.added_to_note"));
                    cx.notify();
                }
                Err(error) => {
                    this.show_toast(tr!("notes.add_failed_error", error = error));
                    cx.notify();
                }
            });
        })
        .detach();
        true
    }

    pub(super) fn add_content_to_selected_note(
        &mut self,
        content: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let content = content.trim();
        if content.is_empty() {
            return false;
        }
        if self.notes.is_empty() {
            return self.add_content_to_new_note(content, cx);
        }
        let Some(_project_id) = self.active_project().map(|project| project.id) else {
            self.show_toast(tr!("notes.add_failed"));
            cx.notify();
            return false;
        };

        self.notes_save_generation = self.notes_save_generation.wrapping_add(1);
        let title = self.notes_title.read(cx).content().trim().to_owned();
        let body = self.notes_body.read(cx).content().to_owned();
        let Some(note) = self.notes.get_mut(self.notes_selected) else {
            return false;
        };
        note.title = if title.is_empty() {
            tr!("notes.untitled")
        } else {
            title
        };
        note.body = body;
        if !note.body.is_empty() {
            note.body.push_str("\n\n");
        }
        note.body.push_str(content);
        note.refresh_search_key();
        note.updated_at = unix_time();
        self.notes_data_generation = self.notes_data_generation.wrapping_add(1);
        let note_snapshot = note.clone();
        let daemon = self.daemon.clone();
        self.sync_note_editors(cx);
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let response = daemon.client().request(
                        Uuid::nil(),
                        Uuid::nil(),
                        padu_client::Command::UpdateNote {
                            note: padu_client::notes::UpdateNote {
                                project_id: note_snapshot.project_id,
                                note_id: note_snapshot.id,
                                title: note_snapshot.title,
                                content: note_snapshot.body,
                                expected_revision: note_snapshot.revision,
                            },
                        },
                    )?;
                    let padu_client::ResponsePayload::NoteUpdated { note } = response else {
                        anyhow::bail!("daemon returned an invalid Notes update response");
                    };
                    Ok::<_, anyhow::Error>(note)
                })
                .await;
            let _ = padu.update(cx, |this, cx| match result {
                Ok(note) => {
                    if let Some(current) =
                        this.notes.iter_mut().find(|current| current.id == note.id)
                    {
                        current.revision = note.revision;
                        current.updated_at = note.updated_at;
                    }
                    this.show_success_toast(tr!("notes.added_to_note"));
                    cx.notify();
                }
                Err(error) => {
                    this.show_toast(tr!("notes.add_failed_error", error = error));
                    cx.notify();
                }
            });
        })
        .detach();
        true
    }

    fn add_note_to_chat(&mut self, note_id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(note) = self.notes.iter().find(|note| note.id == note_id).cloned() else {
            return;
        };
        // Adding a note is a navigation action as well as a composer update.
        // Leave Notes first so the user can immediately see the embedded note
        // in the active session's composer.
        self.navigate_workspace_page(WorkspacePage::Conversation, cx);
        let already_embedded = self
            .composer_embedded_notes
            .iter()
            .any(|embedded| embedded.id == note.id);
        if !already_embedded {
            self.composer_embedded_notes
                .push(padu_protocol::notes::EmbeddedNote {
                    id: note.id,
                    title: note.title,
                    content: note.body,
                    revision: note.revision,
                });
            self.schedule_composer_draft_save(cx);
            cx.notify();
        }
        let focus = self.composer_focus(cx);
        window.focus(&focus, cx);
    }

    fn notes_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.keystroke.modifiers.modified() {
            return;
        }
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let plain_primary = !modifiers.shift && !modifiers.alt;
        match key {
            "n" if modifiers.alt && !modifiers.shift => self.create_note(cx),
            "1" if plain_primary => {
                self.notes_layout = NotesLayout::Edit;
                cx.notify();
            }
            "2" if plain_primary => {
                self.notes_layout = NotesLayout::Split;
                cx.notify();
            }
            "3" if plain_primary => {
                self.notes_layout = NotesLayout::Preview;
                cx.notify();
            }
            "backspace" if plain_primary => {
                if let Some(note) = self.notes.get(self.notes_selected) {
                    self.confirm_delete_note(note.id, window, cx);
                }
            }
            "enter" if plain_primary => {
                if let Some(note) = self.notes.get(self.notes_selected) {
                    self.add_note_to_chat(note.id, window, cx);
                }
            }
            "l" if modifiers.shift && !modifiers.alt => {
                self.notes_list_collapsed = !self.notes_list_collapsed;
                cx.notify();
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    /// Recompute the visible note indices only when the query or the note set
    /// changed. Frame work therefore stays proportional to the viewport, and
    /// the lowercase search keys are reused instead of reallocated.
    fn refresh_notes_filtered(&mut self, query: &str) {
        if self.notes_filtered_query == query
            && self.notes_filtered_generation == self.notes_data_generation
        {
            return;
        }
        self.notes_filtered_query = query.to_owned();
        self.notes_filtered_generation = self.notes_data_generation;
        let filtered = if query.is_empty() {
            (0..self.notes.len()).collect::<Vec<_>>()
        } else {
            self.notes
                .iter()
                .enumerate()
                .filter(|(_, note)| note.search_key.contains(query))
                .map(|(index, _)| index)
                .collect::<Vec<_>>()
        };
        *self.notes_filtered.borrow_mut() = filtered;
    }

    fn sync_notes_list(&self, count: usize) {
        if self.notes_list_state.item_count() == count {
            return;
        }
        if count == 0 {
            self.notes_list_state.reset(0);
        } else {
            self.notes_list_state
                .reset_with_uniform_height(count, px(56.0));
        }
    }

    /// Virtualized row builder for the Notes list. Reads only cached note and
    /// project state; nothing here touches I/O.
    fn notes_row(&self, row_index: usize, selected: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let Some(&index) = self.notes_filtered.borrow().get(row_index) else {
            return div().into_any_element();
        };
        let Some(note) = self.notes.get(index) else {
            return div().into_any_element();
        };
        let title = if note.title.is_empty() {
            tr!("notes.untitled")
        } else {
            note.title.clone()
        };
        let preview = note
            .body
            .lines()
            .next()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned);
        let project_name = self
            .state
            .projects
            .iter()
            .find(|project| project.id == note.project_id)
            .map(Project::display_name)
            .unwrap_or_else(|| tr!("notes.no_project"));
        let created_ago =
            super::notes_utils::format_note_time_ago(unix_time().saturating_sub(note.created_at));
        let note_id = note.id;
        let note_menu = self.menu_handle(SharedString::from(format!("note-menu-{note_id}")), cx);
        let weak = cx.entity().downgrade();
        let row = div()
            .id(SharedString::from(format!("note-row-{note_id}")))
            .tab_index(0)
            .tab_stop(true)
            .w_full()
            .min_h(px(56.0))
            .px(px(9.0))
            .py(px(7.0))
            .rounded(px(8.0))
            .cursor_pointer()
            .when(index == selected, |row| {
                row.bg(theme.sidebar_item_background)
            })
            .hover(|row| row.bg(theme.overlay))
            .focus_visible(|row| row.border_1().border_color(theme.accent))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .w(px(26.0))
                    .h(px(26.0))
                    .flex_none()
                    .rounded(px(6.0))
                    .bg(theme.overlay)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon("icons/file.svg", 13.0, theme.text_secondary)),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(1.0))
                    .child(
                        div()
                            .truncate()
                            .text_size(sp(12.5))
                            .text_color(theme.text)
                            .child(title),
                    )
                    .when_some(preview, |column, preview| {
                        column.child(
                            div()
                                .truncate()
                                .text_size(sp(11.0))
                                .text_color(theme.text_tertiary)
                                .child(preview),
                        )
                    })
                    .child(
                        div()
                            .truncate()
                            .text_size(sp(10.0))
                            .text_color(theme.text_tertiary)
                            .child(format!("{project_name} · {created_ago}")),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_note(index, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    this.select_note(index, cx);
                    cx.stop_propagation();
                }
            }));
        context_menu(
            row,
            SharedString::from(format!("note-row-menu-{note_id}")),
            &note_menu,
            move |_| {
                let weak = weak.clone();
                let add_to_chat_id = note_id;
                let delete_weak = weak.clone();
                vec![
                    MenuItem::new(tr!("files.add_to_chat"), move |window, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.add_note_to_chat(add_to_chat_id, window, cx);
                        });
                    })
                    .icon("icons/compose.svg"),
                    MenuItem::new(tr!("notes.delete"), move |window, cx| {
                        let _ = delete_weak.update(cx, |this, cx| {
                            this.confirm_delete_note(note_id, window, cx);
                        });
                    })
                    .icon("icons/trash.svg")
                    .destructive(true),
                ]
            },
        )
        .into_any_element()
    }

    pub(super) fn render_notes_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let selected = self.notes_selected;
        let query = self.notes_search.read(cx).content().trim().to_lowercase();
        self.refresh_notes_filtered(&query);
        let notes_empty = self.notes.is_empty();
        let filtered_empty = self.notes_filtered.borrow().is_empty();
        self.sync_notes_list(self.notes_filtered.borrow().len());
        let entity = cx.entity().downgrade();
        let list: AnyElement = if filtered_empty {
            div()
                .w_full()
                .py(px(28.0))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.0))
                .text_center()
                .child(icon("icons/file.svg", 22.0, theme.text_tertiary))
                .child(
                    div()
                        .text_size(sp(13.0))
                        .text_color(theme.text_secondary)
                        .child(if notes_empty {
                            tr!("notes.empty_title")
                        } else {
                            tr!("notes.no_matches")
                        }),
                )
                .child(
                    div()
                        .text_size(sp(11.0))
                        .text_color(theme.text_tertiary)
                        .child(if notes_empty {
                            tr!("notes.empty_message")
                        } else {
                            query.clone()
                        }),
                )
                .into_any_element()
        } else {
            list(
                self.notes_list_state.clone(),
                move |row_index, _window, cx| {
                    entity
                        .upgrade()
                        .map(|entity| {
                            entity.update(cx, |this, cx| this.notes_row(row_index, selected, cx))
                        })
                        .unwrap_or_else(|| div().into_any_element())
                },
            )
            .size_full()
            .into_any_element()
        };

        let selected_note = self.notes.get(selected);
        let layout = self.notes_layout;
        let mut editor = div().flex_1().min_w_0().flex().flex_col().gap(px(10.0));
        if let Some(note) = selected_note {
            let note_id = note.id;
            let created_ago = super::notes_utils::format_note_time_ago(
                unix_time().saturating_sub(note.created_at),
            );
            let updated_ago = super::notes_utils::format_note_time_ago(
                unix_time().saturating_sub(note.updated_at),
            );
            editor = editor
                .child(
                    div()
                        .w_full()
                        .h(px(38.0))
                        .flex()
                        .items_center()
                        .text_size(sp(22.0))
                        .font_weight(FontWeight::BOLD)
                        .child(self.notes_title.clone()),
                )
                .child(
                    div()
                        .w_full()
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.0))
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .text_size(sp(11.0))
                                .text_color(theme.text_tertiary)
                                .child(self.notes_button(
                                    "note-add-to-chat",
                                    tr!("files.add_to_chat"),
                                    cx,
                                    move |this, window, cx| {
                                        this.add_note_to_chat(note_id, window, cx);
                                    },
                                ))
                                .child(tr!("notes.created", time = created_ago))
                                .child(tr!("notes.updated", time = updated_ago)),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(1.0))
                                        .p(px(2.0))
                                        .rounded(px(7.0))
                                        .bg(theme.overlay)
                                        .child(self.notes_button(
                                            "note-toggle-preview",
                                            tr!("notes.edit"),
                                            cx,
                                            |this, _, cx| {
                                                this.notes_layout = NotesLayout::Edit;
                                                cx.notify();
                                            },
                                        ))
                                        .child(self.notes_button(
                                            "note-split",
                                            tr!("notes.side_by_side"),
                                            cx,
                                            |this, _, cx| {
                                                this.notes_layout = NotesLayout::Split;
                                                cx.notify();
                                            },
                                        ))
                                        .child(self.notes_button(
                                            "note-preview",
                                            tr!("notes.preview"),
                                            cx,
                                            |this, _, cx| {
                                                this.notes_layout = NotesLayout::Preview;
                                                cx.notify();
                                            },
                                        )),
                                )
                                .child(self.notes_button(
                                    "note-delete",
                                    tr!("notes.delete"),
                                    cx,
                                    |this, window, cx| {
                                        if let Some(note) = this.notes.get(this.notes_selected) {
                                            this.confirm_delete_note(note.id, window, cx);
                                        }
                                    },
                                )),
                        ),
                );
            let preview_pane = if matches!(layout, NotesLayout::Preview | NotesLayout::Split) {
                let palette = MarkdownPalette::from_theme(&theme);
                let cache_key = format!("note:{}", note.id);
                let mut cache = self.file_preview_markdown.borrow_mut();
                if !matches!(cache.as_ref(), Some((key, _)) if key == &cache_key) {
                    *cache = Some((cache_key.clone(), MarkdownView::new()));
                }
                let (_, view) = cache.as_mut().expect("note preview cache entry ensured");
                view.set_text(&note.body, false);
                let ctx = MarkdownCtx::new(
                    format!("note-preview-{}", note.id),
                    &palette,
                    self.scaled_markdown_metrics(MarkdownMetrics::BODY),
                    self.file_preview_selection.clone(),
                );
                let document = md::render::markdown(view, &ctx);
                drop(cache);
                let selection_input = canvas(|_, _, _| (), {
                    let selection = self.file_preview_selection.clone();
                    move |_, _, window, _| md::render::install_selection_input(window, &selection)
                })
                .absolute()
                .w(px(0.0))
                .h(px(0.0));
                Some(
                    div()
                        .id("note-preview-pane")
                        .flex_1()
                        .min_h_full()
                        .min_w_0()
                        .relative()
                        .overflow_y_scroll()
                        .track_scroll(&self.file_preview_scroll_handle)
                        .p(px(14.0))
                        .rounded(px(8.0))
                        .bg(theme.inset)
                        .child(md::render::frame_reset(self.file_preview_selection.clone()))
                        .child(
                            div()
                                .w_full()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_normal()
                                .children(document),
                        )
                        .child(selection_input)
                        .child(scrollbar::vertical(
                            &self.file_preview_scroll_handle,
                            &self.file_preview_scrollbar,
                        )),
                )
            } else {
                None
            };
            let edit_pane = if matches!(layout, NotesLayout::Edit | NotesLayout::Split) {
                let notes_body = self.notes_body.clone();
                Some(
                    self.render_file_editor_body(
                        "notes.md",
                        &notes_body,
                        720.0,
                        true,
                        theme.inset,
                        window,
                        cx,
                    )
                    .rounded(px(8.0)),
                )
            } else {
                None
            };
            editor = match layout {
                NotesLayout::Edit => editor.child(edit_pane.expect("editor pane is present")),
                NotesLayout::Preview => {
                    editor.child(preview_pane.expect("preview pane is present"))
                }
                NotesLayout::Split => {
                    let edit_pane = edit_pane.expect("editor pane is present");
                    let preview_pane = preview_pane.expect("preview pane is present");
                    let split_ratio = self.notes_split_ratio;
                    editor.child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .gap(px(6.0))
                            .child(
                                edit_pane
                                    .w(px(0.0))
                                    .flex_grow(split_ratio)
                                    .flex_shrink(1.0)
                                    .min_w_0(),
                            )
                            .child(
                                div()
                                    .relative()
                                    .ml(px(4.0))
                                    .pl(px(8.0))
                                    .w(px(0.0))
                                    .flex_grow(1.0 - split_ratio)
                                    .flex_shrink(1.0)
                                    .min_w_0()
                                    .child(self.render_panel_resize_handle(
                                        "notes-split-resize-handle",
                                        PanelResizeTarget::NotesSplit,
                                        cx,
                                    ))
                                    .child(preview_pane),
                            ),
                    )
                }
            };
        }

        let list_collapsed = self.notes_list_collapsed;
        let mut body = div()
            .id("notes-page-body")
            .flex_1()
            .min_h_0()
            .flex()
            .gap(px(16.0))
            .px(px(16.0))
            .pb(px(24.0))
            .pt(px(0.0));
        if !list_collapsed {
            body = body.child(
                div()
                    .w(px(240.0))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        self.notes_button("note-new", tr!("notes.new"), cx, |this, _, cx| {
                            this.create_note(cx);
                        }),
                    )
                    .child(
                        div()
                            .h(px(30.0))
                            .flex_none()
                            .rounded(px(6.0))
                            .bg(theme.inset)
                            .child(
                                TextField::new("notes-search", self.notes_search.clone())
                                    .icon("icons/search.svg", 13.0),
                            ),
                    )
                    .child(
                        div()
                            .id("notes-list-scroll")
                            .flex_1()
                            .min_h_0()
                            .relative()
                            .child(list)
                            .child(scrollbar::vertical(
                                &self.notes_list_state,
                                &self.notes_scrollbar,
                            )),
                    ),
            );
        }
        body = body.child(editor);

        div()
            .id("notes-page")
            .key_context("NotesPage")
            .on_key_down(cx.listener(Self::notes_key_down))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.canvas)
            .child(
                div()
                    .id("notes-page-header")
                    .h(px(48.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(16.0))
                    .child(icon("icons/file.svg", 16.0, theme.text_secondary))
                    .child(
                        div()
                            .text_size(sp(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(tr!("settings.notes")),
                    )
                    .child(self.window_drag_region(
                        div().id("notes-page-drag-region").h_full().flex_1(),
                        cx,
                    ))
                    .child(self.notes_button(
                        "note-toggle-list",
                        if list_collapsed {
                            tr!("notes.expand")
                        } else {
                            tr!("notes.collapse")
                        },
                        cx,
                        |this, _, cx| {
                            this.notes_list_collapsed = !this.notes_list_collapsed;
                            cx.notify();
                        },
                    )),
            )
            .child(body)
            .into_any_element()
    }

    fn notes_button(
        &self,
        id: &'static str,
        label: String,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Padu, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let theme = Theme::current(cx);
        let selected = match id {
            "note-toggle-preview" => self.notes_layout == NotesLayout::Edit,
            "note-split" => self.notes_layout == NotesLayout::Split,
            "note-preview" => self.notes_layout == NotesLayout::Preview,
            _ => false,
        };
        let (icon_path, icon_only) = match id {
            "note-new" => ("icons/plus.svg", false),
            "note-toggle-preview" => ("icons/pencil.svg", true),
            "note-split" => ("icons/panel-right.svg", true),
            "note-preview" => ("icons/eye.svg", true),
            "note-delete" => ("icons/trash.svg", true),
            "note-add-to-chat" => ("icons/compose.svg", false),
            "note-toggle-list" => ("icons/panel-left.svg", true),
            _ => ("icons/check.svg", false),
        };
        let foreground = if id == "note-add-to-chat" {
            theme.on_inverse
        } else {
            theme.text_secondary
        };
        let tooltip = label.clone();
        let shortcut = match id {
            "note-new" => crate::platform::primary_shortcut("⌘⌥N", "Ctrl+Alt+N"),
            "note-toggle-preview" => crate::platform::primary_shortcut("⌘1", "Ctrl+1"),
            "note-split" => crate::platform::primary_shortcut("⌘2", "Ctrl+2"),
            "note-preview" => crate::platform::primary_shortcut("⌘3", "Ctrl+3"),
            "note-delete" => crate::platform::primary_shortcut("⌘⌫", "Ctrl+Backspace"),
            "note-add-to-chat" => crate::platform::primary_shortcut("⌘↵", "Ctrl+Enter"),
            "note-toggle-list" => crate::platform::primary_shortcut("⌘⇧L", "Ctrl+Shift+L"),
            _ => "",
        };
        let shortcut_id = id;
        let tooltip = Tooltip::with_shortcut(tooltip, shortcut);
        let action = Rc::new(action);
        let key_action = action.clone();
        div()
            .id(id)
            .tab_index(0)
            .tab_stop(true)
            .h(px(28.0))
            .px(px(10.0))
            .rounded(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .cursor_pointer()
            .when(selected, |button| button.bg(theme.overlay_strong))
            .text_size(sp(12.5))
            .text_color(foreground)
            .focus_visible(|style| style.border_1().border_color(theme.accent))
            .when(id != "note-add-to-chat", |button| {
                button.hover(|style| style.bg(theme.overlay_strong))
            })
            .when(id == "note-add-to-chat", |button| button.bg(theme.inverse))
            .when(id != "note-add-to-chat", |button| button.tooltip(tooltip))
            .on_click(cx.listener(move |this, _, window, cx| action(this, window, cx)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                let shortcut_pressed = event.keystroke.modifiers.modified()
                    && match shortcut_id {
                        "note-new" => {
                            key == "n"
                                && event.keystroke.modifiers.alt
                                && !event.keystroke.modifiers.shift
                        }
                        "note-toggle-preview" => key == "1",
                        "note-split" => key == "2",
                        "note-preview" => key == "3",
                        "note-delete" => key == "backspace",
                        "note-add-to-chat" => key == "enter",
                        "note-toggle-list" => {
                            key == "l"
                                && event.keystroke.modifiers.shift
                                && !event.keystroke.modifiers.alt
                        }
                        _ => false,
                    };
                if (!event.keystroke.modifiers.modified() && matches!(key, "enter" | "space"))
                    || shortcut_pressed
                {
                    key_action(this, window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(icon(icon_path, 14.0, foreground))
            .when(!icon_only, |button| button.child(label))
            .when(matches!(id, "note-new" | "note-add-to-chat"), |button| {
                button.child(crate::ui::kbd_badge(shortcut, &theme))
            })
    }
}
