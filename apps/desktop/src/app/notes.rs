use super::*;

fn format_timestamp(value: u64) -> String {
    DateTime::from_timestamp(value as i64, 0)
        .map(|timestamp| {
            timestamp
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "—".to_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NotesLayout {
    Edit,
    Split,
    Preview,
}

#[derive(Clone, Debug)]
pub(super) struct Note {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub body: String,
    pub revision: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

impl Padu {
    pub(super) fn open_notes(&mut self, cx: &mut Context<Self>) {
        self.settings_page = None;
        self.workspace_page = WorkspacePage::Notes;
        let Some(project_id) = self.active_project().map(|project| project.id) else {
            self.notes.clear();
            self.notes_selected = 0;
            cx.notify();
            return;
        };
        self.load_notes_from_daemon(project_id, cx);
        cx.notify();
    }

    fn load_notes_from_daemon(&mut self, project_id: Uuid, cx: &mut Context<Self>) {
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
                                project_id,
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
                if let Ok(notes) = result {
                    this.notes = notes
                        .into_iter()
                        .map(|note| Note {
                            id: note.id,
                            project_id: note.project_id,
                            title: note.title,
                            body: note.content,
                            revision: note.revision,
                            created_at: note.created_at,
                            updated_at: note.updated_at,
                        })
                        .collect();
                    this.notes_selected =
                        this.notes_selected.min(this.notes.len().saturating_sub(1));
                    this.sync_note_editors(cx);
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
                    this.notes.push(Note {
                        id: note.id,
                        project_id: note.project_id,
                        title: note.title,
                        body: note.content,
                        revision: note.revision,
                        created_at: note.created_at,
                        updated_at: note.updated_at,
                    });
                    this.notes_selected = this.notes.len() - 1;
                    this.sync_note_editors(cx);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn save_note_edit(&mut self, cx: &mut Context<Self>) {
        let Some(note) = self.notes.get_mut(self.notes_selected) else {
            return;
        };
        note.title = self.notes_title.read(cx).content().trim().to_owned();
        if note.title.is_empty() {
            note.title = tr!("notes.untitled");
        }
        note.body = self.notes_body.read(cx).content().to_owned();
        note.updated_at = unix_time();
        let note_snapshot = note.clone();
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
            let _ = padu.update(cx, |this, cx| {
                if let Ok(note) = result
                    && let Some(current) =
                        this.notes.iter_mut().find(|current| current.id == note.id)
                {
                    current.revision = note.revision;
                    current.updated_at = note.updated_at;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn delete_note(&mut self, cx: &mut Context<Self>) {
        if self.notes.is_empty() {
            return;
        }
        let note = self.notes.remove(self.notes_selected);
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

    pub(super) fn add_content_to_selected_note(&mut self, content: &str, cx: &mut Context<Self>) {
        if self.notes.is_empty() {
            self.create_note(cx);
            return;
        }
        self.save_note_edit(cx);
        let Some(note) = self.notes.get_mut(self.notes_selected) else {
            return;
        };
        if !note.body.is_empty() {
            note.body.push_str("\n\n");
        }
        note.body.push_str(content.trim());
        note.updated_at = unix_time();
        self.sync_note_editors(cx);
        self.open_notes(cx);
    }

    pub(super) fn render_notes_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let notes = self.notes.clone();
        let selected = self.notes_selected;
        let mut list = div().flex().flex_col().gap(px(3.0));
        for (index, note) in notes.iter().enumerate() {
            let title = if note.title.is_empty() {
                tr!("notes.untitled")
            } else {
                note.title.clone()
            };
            let label = if note.body.is_empty() {
                title.clone()
            } else {
                format!("{title}\n{}", note.body.lines().next().unwrap_or_default())
            };
            list = list.child(
                div()
                    .id(SharedString::from(format!("note-row-{}", note.id)))
                    .tab_index(0)
                    .tab_stop(true)
                    .w_full()
                    .min_h(px(48.0))
                    .px(px(10.0))
                    .py(px(7.0))
                    .rounded(px(7.0))
                    .cursor_pointer()
                    .when(index == selected, |row| {
                        row.bg(theme.sidebar_item_background)
                    })
                    .hover(|row| row.bg(theme.overlay))
                    .focus_visible(|row| row.border_1().border_color(theme.accent))
                    .text_size(sp(12.5))
                    .text_color(theme.text)
                    .whitespace_normal()
                    .child(SharedString::from(label))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_note(index, cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            this.select_note(index, cx);
                            cx.stop_propagation();
                        }
                    })),
            );
        }

        let selected_note = self.notes.get(selected);
        let layout = self.notes_layout;
        let mut editor = div().flex_1().min_w_0().flex().flex_col().gap(px(10.0));
        if let Some(note) = selected_note {
            editor = editor
                .child(TextField::new("note-title", self.notes_title.clone()))
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .gap(px(6.0))
                        .child(
                            self.notes_button("note-save", tr!("notes.save"), cx, |this, cx| {
                                this.save_note_edit(cx);
                                cx.notify();
                            }),
                        )
                        .child(self.notes_button(
                            "note-toggle-preview",
                            tr!("notes.edit"),
                            cx,
                            |this, cx| {
                                this.save_note_edit(cx);
                                this.notes_layout = NotesLayout::Edit;
                                cx.notify();
                            },
                        ))
                        .child(self.notes_button(
                            "note-split",
                            tr!("notes.side_by_side"),
                            cx,
                            |this, cx| {
                                this.save_note_edit(cx);
                                this.notes_layout = NotesLayout::Split;
                                cx.notify();
                            },
                        ))
                        .child(self.notes_button(
                            "note-preview",
                            tr!("notes.preview"),
                            cx,
                            |this, cx| {
                                this.save_note_edit(cx);
                                this.notes_layout = NotesLayout::Preview;
                                cx.notify();
                            },
                        ))
                        .child(self.notes_button(
                            "note-delete",
                            tr!("notes.delete"),
                            cx,
                            |this, cx| {
                                this.delete_note(cx);
                            },
                        )),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(sp(11.0))
                        .text_color(theme.text_tertiary)
                        .child(format!(
                            "{} · created {} · updated {}",
                            self.active_project()
                                .map(|project| project.display_name())
                                .unwrap_or_else(|| tr!("notes.no_project")),
                            format_timestamp(note.created_at),
                            format_timestamp(note.updated_at),
                        )),
                );
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
            let preview_pane = div()
                .id("note-preview-pane")
                .flex_1()
                .min_h_0()
                .relative()
                .overflow_y_scroll()
                .track_scroll(&self.file_preview_scroll_handle)
                .p(px(14.0))
                .rounded(px(8.0))
                .bg(theme.inset)
                .child(md::render::frame_reset(self.file_preview_selection.clone()))
                .children(document)
                .child(selection_input)
                .child(scrollbar::vertical(
                    &self.file_preview_scroll_handle,
                    &self.file_preview_scrollbar,
                ));
            let edit_pane = div()
                .id("note-editor-pane")
                .flex_1()
                .min_h(px(180.0))
                .min_w_0()
                .p(px(10.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(theme.border_strong)
                .bg(theme.inset)
                .child(self.notes_body.clone());
            editor = match layout {
                NotesLayout::Edit => editor.child(edit_pane),
                NotesLayout::Preview => editor.child(preview_pane),
                NotesLayout::Split => editor.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .gap(px(10.0))
                        .child(edit_pane)
                        .child(preview_pane),
                ),
            };
        }

        div()
            .id("notes-page")
            .size_full()
            .flex()
            .gap(px(16.0))
            .p(px(24.0))
            .child(
                div()
                    .w(px(220.0))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        self.notes_button("note-new", tr!("notes.new"), cx, |this, cx| {
                            this.create_note(cx);
                        }),
                    )
                    .child(
                        div()
                            .id("notes-list-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(list),
                    ),
            )
            .child(editor)
            .into_any_element()
    }

    fn notes_button(
        &self,
        id: &'static str,
        label: String,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Padu, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let theme = Theme::current(cx);
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
            .cursor_pointer()
            .bg(theme.overlay)
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .focus_visible(|style| style.border_1().border_color(theme.accent))
            .hover(|style| style.bg(theme.overlay_strong))
            .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    key_action(this, cx);
                    cx.stop_propagation();
                }
            }))
            .child(label)
    }
}
