use gpui::{KeyBinding, actions};

use super::*;

actions!(padu_inline_file, [CancelInlineFile, SubmitInlineFile]);

pub(crate) const INLINE_FILE_PARENT_CONTEXT: &str = "InlineFileField";
pub(crate) const INLINE_FILE_FIELD_CONTEXT: &str = "InlineFileField > TextInput";

pub fn init_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", CancelInlineFile, Some(INLINE_FILE_FIELD_CONTEXT)),
        KeyBinding::new("enter", SubmitInlineFile, Some(INLINE_FILE_FIELD_CONTEXT)),
    ]);
}

impl Padu {
    pub(crate) fn render_right_panel_files(
        &mut self,
        panel_width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        if let Some(relative_path) = self.right_panel_files_selected_path.clone() {
            self.render_right_panel_file(relative_path, panel_width, window, cx)
        } else {
            self.render_right_panel_working_tree(None, cx)
        }
    }

    pub(crate) fn render_right_panel_working_tree(
        &self,
        selected_path: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = Theme::current(cx);
        let Some(project) = self.active_project() else {
            return self.render_right_panel_empty_message(
                tr!("files.no_project_open"),
                tr!("files.no_project_open_description"),
                cx,
            );
        };
        let show_project_name = selected_path.is_none();
        let project_name = project.display_name();
        // Read only. The walk is filesystem I/O, so it happens in
        // `refresh_right_panel_working_tree`, never in a frame.
        let entries = self.right_panel_working_tree.clone();
        let focus = self.transcript_control_focus("right-panel-working-tree", cx);

        let cursor_index = self
            .right_panel_files_cursor
            .filter(|&index| index < entries.len());

        let empty_menu = self.menu_handle(
            SharedString::from("right-panel-working-tree-empty-context"),
            cx,
        );
        let root_path = self.selected_workspace_path().map(Path::to_path_buf);
        let weak = cx.entity().downgrade();

        let inline_create = match &self.right_panel_inline_file_operation {
            Some(InlineFileOperation {
                kind: InlineFileOperationKind::CreateFile { parent, depth },
                input,
            }) => Some((false, parent.clone(), *depth, input.clone())),
            Some(InlineFileOperation {
                kind: InlineFileOperationKind::CreateDirectory { parent, depth },
                input,
            }) => Some((true, parent.clone(), *depth, input.clone())),
            _ => None,
        };
        let renaming_source = match &self.right_panel_inline_file_operation {
            Some(InlineFileOperation {
                kind: InlineFileOperationKind::Rename { source },
                input,
            }) => Some((source.clone(), input.clone())),
            _ => None,
        };
        let is_root_create = inline_create
            .as_ref()
            .is_some_and(|(_, parent, _, _)| root_path.as_ref() == Some(parent));
        let mut rendered_inline_create = is_root_create;

        let mut list = div()
            .id("right-panel-working-tree")
            .track_focus(&focus)
            .tab_index(0)
            .key_context("WorkingTree")
            .size_full()
            .min_h_full()
            .overflow_y_scroll()
            .track_scroll(&self.right_panel_files_scroll_handle)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    let focus = this.transcript_control_focus("right-panel-working-tree", cx);
                    window.focus(&focus, cx);
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.right_panel_working_tree_key_down(event, window, cx);
            }))
            .flex()
            .flex_col()
            .py(px(6.0));

        if is_root_create {
            if let Some((is_dir, _, depth, ref input)) = inline_create {
                list = list.child(self.render_inline_create_row(is_dir, depth, input, cx));
            }
        }

        for (index, entry) in entries.into_iter().enumerate() {
            let relative_path = entry.relative_path.clone();
            let absolute_path = entry.absolute_path.clone();
            let is_dir = entry.is_dir;
            let entry_depth = entry.depth;
            let selected = selected_path == Some(relative_path.as_str());
            let is_cursor = cursor_index == Some(index);
            let is_renaming = renaming_source
                .as_ref()
                .is_some_and(|(src, _)| src == &absolute_path);
            let rename_input = renaming_source
                .as_ref()
                .filter(|(src, _)| src == &absolute_path)
                .map(|(_, inp)| inp);

            let row = div()
                .id(index)
                .h(px(30.0))
                .min_h(px(30.0))
                .flex_none()
                .mx(px(8.0))
                .pl(px(8.0 + entry.depth as f32 * 16.0))
                .pr(px(8.0))
                .rounded(px(6.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .cursor_pointer()
                .when(selected && is_cursor, |element| {
                    element.bg(theme.overlay_strong)
                })
                .when(selected ^ is_cursor, |element| element.bg(theme.overlay))
                .when(!selected && !is_cursor, |element| {
                    element.hover(|element| element.bg(theme.overlay))
                })
                .when(entry.is_ignored, |element| element.opacity(0.55))
                .child(if is_dir {
                    icon(
                        if entry.expanded {
                            "icons/chevron-down.svg"
                        } else {
                            "icons/chevron-right.svg"
                        },
                        10.0,
                        theme.text_ghost,
                    )
                    .into_any_element()
                } else {
                    div().w(px(10.0)).h(px(10.0)).flex_none().into_any_element()
                })
                .child(if is_dir {
                    icon(
                        if entry.expanded {
                            "icons/folder-open.svg"
                        } else {
                            "icons/folder.svg"
                        },
                        14.0,
                        theme.text_tertiary,
                    )
                    .into_any_element()
                } else if let Some(file_icon_path) = entry.file_icon {
                    file_icon(file_icon_path, 14.0).into_any_element()
                } else {
                    div().w(px(14.0)).h(px(14.0)).flex_none().into_any_element()
                })
                .child(if let Some(input) = rename_input {
                    self.render_inline_rename_widget(input, cx)
                } else {
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_size(sp(12.5))
                        .text_color(if entry.is_ignored {
                            theme.text_ghost
                        } else {
                            theme.text_secondary
                        })
                        .child(entry.name)
                        .into_any_element()
                });
            let menu = self.menu_handle(
                SharedString::from(format!("right-panel-file-menu-{relative_path}")),
                cx,
            );
            let keyboard_menu = menu.clone();
            let menu_path = absolute_path.clone();
            let menu_relative = relative_path.clone();

            let click_path = absolute_path.clone();
            let row_weak = weak.clone();
            let mut row = if is_renaming {
                row
            } else if is_dir {
                row.on_click(cx.listener(move |this, _, window, cx| {
                    this.right_panel_files_cursor = Some(index);
                    let focus = this.transcript_control_focus("right-panel-working-tree", cx);
                    window.focus(&focus, cx);
                    if !this.right_panel_expanded_paths.remove(&click_path) {
                        this.right_panel_expanded_paths.insert(click_path.clone());
                    }
                    this.refresh_right_panel_working_tree(cx);
                    cx.notify();
                }))
            } else {
                row.on_click(cx.listener(move |this, _, window, cx| {
                    this.right_panel_files_cursor = Some(index);
                    let focus = this.transcript_control_focus("right-panel-working-tree", cx);
                    window.focus(&focus, cx);
                    this.open_right_panel_file(relative_path.clone(), cx);
                }))
            };
            row = row.on_key_down(move |event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "f10" && event.keystroke.modifiers.shift {
                    keyboard_menu.open_context_menu(window, cx);
                    cx.stop_propagation();
                }
            });
            list = list.child(context_menu(
                row,
                SharedString::from(format!("right-panel-file-context-{menu_relative}")),
                &menu,
                move |_| {
                    let copy_path = menu_path.clone();
                    let copy_relative = menu_relative.clone();
                    let rename_weak = row_weak.clone();
                    let delete_weak = row_weak.clone();
                    let new_file_weak = row_weak.clone();
                    let new_folder_weak = row_weak.clone();
                    let chat_weak = row_weak.clone();
                    let mut items = Vec::new();

                    // Add to Chat inserts a textual inline reference. It must not
                    // stage or upload the file as an attachment.
                    let chat_relative = if is_dir {
                        format!("{menu_relative}/")
                    } else {
                        menu_relative.clone()
                    };
                    items.push(
                        MenuItem::new(tr!("files.add_to_chat"), move |window, cx| {
                            let _ = chat_weak.update(cx, |this, cx| {
                                this.composer.update(cx, |composer, cx| {
                                    composer.insert_file_reference(&chat_relative, cx);
                                });
                                let focus = this.composer.read(cx).focus();
                                window.focus(&focus, cx);
                            });
                        })
                        .icon("icons/compose.svg"),
                    );

                    if is_dir {
                        let parent = menu_path.clone();
                        let depth = entry_depth + 1;
                        items.push(
                            MenuItem::new(tr!("files.new_file"), move |window, cx| {
                                let _ = new_file_weak.update(cx, |this, cx| {
                                    this.begin_inline_create(
                                        false,
                                        parent.clone(),
                                        depth,
                                        window,
                                        cx,
                                    );
                                });
                            })
                            .icon("icons/file-add.svg"),
                        );
                        let parent = menu_path.clone();
                        items.push(
                            MenuItem::new(tr!("files.new_folder"), move |window, cx| {
                                let _ = new_folder_weak.update(cx, |this, cx| {
                                    this.begin_inline_create(
                                        true,
                                        parent.clone(),
                                        depth,
                                        window,
                                        cx,
                                    );
                                });
                            })
                            .icon("icons/folder-new.svg"),
                        );
                    } else {
                        let parent = menu_path
                            .parent()
                            .map(Path::to_path_buf)
                            .unwrap_or_else(|| menu_path.clone());
                        let depth = entry_depth;
                        items.push(
                            MenuItem::new(tr!("files.new_file"), move |window, cx| {
                                let _ = new_file_weak.update(cx, |this, cx| {
                                    this.begin_inline_create(
                                        false,
                                        parent.clone(),
                                        depth,
                                        window,
                                        cx,
                                    );
                                });
                            })
                            .icon("icons/file-add.svg"),
                        );
                        let parent = menu_path
                            .parent()
                            .map(Path::to_path_buf)
                            .unwrap_or_else(|| menu_path.clone());
                        items.push(
                            MenuItem::new(tr!("files.new_folder"), move |window, cx| {
                                let _ = new_folder_weak.update(cx, |this, cx| {
                                    this.begin_inline_create(
                                        true,
                                        parent.clone(),
                                        depth,
                                        window,
                                        cx,
                                    );
                                });
                            })
                            .icon("icons/folder-new.svg"),
                        );
                    }
                    items.push(
                        MenuItem::new(tr!("files.copy_path"), {
                            let copy_path = copy_path.clone();
                            let copy_weak = row_weak.clone();
                            move |_, cx| {
                                let path_string = copy_path.to_string_lossy().into_owned();
                                let file_name = copy_path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or(&path_string)
                                    .to_owned();
                                cx.write_to_clipboard(ClipboardItem::new_string(path_string));
                                let _ = copy_weak.update(cx, |this, _| {
                                    this.show_success_toast(tr!(
                                        "files.copied_path",
                                        path = file_name
                                    ));
                                });
                            }
                        })
                        .icon("icons/copy.svg"),
                    );
                    items.push(
                        MenuItem::new(tr!("files.copy_relative_path"), {
                            let copy_relative = copy_relative.clone();
                            let copy_weak = row_weak.clone();
                            move |_, cx| {
                                let file_name = Path::new(&copy_relative)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or(&copy_relative)
                                    .to_owned();
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    copy_relative.clone(),
                                ));
                                let _ = copy_weak.update(cx, |this, _| {
                                    this.show_success_toast(tr!(
                                        "files.copied_path",
                                        path = file_name
                                    ));
                                });
                            }
                        })
                        .icon("icons/copy.svg"),
                    );
                    let source = menu_path.clone();
                    items.push(
                        MenuItem::new(tr!("common.rename"), move |window, cx| {
                            let _ = rename_weak.update(cx, |this, cx| {
                                this.begin_inline_rename(source.clone(), window, cx);
                            });
                        })
                        .icon("icons/pencil.svg"),
                    );
                    let target = menu_path.clone();
                    items.push(MenuItem::Separator);
                    items.push(
                        MenuItem::new(tr!("files.delete"), move |window, cx| {
                            let _ = delete_weak.update(cx, |this, cx| {
                                this.confirm_delete_path(target.clone(), window, cx);
                            });
                        })
                        .icon("icons/trash.svg")
                        .destructive(true),
                    );
                    items
                },
            ));

            if !is_root_create {
                if let Some((is_dir, ref parent_path, create_depth, ref create_input)) =
                    inline_create
                {
                    if &absolute_path == parent_path {
                        list = list.child(self.render_inline_create_row(
                            is_dir,
                            create_depth,
                            create_input,
                            cx,
                        ));
                        rendered_inline_create = true;
                    }
                }
            }
        }

        if !rendered_inline_create {
            if let Some((is_dir, _, depth, ref input)) = inline_create {
                list = list.child(self.render_inline_create_row(is_dir, depth, input, cx));
            }
        }

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(42.0))
                    .flex_none()
                    .px(px(16.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .when(show_project_name, |header| {
                        header.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(icon("icons/folder.svg", 13.0, theme.text_tertiary))
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_size(sp(12.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text_secondary)
                                        .child(project_name.clone()),
                                ),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .flex_none()
                            .child({
                                let focus =
                                    self.transcript_control_focus("right-panel-find-file", cx);
                                div()
                                    .id("right-panel-find-file")
                                    .track_focus(&focus)
                                    .tab_index(0)
                                    .size(px(26.0))
                                    .rounded(px(7.0))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .focus_visible(|style| {
                                        style.border_1().border_color(theme.accent)
                                    })
                                    .hover(|style| style.bg(theme.overlay))
                                    .child(icon("icons/search.svg", 13.0, theme.text_tertiary))
                                    .tooltip(|window, cx| {
                                        Tooltip::new(tr!("command_palette.find_file"))
                                            .build(window, cx)
                                    })
                                    .on_click(|_, _, cx| {
                                        cx.defer(|cx| cx.dispatch_action(&OpenFilePicker));
                                    })
                                    .on_key_down(|event: &KeyDownEvent, _, cx| {
                                        if matches!(event.keystroke.key.as_str(), "enter" | "space")
                                        {
                                            cx.defer(|cx| cx.dispatch_action(&OpenFilePicker));
                                            cx.stop_propagation();
                                        }
                                    })
                            })
                            .child(
                                icon_button("right-panel-new-file", "icons/file-add.svg", theme)
                                    .tooltip(|window, cx| {
                                        Tooltip::new(tr!("files.new_file")).build(window, cx)
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if let Some(root) =
                                            this.selected_workspace_path().map(Path::to_path_buf)
                                        {
                                            this.begin_inline_create(false, root, 0, window, cx);
                                        }
                                    })),
                            )
                            .child(
                                icon_button(
                                    "right-panel-new-folder",
                                    "icons/folder-new.svg",
                                    theme,
                                )
                                .tooltip(|window, cx| {
                                    Tooltip::new(tr!("files.new_folder")).build(window, cx)
                                })
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        if let Some(root) =
                                            this.selected_workspace_path().map(Path::to_path_buf)
                                        {
                                            this.begin_inline_create(true, root, 0, window, cx);
                                        }
                                    },
                                )),
                            )
                            .child(
                                icon_button(
                                    "right-panel-refresh-files",
                                    "icons/rotate-cw.svg",
                                    theme,
                                )
                                .tooltip(|window, cx| {
                                    Tooltip::new(tr!("files.refresh")).build(window, cx)
                                })
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.refresh_right_panel_working_tree(cx);
                                    },
                                )),
                            )
                            .child({
                                let hidden = self.right_panel_show_hidden_files;
                                let focus =
                                    self.transcript_control_focus("right-panel-hidden-files", cx);
                                div()
                                    .id("right-panel-hidden-files")
                                    .track_focus(&focus)
                                    .tab_index(0)
                                    .size(px(26.0))
                                    .rounded(px(7.0))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .focus_visible(|style| {
                                        style.border_1().border_color(theme.accent)
                                    })
                                    .hover(|style| style.bg(theme.overlay))
                                    .child(icon(
                                        if hidden {
                                            "icons/eye.svg"
                                        } else {
                                            "icons/eye-off.svg"
                                        },
                                        12.0,
                                        theme.text_tertiary,
                                    ))
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(if hidden {
                                            tr!("files.hide_hidden")
                                        } else {
                                            tr!("files.show_hidden")
                                        })
                                        .build(window, cx)
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_right_panel_hidden_files(cx);
                                    }))
                                    .on_key_down(cx.listener(
                                        |this, event: &KeyDownEvent, _, cx| {
                                            if matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            ) {
                                                this.toggle_right_panel_hidden_files(cx);
                                                cx.stop_propagation();
                                            }
                                        },
                                    ))
                            }),
                    ),
            )
            .child({
                let empty_list = context_menu(
                    list,
                    SharedString::from("right-panel-working-tree-empty-context"),
                    &empty_menu,
                    move |_| {
                        let mut items = Vec::new();
                        if let Some(root) = root_path.clone() {
                            let root_for_file = root.clone();
                            let root_for_dir = root.clone();
                            let weak_file = weak.clone();
                            let weak_dir = weak.clone();
                            items.push(
                                MenuItem::new(tr!("files.new_file"), move |window, cx| {
                                    let _ = weak_file.update(cx, |this, cx| {
                                        this.begin_inline_create(
                                            false,
                                            root_for_file.clone(),
                                            0,
                                            window,
                                            cx,
                                        );
                                    });
                                })
                                .icon("icons/file-add.svg"),
                            );
                            items.push(
                                MenuItem::new(tr!("files.new_folder"), move |window, cx| {
                                    let _ = weak_dir.update(cx, |this, cx| {
                                        this.begin_inline_create(
                                            true,
                                            root_for_dir.clone(),
                                            0,
                                            window,
                                            cx,
                                        );
                                    });
                                })
                                .icon("icons/folder-new.svg"),
                            );
                        }
                        items
                    },
                );
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(empty_list)
                    .child(scrollbar::vertical(
                        &self.right_panel_files_scroll_handle,
                        &self.right_panel_files_scrollbar,
                    ))
            })
    }

    pub(crate) fn right_panel_working_tree_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entries = self.right_panel_working_tree.clone();
        if entries.is_empty() {
            return;
        }
        let current = self
            .right_panel_files_cursor
            .filter(|&index| index < entries.len())
            .unwrap_or(0);
        let key = event.keystroke.key.as_str();
        match key {
            "up" => {
                let target = current.saturating_sub(1);
                self.right_panel_files_cursor = Some(target);
                self.right_panel_files_scroll_handle.scroll_to_item(target);
                cx.notify();
                cx.stop_propagation();
            }
            "down" => {
                let target = (current + 1).min(entries.len() - 1);
                self.right_panel_files_cursor = Some(target);
                self.right_panel_files_scroll_handle.scroll_to_item(target);
                cx.notify();
                cx.stop_propagation();
            }
            "home" => {
                self.right_panel_files_cursor = Some(0);
                self.right_panel_files_scroll_handle.scroll_to_item(0);
                cx.notify();
                cx.stop_propagation();
            }
            "end" => {
                let target = entries.len() - 1;
                self.right_panel_files_cursor = Some(target);
                self.right_panel_files_scroll_handle.scroll_to_item(target);
                cx.notify();
                cx.stop_propagation();
            }
            "left" => {
                let entry = &entries[current];
                if entry.is_dir && entry.expanded {
                    self.right_panel_expanded_paths.remove(&entry.absolute_path);
                    self.refresh_right_panel_working_tree(cx);
                    cx.notify();
                } else if entry.depth > 0 {
                    if let Some(parent_idx) = entries[..current]
                        .iter()
                        .rposition(|candidate| candidate.is_dir && candidate.depth < entry.depth)
                    {
                        self.right_panel_files_cursor = Some(parent_idx);
                        self.right_panel_files_scroll_handle
                            .scroll_to_item(parent_idx);
                        cx.notify();
                    }
                }
                cx.stop_propagation();
            }
            "right" => {
                let entry = &entries[current];
                if entry.is_dir {
                    if !entry.expanded {
                        self.right_panel_expanded_paths
                            .insert(entry.absolute_path.clone());
                        self.refresh_right_panel_working_tree(cx);
                        cx.notify();
                    } else if current + 1 < entries.len()
                        && entries[current + 1].depth > entry.depth
                    {
                        let target = current + 1;
                        self.right_panel_files_cursor = Some(target);
                        self.right_panel_files_scroll_handle.scroll_to_item(target);
                        cx.notify();
                    }
                }
                cx.stop_propagation();
            }
            "enter" | "space" => {
                let entry = &entries[current];
                if entry.is_dir {
                    if !self.right_panel_expanded_paths.remove(&entry.absolute_path) {
                        self.right_panel_expanded_paths
                            .insert(entry.absolute_path.clone());
                    }
                    self.refresh_right_panel_working_tree(cx);
                    cx.notify();
                } else {
                    self.open_right_panel_file(entry.relative_path.clone(), cx);
                }
                cx.stop_propagation();
            }
            "f2" => {
                let entry = &entries[current];
                self.begin_inline_rename(entry.absolute_path.clone(), window, cx);
                cx.stop_propagation();
            }
            "delete" | "backspace" => {
                let entry = &entries[current];
                self.confirm_delete_path(entry.absolute_path.clone(), window, cx);
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    pub(crate) fn render_right_panel_file(
        &mut self,
        relative_path: String,
        panel_width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = Theme::current(cx);
        let file_tree_width = fitted_file_tree_width(panel_width, self.right_panel_file_tree_width);
        let (editor_state, writable, dirty) =
            self.ensure_right_panel_file_editor(&relative_path, window, cx);

        // Markdown files carry the global source/preview toggle; every other
        // language always shows source.
        let is_markdown = file_highlighter_language(&relative_path) == "markdown";
        let preview = is_markdown && self.state.markdown_preview;
        let body = if preview {
            self.render_file_markdown_preview(&relative_path, &editor_state, cx)
        } else {
            self.render_file_editor_body(
                &relative_path,
                &editor_state,
                panel_width - file_tree_width,
                writable,
                theme.surface,
                window,
                cx,
            )
        };
        let preview_toggle = is_markdown.then(|| {
            let focus = self.transcript_control_focus("file-markdown-preview-toggle", cx);
            let (icon_path, label) = if preview {
                ("icons/pencil.svg", tr!("files.edit_markdown_source"))
            } else {
                ("icons/eye.svg", tr!("files.preview_markdown"))
            };
            div()
                .id("file-markdown-preview-toggle")
                .track_focus(&focus)
                .tab_index(0)
                .size(px(26.0))
                .rounded(px(7.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .focus_visible(|style| style.border_1().border_color(theme.accent))
                .hover(|style| style.bg(theme.overlay))
                .child(icon(icon_path, 12.0, theme.text_tertiary))
                .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_markdown_preview(cx)))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        this.toggle_markdown_preview(cx);
                        cx.stop_propagation();
                    }
                }))
        });

        let segments: Vec<&str> = relative_path.split('/').collect();
        let file_name = segments.last().copied().unwrap_or(&relative_path);
        let dir_segments = &segments[..segments.len().saturating_sub(1)];

        let copy_focus = self.transcript_control_focus("right-panel-copy-path", cx);
        let copy_path = relative_path.clone();
        let copy_display_name = file_name.to_owned();
        let copy_button = div()
            .id("right-panel-copy-path")
            .track_focus(&copy_focus)
            .tab_index(0)
            .size(px(26.0))
            .rounded(px(7.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .focus_visible(|style| style.border_1().border_color(theme.accent))
            .hover(|style| style.bg(theme.overlay))
            .child(icon("icons/copy.svg", 12.0, theme.text_tertiary))
            .tooltip(|window, cx| Tooltip::new(tr!("files.copy_path")).build(window, cx))
            .on_click(cx.listener({
                let copy_display_name = copy_display_name.clone();
                move |this, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy_path.clone()));
                    this.show_success_toast(tr!(
                        "files.copied_path",
                        path = copy_display_name.clone()
                    ));
                }
            }))
            .on_key_down(cx.listener({
                let copy_path = relative_path.clone();
                let copy_display_name = copy_display_name.clone();
                move |this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy_path.clone()));
                        this.show_success_toast(tr!(
                            "files.copied_path",
                            path = copy_display_name.clone()
                        ));
                        cx.stop_propagation();
                    }
                }
            }));

        let breadcrumb_trail = div()
            .min_w_0()
            .flex_1()
            .flex()
            .items_center()
            .gap(px(4.0))
            .overflow_hidden()
            .children(dir_segments.iter().map(|segment| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .flex_none()
                    .child(
                        div()
                            .text_size(sp(12.5))
                            .text_color(theme.text_tertiary)
                            .child(segment.to_string()),
                    )
                    .child(
                        div()
                            .text_size(sp(12.0))
                            .text_color(theme.text_ghost)
                            .child("/"),
                    )
            }))
            .child(
                div()
                    .truncate()
                    .text_size(sp(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(file_name.to_string()),
            )
            .when(dirty, |trail| {
                trail.child(
                    div()
                        .id("right-panel-file-breadcrumb-dirty")
                        .size(px(6.0))
                        .rounded_full()
                        .bg(theme.warning)
                        .flex_none()
                        .tooltip(|window, cx| {
                            Tooltip::new(tr!(
                                "files.unsaved_changes",
                                shortcut = crate::platform::primary_shortcut("⌘S", "Ctrl+S")
                            ))
                            .build(window, cx)
                        }),
                )
            });

        let editor = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(42.0))
                    .flex_none()
                    .px(px(14.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(file_icon(file_icon_for_path(&relative_path), 14.0))
                    .child(breadcrumb_trail)
                    .child(copy_button)
                    .children(preview_toggle),
            )
            .child(body);

        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .child(editor)
            .child(
                div()
                    .w(px(file_tree_width))
                    .min_w(px(FILE_TREE_MIN_WIDTH))
                    .h_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .relative()
                    .border_l_1()
                    .border_color(theme.border_strong)
                    .child(self.render_right_panel_working_tree(Some(&relative_path), cx))
                    .child(self.render_panel_resize_handle(
                        "right-panel-file-tree-resize-handle",
                        PanelResizeTarget::FileTree,
                        cx,
                    )),
            )
    }

    pub(crate) fn ensure_right_panel_file_editor(
        &mut self,
        relative_path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<TextInput>, bool, bool) {
        if let Some(editor) = self.right_panel_file_editors.get(relative_path) {
            return (editor.state.clone(), editor.writable, editor.dirty);
        }

        // Reached from `render`, so the file cannot be read here. The editor
        // starts empty and locked, and `read_right_panel_file_into_editor`
        // fills it in from the background executor a frame or two later.
        let language = file_highlighter_language(relative_path);
        let state = cx.new(|cx| {
            TextInput::new(window, cx)
                .multi_line()
                .syntax(Some(language))
                .read_only(true)
        });

        self.right_panel_file_editors.insert(
            relative_path.to_owned(),
            RightPanelFileEditor {
                state: state.clone(),
                disk_content: String::new(),
                writable: false,
                dirty: false,
                reading: false,
                read_epoch: 0,
            },
        );

        // Dirty tracking follows content edits. Observing raw notifies would
        // also fire for caret blinks and selection drags, cloning the whole
        // file's text for each one.
        let subscribed_path = relative_path.to_owned();
        cx.subscribe(
            &state,
            move |this: &mut Self, state, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Edited) {
                    return;
                }
                let value = state.read(cx).content().to_owned();
                if let Some(editor) = this
                    .right_panel_file_editors
                    .get_mut(subscribed_path.as_str())
                {
                    let dirty = editor.writable && value != editor.disk_content;
                    if editor.dirty != dirty {
                        editor.dirty = dirty;
                        cx.notify();
                    }
                }
                // Any content change — typing, a replace, a reload from disk —
                // moves the text out from under an open find's match list.
                this.refresh_file_search_for_edit(subscribed_path.as_str(), cx);
            },
        )
        .detach();

        let focused_path = relative_path.to_owned();
        cx.subscribe(&state, move |this: &mut Self, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Focus) {
                this.reload_right_panel_file_if_clean(focused_path.as_str(), cx);
            }
        })
        .detach();

        self.read_right_panel_file_into_editor(relative_path.to_owned(), cx);
        (state, false, false)
    }

    /// Reads a file into its editor off the UI thread.
    ///
    /// One `read_to_string` of an arbitrarily large file — hundreds of frames
    /// for a big one — so it never runs in a frame. The editor keeps whatever
    /// it is already showing until the read lands.
    ///
    /// The result is applied only if the same session is still selected and the
    /// editor is still the one that asked, so a read started before a project
    /// or session switch cannot write another workspace's text into the view.
    pub(crate) fn read_right_panel_file_into_editor(
        &mut self,
        relative_path: String,
        cx: &mut Context<Self>,
    ) {
        let project_path = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf);
        let (Some(project_path), Some(session_id)) = (project_path, self.state.selected_session)
        else {
            // Nothing to read from. Say so in the editor rather than leaving it
            // looking like an empty file.
            if let Some(editor) = self.right_panel_file_editors.get_mut(&relative_path) {
                editor.reading = false;
                editor.disk_content = tr!("files.no_project_is_open");
                editor.writable = false;
                let state = editor.state.clone();
                let content = editor.disk_content.clone();
                state.update(cx, |state, cx| state.set_content(content, cx));
            }
            return;
        };
        let Some(editor) = self.right_panel_file_editors.get_mut(&relative_path) else {
            return;
        };
        // A second asker would only duplicate the read and race to apply it.
        if editor.reading {
            return;
        }
        editor.reading = true;
        editor.read_epoch += 1;
        let epoch = editor.read_epoch;
        let workspace = padu_client::WorkspaceClient::new(self.daemon.client());

        cx.spawn(async move |padu, cx| {
            let read = cx
                .background_executor()
                .spawn({
                    let project_path = project_path.clone();
                    let relative_path = relative_path.clone();
                    async move { read_right_panel_file(&workspace, &project_path, &relative_path) }
                })
                .await;
            padu.update(cx, |padu, cx| {
                if padu.state.selected_session != Some(session_id)
                    || padu
                        .selected_workspace_path()
                        .is_none_or(|path| path != project_path)
                {
                    // The editor moved into another session's stored state, or
                    // the project changed. Clear the flag so a later reload can
                    // ask again, and drop the text.
                    if let Some(editor) = padu.right_panel_file_editors.get_mut(&relative_path) {
                        editor.reading = false;
                    }
                    return;
                }
                let (content, writable) = read;
                let Some(editor) = padu.right_panel_file_editors.get_mut(&relative_path) else {
                    return;
                };
                // A save landed while the read was in flight, so this text
                // describes the file as it was before that save.
                if editor.read_epoch != epoch {
                    return;
                }
                editor.reading = false;
                // An edit landed while the read was in flight; the user's text
                // wins over the copy on disk.
                if editor.dirty {
                    return;
                }
                if editor.disk_content == content && editor.writable == writable {
                    return;
                }
                editor.disk_content = content.clone();
                editor.writable = writable;
                editor.dirty = false;
                let state = editor.state.clone();
                state.update(cx, |state, cx| {
                    state.set_read_only(!writable);
                    state.set_content(content, cx);
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The editor body: a line-number gutter beside soft-wrapped text.
    ///
    /// The gutter is *painted*, not laid out — one canvas that shapes only the
    /// numbers currently on screen, the way Zed's editor element does. A div per
    /// line would put one layout node per line of the file in every frame, which
    /// is what made large files crawl.
    ///
    /// Row heights come from the text's measured layout rather than a nominal
    /// line height, so a soft-wrapped line still gets exactly one number and the
    /// two columns cannot drift apart down a long file.
    pub(crate) fn render_file_editor_body(
        &mut self,
        relative_path: &str,
        editor_state: &Entity<TextInput>,
        pane_width: f32,
        writable: bool,
        background: Hsla,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        const GUTTER_PAD_RIGHT: f32 = 8.0;
        const CONTENT_PAD_TOP: f32 = 6.0;

        let text_size = self.state.code_font_size;
        let line_height = (text_size * 1.5).round();

        // An open find bar follows whichever file this body is showing; a
        // cheap comparison every frame, one recompute on the frame after the
        // visible file actually changes.
        self.sync_file_search_target(relative_path, cx);
        let find_bar = self.render_file_search_bar(pane_width, writable, window, cx);

        let theme = Theme::current(cx);
        let field = editor_state.read(cx);
        let line_count = field.content().split('\n').count().max(1);
        let heights = field.wrapped_line_heights();
        // A mono digit advances ~0.6em, so the gutter tracks the font size.
        let digit_width = (text_size * 0.6).ceil();
        let gutter_width = 20.0 + digit_width * (line_count.to_string().len() as f32);
        let content_height = if heights.is_empty() {
            px(line_height) * line_count as f32
        } else {
            heights.iter().fold(Pixels::ZERO, |total, h| total + *h)
        };

        let viewport = self.right_panel_editor_scroll_handle.clone();
        let number_color = theme.text_ghost;
        let gutter = canvas(
            |_, _, _| (),
            move |bounds: gpui::Bounds<Pixels>, _, window: &mut Window, cx: &mut App| {
                let visible = viewport.bounds();
                let mut y = bounds.origin.y;
                for number in 1..=line_count {
                    let height = heights
                        .get(number - 1)
                        .copied()
                        .unwrap_or_else(|| px(line_height));
                    // Everything below the viewport is unreachable from here on.
                    if y > visible.bottom() {
                        break;
                    }
                    if y + height >= visible.top() {
                        let text = SharedString::from(number.to_string());
                        let run = gpui::TextRun {
                            len: text.len(),
                            font: gpui::font(md::render::MONO_FAMILY),
                            color: number_color,
                            ..Default::default()
                        };
                        let line =
                            window
                                .text_system()
                                .shape_line(text, px(text_size), &[run], None);
                        let origin = point(bounds.right() - line.width, y);
                        let _ = line.paint(
                            origin,
                            px(line_height),
                            gpui::TextAlign::Left,
                            None,
                            window,
                            cx,
                        );
                    }
                    y += height;
                }
            },
        )
        .flex_none()
        .w(px(gutter_width - GUTTER_PAD_RIGHT))
        .h(content_height);

        // The find bar sits in normal flow above the scroll region — Zed's
        // buffer-search arrangement — so an open bar pushes the content and
        // its line-number gutter down instead of covering the first lines.
        div()
            .key_context("FileEditorPane")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .bg(background)
            .font_family(md::render::MONO_FAMILY)
            .text_size(px(text_size))
            .line_height(px(line_height))
            .children(find_bar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(
                        div()
                            .id(SharedString::from(format!("file-editor-{relative_path}")))
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.right_panel_editor_scroll_handle)
                            .child(
                                div()
                                    .w_full()
                                    .pt(px(CONTENT_PAD_TOP))
                                    .pb(px(CONTENT_PAD_TOP))
                                    .flex()
                                    .items_start()
                                    .child(gutter)
                                    .child(div().w(px(GUTTER_PAD_RIGHT)).flex_none())
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .pr(px(10.0))
                                            .child(editor_state.clone()),
                                    ),
                            ),
                    )
                    .child(scrollbar::vertical(
                        &self.right_panel_editor_scroll_handle,
                        &self.right_panel_editor_scrollbar,
                    )),
            )
    }

    /// Flips the global markdown source/preview mode and persists it, so the
    /// choice follows the user across files and sessions.
    pub(crate) fn toggle_markdown_preview(&mut self, cx: &mut Context<Self>) {
        self.state.markdown_preview = !self.state.markdown_preview;
        self.save();
        cx.notify();
    }

    /// The rendered-markdown alternative to the editor body, shown while the
    /// global preview toggle is on. It renders the editor's current text —
    /// unsaved edits included — with the transcript's markdown engine; the
    /// parse is cached per path, so re-rendering an unchanged document costs
    /// `Rc` clones, not a re-parse. Reads only in-memory editor state: the
    /// render path may not touch the filesystem.
    pub(crate) fn render_file_markdown_preview(
        &mut self,
        relative_path: &str,
        editor_state: &Entity<TextInput>,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = Theme::current(cx);
        let palette = MarkdownPalette::from_theme(&theme);
        let mut cache = self.file_preview_markdown.borrow_mut();
        if !matches!(cache.as_ref(), Some((cached, _)) if cached == relative_path) {
            *cache = Some((relative_path.to_owned(), MarkdownView::new()));
        }
        let (_, view) = cache.as_mut().expect("entry ensured above");
        view.set_text(editor_state.read(cx).content(), false);
        let ctx = MarkdownCtx::new(
            format!("file-preview-{relative_path}"),
            &palette,
            MarkdownMetrics::document(self.state.ui_font_size, self.state.code_font_size),
            self.file_preview_selection.clone(),
        )
        .with_link_handler(self.markdown_link_handler.clone());
        let document = md::render::markdown(view, &ctx);

        let selection_input = {
            let selection = self.file_preview_selection.clone();
            canvas(
                |_, _, _| (),
                move |_, _, window, _| md::render::install_selection_input(window, &selection),
            )
            .absolute()
            .w(px(0.0))
            .h(px(0.0))
        };

        div()
            .flex_1()
            .min_h_0()
            .relative()
            .bg(theme.surface)
            .child(
                div()
                    .id(SharedString::from(format!("file-preview-{relative_path}")))
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.file_preview_scroll_handle)
                    // Painted before the document, so the frame's selection
                    // registry holds exactly this frame's text elements.
                    .child(md::render::frame_reset(self.file_preview_selection.clone()))
                    .child(
                        div()
                            .px(px(16.0))
                            .pt(px(14.0))
                            .pb(px(24.0))
                            .text_color(theme.text)
                            .children(document),
                    ),
            )
            .child(selection_input)
            .child(scrollbar::vertical(
                &self.file_preview_scroll_handle,
                &self.file_preview_scrollbar,
            ))
    }

    /// Picks up an external edit to a file the user has not modified here.
    ///
    /// Reaches the filesystem, so it queues a background read rather than
    /// blocking; the editor keeps showing its current text until that lands.
    pub(crate) fn reload_right_panel_file_if_clean(
        &mut self,
        relative_path: &str,
        cx: &mut Context<Self>,
    ) {
        if self
            .right_panel_file_editors
            .get(relative_path)
            .is_none_or(|editor| editor.dirty)
        {
            return;
        }
        self.read_right_panel_file_into_editor(relative_path.to_owned(), cx);
    }

    pub(crate) fn reload_clean_right_panel_file_editors(&mut self, cx: &mut Context<Self>) {
        let paths = self
            .right_panel_file_editors
            .iter()
            .filter(|(_, editor)| !editor.dirty)
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();
        for path in paths {
            self.reload_right_panel_file_if_clean(&path, cx);
        }
    }

    pub(crate) fn save_right_panel_file_action(
        &mut self,
        _: &SaveFile,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(relative_path) = self.visible_right_panel_file_path() else {
            return;
        };
        let Some(project_path) = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf)
        else {
            return;
        };
        let Some(editor) = self.right_panel_file_editors.get(&relative_path) else {
            return;
        };
        if !editor.writable {
            self.show_toast(if editor.reading {
                tr!("files.could_not_save_opening", path = relative_path)
            } else {
                tr!("files.could_not_save_read_only", path = relative_path)
            });
            cx.notify();
            return;
        }

        let content = editor.state.read(cx).content().to_owned();
        let Some(session_id) = self.state.selected_session else {
            return;
        };
        let epoch = if let Some(editor) = self.right_panel_file_editors.get_mut(&relative_path) {
            editor.reading = false;
            editor.read_epoch += 1;
            editor.read_epoch
        } else {
            return;
        };
        let workspace = padu_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn({
                    let project_path = project_path.clone();
                    let relative_path = relative_path.clone();
                    let content = content.clone();
                    async move {
                        match workspace.request(padu_client::WorkspaceOperation::WriteTextFile {
                            root: project_path,
                            relative_path: PathBuf::from(relative_path),
                            content,
                        })? {
                            padu_client::WorkspaceResult::Ack => Ok(()),
                            _ => anyhow::bail!("the daemon returned an invalid file response"),
                        }
                    }
                })
                .await;
            let _ = padu.update(cx, |padu, cx| {
                if padu.state.selected_session != Some(session_id)
                    || padu
                        .selected_workspace_path()
                        .is_none_or(|path| path != project_path)
                {
                    return;
                }
                match result {
                    Ok(()) => {
                        if let Some(editor) = padu.right_panel_file_editors.get_mut(&relative_path)
                            && editor.read_epoch == epoch
                        {
                            let current = editor.state.read(cx).content();
                            editor.disk_content = content.clone();
                            editor.dirty = current != content;
                        }
                    }
                    Err(error) => padu.show_toast(tr!(
                        "files.could_not_save",
                        path = relative_path,
                        error = error.to_string()
                    )),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Re-reads whichever workspace surface is on screen.
    pub(crate) fn refresh_workspace_surfaces(&mut self, cx: &mut Context<Self>) {
        match self.active_right_panel_surface() {
            Some(RightPanelSurface::Diff) => self.refresh_right_panel_diff(cx),
            Some(RightPanelSurface::Files | RightPanelSurface::File(_)) => {
                self.refresh_right_panel_working_tree(cx)
            }
            _ => {}
        }
    }

    fn render_inline_create_row(
        &self,
        is_dir: bool,
        depth: usize,
        input: &Entity<TextInput>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = Theme::current(cx);
        div()
            .id("right-panel-inline-create-row")
            .key_context(INLINE_FILE_PARENT_CONTEXT)
            .on_action(cx.listener(|this, _: &SubmitInlineFile, window, cx| {
                this.submit_inline_file_operation(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CancelInlineFile, window, cx| {
                this.cancel_inline_file_operation(window, cx);
            }))
            .on_action(cx.listener(|this, _: &crate::input::Clear, window, cx| {
                this.cancel_inline_file_operation(window, cx);
            }))
            .h(px(30.0))
            .min_h(px(30.0))
            .flex_none()
            .mx(px(8.0))
            .pl(px(8.0 + depth as f32 * 16.0))
            .pr(px(8.0))
            .rounded(px(6.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .bg(theme.overlay)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "enter" => {
                        this.submit_inline_file_operation(window, cx);
                        cx.stop_propagation();
                    }
                    "escape" => {
                        this.cancel_inline_file_operation(window, cx);
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .child(div().w(px(10.0)).h(px(10.0)).flex_none())
            .child(if is_dir {
                icon("icons/folder-new.svg", 14.0, theme.text_tertiary).into_any_element()
            } else {
                icon("icons/file-add.svg", 14.0, theme.text_tertiary).into_any_element()
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(24.0))
                    .px(px(6.0))
                    .rounded(px(5.0))
                    .border_1()
                    .border_color(theme.accent)
                    .bg(theme.surface)
                    .flex()
                    .items_center()
                    .child(input.clone()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .flex_none()
                    .child(
                        div()
                            .id("inline-confirm-btn")
                            .size(px(20.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|e| e.bg(theme.overlay_strong))
                            .child(icon("icons/check.svg", 12.0, theme.accent))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.submit_inline_file_operation(window, cx);
                            })),
                    )
                    .child(
                        div()
                            .id("inline-cancel-btn")
                            .size(px(20.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|e| e.bg(theme.overlay_strong))
                            .child(icon("icons/x.svg", 12.0, theme.text_tertiary))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.cancel_inline_file_operation(window, cx);
                            })),
                    ),
            )
    }

    fn render_inline_rename_widget(
        &self,
        input: &Entity<TextInput>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        div()
            .key_context(INLINE_FILE_PARENT_CONTEXT)
            .on_action(cx.listener(|this, _: &SubmitInlineFile, window, cx| {
                this.submit_inline_file_operation(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CancelInlineFile, window, cx| {
                this.cancel_inline_file_operation(window, cx);
            }))
            .on_action(cx.listener(|this, _: &crate::input::Clear, window, cx| {
                this.cancel_inline_file_operation(window, cx);
            }))
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(4.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "enter" => {
                        this.submit_inline_file_operation(window, cx);
                        cx.stop_propagation();
                    }
                    "escape" => {
                        this.cancel_inline_file_operation(window, cx);
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(24.0))
                    .px(px(6.0))
                    .rounded(px(5.0))
                    .border_1()
                    .border_color(theme.accent)
                    .bg(theme.surface)
                    .flex()
                    .items_center()
                    .child(input.clone()),
            )
            .child(
                div()
                    .id("inline-rename-confirm-btn")
                    .size(px(20.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|e| e.bg(theme.overlay_strong))
                    .child(icon("icons/check.svg", 12.0, theme.accent))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.submit_inline_file_operation(window, cx);
                    })),
            )
            .child(
                div()
                    .id("inline-rename-cancel-btn")
                    .size(px(20.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|e| e.bg(theme.overlay_strong))
                    .child(icon("icons/x.svg", 12.0, theme.text_tertiary))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.cancel_inline_file_operation(window, cx);
                    })),
            )
            .into_any_element()
    }

    pub(crate) fn begin_inline_create(
        &mut self,
        is_dir: bool,
        parent: PathBuf,
        depth: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.right_panel_expanded_paths.contains(&parent) {
            self.right_panel_expanded_paths.insert(parent.clone());
            self.refresh_right_panel_working_tree(cx);
        }
        let input = cx.new(|cx| {
            TextInput::new(window, cx).placeholder(if is_dir {
                tr!("files.new_folder")
            } else {
                tr!("files.name_placeholder")
            })
        });
        cx.subscribe(&input, |this: &mut Self, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Submit(_)) {
                this.commit_inline_file_operation(cx);
            }
        })
        .detach();
        let focus = input.read(cx).focus();
        let kind = if is_dir {
            InlineFileOperationKind::CreateDirectory { parent, depth }
        } else {
            InlineFileOperationKind::CreateFile { parent, depth }
        };
        self.right_panel_inline_file_operation = Some(InlineFileOperation { kind, input });
        window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        cx.notify();
    }

    pub(crate) fn begin_inline_rename(
        &mut self,
        source: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        let is_file = source.is_file();
        let stem_len = if is_file {
            source
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(|s| s.len())
                .unwrap_or(name.len())
        } else {
            name.len()
        };
        let input = cx.new(|cx| TextInput::new(window, cx));
        input.update(cx, |input, cx| {
            input.set_content(name.clone(), cx);
            if stem_len > 0 {
                input.select_range(0..stem_len, cx);
            } else {
                input.select_all_text(cx);
            }
        });
        cx.subscribe(&input, |this: &mut Self, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Submit(_)) {
                this.commit_inline_file_operation(cx);
            }
        })
        .detach();
        let focus = input.read(cx).focus();
        self.right_panel_inline_file_operation = Some(InlineFileOperation {
            kind: InlineFileOperationKind::Rename { source },
            input,
        });
        window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        cx.notify();
    }

    pub(crate) fn cancel_inline_file_operation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.right_panel_inline_file_operation.take().is_some() {
            let focus = self.transcript_control_focus("right-panel-working-tree", cx);
            window.focus(&focus, cx);
            cx.notify();
        }
    }

    pub(crate) fn submit_inline_file_operation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.commit_inline_file_operation(cx) {
            let focus = self.transcript_control_focus("right-panel-working-tree", cx);
            window.focus(&focus, cx);
        }
    }

    pub(crate) fn commit_inline_file_operation(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(op) = self.right_panel_inline_file_operation.take() else {
            return false;
        };
        let name = op.input.read(cx).content().trim().to_owned();
        if name.is_empty() {
            cx.notify();
            return true;
        }
        if Path::new(&name).components().count() != 1 || name.contains('/') || name.contains('\\') {
            self.show_toast(tr!("files.invalid_name"));
            self.right_panel_inline_file_operation = Some(op);
            cx.notify();
            return false;
        }
        let Some(root) = self.selected_workspace_path().map(Path::to_path_buf) else {
            return true;
        };
        let (operation, is_create_file, created_rel) = match op.kind {
            InlineFileOperationKind::CreateFile { parent, .. } => {
                let Ok(parent_rel) = parent.strip_prefix(&root) else {
                    return true;
                };
                let relative_path = parent_rel.join(&name);
                let rel_str = relative_path.to_string_lossy().into_owned();
                (
                    padu_client::WorkspaceOperation::CreateFile {
                        root: root.clone(),
                        relative_path,
                    },
                    true,
                    Some(rel_str),
                )
            }
            InlineFileOperationKind::CreateDirectory { parent, .. } => {
                let Ok(parent_rel) = parent.strip_prefix(&root) else {
                    return true;
                };
                (
                    padu_client::WorkspaceOperation::CreateDirectory {
                        root: root.clone(),
                        relative_path: parent_rel.join(&name),
                    },
                    false,
                    None,
                )
            }
            InlineFileOperationKind::Rename { source } => {
                let current_name = source.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if current_name == name {
                    cx.notify();
                    return true;
                }
                let Ok(source_relative) = source.strip_prefix(&root) else {
                    return true;
                };
                let Some(parent) = source_relative.parent() else {
                    return true;
                };
                let new_relative = parent.join(&name);
                if let Some(selected) = self.right_panel_files_selected_path.as_ref() {
                    if selected == &source_relative.to_string_lossy() {
                        self.right_panel_files_selected_path =
                            Some(new_relative.to_string_lossy().into_owned());
                    }
                }
                (
                    padu_client::WorkspaceOperation::RenamePath {
                        root: root.clone(),
                        from: source_relative.to_path_buf(),
                        to: new_relative,
                    },
                    false,
                    None,
                )
            }
        };
        let workspace = padu_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { workspace.request(operation) })
                .await;
            let _ = padu.update(cx, |padu, cx| {
                match result {
                    Ok(padu_client::WorkspaceResult::Ack) => {
                        padu.refresh_right_panel_working_tree(cx);
                        if is_create_file {
                            if let Some(rel) = created_rel {
                                padu.open_right_panel_file(rel, cx);
                            }
                        }
                    }
                    Ok(_) | Err(_) => padu.show_toast(tr!("files.operation_failed")),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
        true
    }

    pub(crate) fn execute_delete_path(&mut self, target: PathBuf, cx: &mut Context<Self>) {
        let Some(root) = self.selected_workspace_path().map(Path::to_path_buf) else {
            return;
        };
        let Ok(relative_path) = target.strip_prefix(&root) else {
            return;
        };
        let rel_str = relative_path.to_string_lossy().into_owned();
        if self.right_panel_files_selected_path.as_deref() == Some(&rel_str) {
            self.right_panel_files_selected_path = None;
        }
        let operation = padu_client::WorkspaceOperation::DeletePath {
            root,
            relative_path: relative_path.to_path_buf(),
        };
        let workspace = padu_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |padu, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { workspace.request(operation) })
                .await;
            let _ = padu.update(cx, |padu, cx| {
                match result {
                    Ok(padu_client::WorkspaceResult::Ack) => {
                        padu.refresh_right_panel_working_tree(cx);
                    }
                    Ok(_) | Err(_) => padu.show_toast(tr!("files.operation_failed")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn toggle_right_panel_hidden_files(&mut self, cx: &mut Context<Self>) {
        self.right_panel_show_hidden_files = !self.right_panel_show_hidden_files;
        self.refresh_right_panel_working_tree(cx);
        cx.notify();
    }

    /// Re-walks the project's working tree.
    ///
    /// `read_dir` plus a `stat` per entry, recursively over expanded
    /// directories — filesystem I/O, so it runs on the background executor and
    /// the panel keeps drawing the previous listing until the result lands.
    /// Called when the tree's inputs change, never from a frame.
    pub(crate) fn refresh_right_panel_working_tree(&mut self, cx: &mut Context<Self>) {
        let Some(project_path) = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf)
        else {
            self.right_panel_working_tree.clear();
            return;
        };
        // The tree on disk moves under us, and the expanded set may just have
        // changed, so a cached listing is only good until something asks again.
        self.working_trees.invalidate(&project_path);
        match self.working_trees.read(&project_path) {
            Query::Ready(entries) => self.right_panel_working_tree = (*entries).clone(),
            Query::Pending => {}
            Query::Missing(token) => {
                let expanded = self.right_panel_expanded_paths.clone();
                let show_hidden = self.right_panel_show_hidden_files;
                let workspace = padu_client::WorkspaceClient::new(self.daemon.client());
                cx.spawn(async move |padu, cx| {
                    let entries = cx
                        .background_executor()
                        .spawn({
                            let path = project_path.clone();
                            async move {
                                match workspace.request(padu_client::WorkspaceOperation::ListTree {
                                    root: path,
                                    expanded_paths: expanded.into_iter().collect(),
                                    show_hidden,
                                }) {
                                    Ok(padu_client::WorkspaceResult::WorkingTree { entries }) => {
                                        entries
                                            .into_iter()
                                            .map(|entry| WorkingTreeEntry {
                                                file_icon: (!entry.is_dir)
                                                    .then(|| file_icon_for_name(&entry.name)),
                                                relative_path: entry.relative_path,
                                                absolute_path: entry.absolute_path,
                                                name: entry.name,
                                                is_dir: entry.is_dir,
                                                is_ignored: entry.is_ignored,
                                                expanded: entry.expanded,
                                                depth: entry.depth,
                                            })
                                            .collect()
                                    }
                                    Ok(_) | Err(_) => Vec::new(),
                                }
                            }
                        })
                        .await;
                    padu.update(cx, |padu, cx| {
                        if padu.working_trees.fulfill(token, entries.clone())
                            && padu
                                .selected_workspace_path()
                                .is_some_and(|path| path == project_path)
                        {
                            padu.right_panel_working_tree = entries;
                            cx.notify();
                        }
                    })
                    .ok();
                })
                .detach();
            }
        }
    }
}
