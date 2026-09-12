use super::*;

impl Padu {
    pub(super) fn render_keybindings_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);

        let sections: &[(&str, &[(&str, &str, &str)])] = &[
            (
                "keybindings.section_general",
                &[
                    (
                        "keybindings.new_task",
                        "keybindings.new_task_desc",
                        crate::platform::primary_shortcut("⌘N", "Ctrl+N"),
                    ),
                    (
                        "keybindings.open_project",
                        "keybindings.open_project_desc",
                        crate::platform::primary_shortcut("⌘O", "Ctrl+O"),
                    ),
                    (
                        "keybindings.command_palette",
                        "keybindings.command_palette_desc",
                        crate::platform::primary_shortcut("⌘K", "Ctrl+K"),
                    ),
                    (
                        "keybindings.open_settings",
                        "keybindings.open_settings_desc",
                        crate::platform::primary_shortcut("⌘,", "Ctrl+,"),
                    ),
                    (
                        "keybindings.close_window",
                        "keybindings.close_window_desc",
                        crate::platform::primary_shortcut("⌘W", "Ctrl+W"),
                    ),
                    (
                        "keybindings.quit",
                        "keybindings.quit_desc",
                        crate::platform::primary_shortcut("⌘Q", "Ctrl+Q"),
                    ),
                ],
            ),
            (
                "keybindings.section_navigation",
                &[
                    (
                        "keybindings.toggle_sidebar",
                        "keybindings.toggle_sidebar_desc",
                        crate::platform::primary_shortcut("⌘B", "Ctrl+B"),
                    ),
                    (
                        "keybindings.toggle_right_panel",
                        "keybindings.toggle_right_panel_desc",
                        crate::platform::primary_shortcut("⇧⌘B", "Ctrl+Shift+B"),
                    ),
                    (
                        "keybindings.open_browser",
                        "keybindings.open_browser_desc",
                        crate::platform::primary_shortcut("⌥⌘B", "Ctrl+Alt+B"),
                    ),
                    (
                        "keybindings.open_terminal",
                        "keybindings.open_terminal_desc",
                        crate::platform::primary_shortcut("⌘T", "Ctrl+T"),
                    ),
                    (
                        "keybindings.open_files",
                        "keybindings.open_files_desc",
                        crate::platform::primary_shortcut("⇧⌘E", "Ctrl+Shift+E"),
                    ),
                    (
                        "keybindings.open_review",
                        "keybindings.open_review_desc",
                        crate::platform::primary_shortcut("⌘D", "Ctrl+D"),
                    ),
                    (
                        "keybindings.toggle_usage_panel",
                        "keybindings.toggle_usage_panel_desc",
                        crate::platform::primary_shortcut("⌘U", "Ctrl+U"),
                    ),
                    (
                        "keybindings.toggle_fps",
                        "keybindings.toggle_fps_desc",
                        crate::platform::primary_shortcut("⌥⇧⌘F", "Ctrl+Alt+Shift+F"),
                    ),
                    (
                        "keybindings.navigate_back",
                        "keybindings.navigate_back_desc",
                        crate::platform::primary_shortcut("⌘[", "Ctrl+["),
                    ),
                    (
                        "keybindings.navigate_forward",
                        "keybindings.navigate_forward_desc",
                        crate::platform::primary_shortcut("⌘]", "Ctrl+]"),
                    ),
                    (
                        "keybindings.previous_session",
                        "keybindings.previous_session_desc",
                        crate::platform::primary_shortcut("⌥⌘↑", "Ctrl+Alt+Up"),
                    ),
                    (
                        "keybindings.next_session",
                        "keybindings.next_session_desc",
                        crate::platform::primary_shortcut("⌥⌘↓", "Ctrl+Alt+Down"),
                    ),
                    (
                        "keybindings.next_task",
                        "keybindings.next_task_desc",
                        "Ctrl+Tab",
                    ),
                    (
                        "keybindings.prev_task",
                        "keybindings.prev_task_desc",
                        "Ctrl+Shift+Tab",
                    ),
                ],
            ),
            (
                "keybindings.section_chat",
                &[
                    (
                        "keybindings.focus_composer",
                        "keybindings.focus_composer_desc",
                        crate::platform::primary_shortcut("⌘L", "Ctrl+L"),
                    ),
                    (
                        "keybindings.select_model",
                        "keybindings.select_model_desc",
                        crate::platform::primary_shortcut("⌘/", "Ctrl+/"),
                    ),
                    (
                        "keybindings.cancel_turn",
                        "keybindings.cancel_turn_desc",
                        "Esc",
                    ),
                ],
            ),
            (
                "keybindings.section_editor",
                &[
                    (
                        "keybindings.save_file",
                        "keybindings.save_file_desc",
                        crate::platform::primary_shortcut("⌘S", "Ctrl+S"),
                    ),
                    (
                        "keybindings.find",
                        "keybindings.find_desc",
                        crate::platform::primary_shortcut("⌘F", "Ctrl+F"),
                    ),
                    (
                        "keybindings.replace",
                        "keybindings.replace_desc",
                        crate::platform::primary_shortcut("⌥⌘F", "Ctrl+Alt+F"),
                    ),
                    (
                        "keybindings.find_next",
                        "keybindings.find_next_desc",
                        crate::platform::primary_shortcut("⌘G", "Ctrl+G"),
                    ),
                    (
                        "keybindings.find_prev",
                        "keybindings.find_prev_desc",
                        crate::platform::primary_shortcut("⇧⌘G", "Ctrl+Shift+G"),
                    ),
                ],
            ),
            (
                "keybindings.section_browser",
                &[
                    (
                        "keybindings.browser_focus",
                        "keybindings.browser_focus_desc",
                        crate::platform::primary_shortcut("⌘L", "Ctrl+L"),
                    ),
                    (
                        "keybindings.browser_reload",
                        "keybindings.browser_reload_desc",
                        crate::platform::primary_shortcut("⌘R", "Ctrl+R"),
                    ),
                    (
                        "keybindings.browser_hard_reload",
                        "keybindings.browser_hard_reload_desc",
                        crate::platform::primary_shortcut("⇧⌘R", "Ctrl+Shift+R"),
                    ),
                    (
                        "keybindings.browser_back",
                        "keybindings.browser_back_desc",
                        crate::platform::primary_shortcut("⌘[", "Ctrl+["),
                    ),
                    (
                        "keybindings.browser_forward",
                        "keybindings.browser_forward_desc",
                        crate::platform::primary_shortcut("⌘]", "Ctrl+]"),
                    ),
                ],
            ),
            (
                "keybindings.section_files",
                &[
                    (
                        "keybindings.files_find",
                        "keybindings.files_find_desc",
                        crate::platform::primary_shortcut("⌘P", "Ctrl+P"),
                    ),
                    (
                        "keybindings.files_new_file",
                        "keybindings.files_new_file_desc",
                        "N",
                    ),
                    (
                        "keybindings.files_new_folder",
                        "keybindings.files_new_folder_desc",
                        "⇧N",
                    ),
                    (
                        "keybindings.files_refresh",
                        "keybindings.files_refresh_desc",
                        "R",
                    ),
                    (
                        "keybindings.files_hidden",
                        "keybindings.files_hidden_desc",
                        "H",
                    ),
                ],
            ),
            (
                "keybindings.section_review",
                &[
                    (
                        "keybindings.refresh_review",
                        "keybindings.refresh_review_desc",
                        crate::platform::primary_shortcut("⌘R", "Ctrl+R"),
                    ),
                    (
                        "keybindings.toggle_review_layout",
                        "keybindings.toggle_review_layout_desc",
                        crate::platform::primary_shortcut("⇧⌘T", "Ctrl+Shift+T"),
                    ),
                    (
                        "keybindings.toggle_review_files",
                        "keybindings.toggle_review_files_desc",
                        crate::platform::primary_shortcut("⌘\\", "Ctrl+\\"),
                    ),
                    (
                        "keybindings.toggle_review_collapse",
                        "keybindings.toggle_review_collapse_desc",
                        crate::platform::primary_shortcut("⇧⌘C", "Ctrl+Shift+C"),
                    ),
                    (
                        "keybindings.toggle_review_file",
                        "keybindings.toggle_review_file_desc",
                        crate::platform::primary_shortcut("⇧⌘K", "Ctrl+Shift+K"),
                    ),
                    (
                        "keybindings.toggle_review_tree",
                        "keybindings.toggle_review_tree_desc",
                        crate::platform::primary_shortcut("⇧⌘O", "Ctrl+Shift+O"),
                    ),
                ],
            ),
        ];

        let query = self
            .keybindings_search
            .read(cx)
            .content()
            .trim()
            .to_lowercase();

        let search_input = div().w_full().child(
            TextField::new("keybindings-search-field", self.keybindings_search.clone())
                .icon("icons/search.svg", 13.0)
                .w_full(),
        );

        let mut content = div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .mt(px(15.0))
            .child(search_input);

        let mut matched_any = false;

        for (section_title_key, shortcuts) in sections {
            let section_title = crate::i18n::translate(section_title_key);
            let section_title_lower = section_title.to_lowercase();
            let section_matches = !query.is_empty() && section_title_lower.contains(&query);

            let matching_shortcuts: Vec<_> = shortcuts
                .iter()
                .filter(|(title_key, desc_key, shortcut)| {
                    if query.is_empty() || section_matches {
                        return true;
                    }
                    let title = crate::i18n::translate(title_key).to_lowercase();
                    let desc = crate::i18n::translate(desc_key).to_lowercase();
                    let shortcut_lower = shortcut.to_lowercase();
                    title.contains(&query)
                        || desc.contains(&query)
                        || shortcut_lower.contains(&query)
                })
                .collect();

            if matching_shortcuts.is_empty() {
                continue;
            }
            matched_any = true;

            let mut section_card = div()
                .w_full()
                .flex()
                .flex_col()
                .rounded(px(13.0))
                .overflow_hidden()
                .bg(theme.raised);

            for (index, (title_key, desc_key, shortcut)) in matching_shortcuts.iter().enumerate() {
                if index > 0 {
                    section_card =
                        section_card.child(div().mx(px(20.0)).h(px(1.0)).bg(theme.border));
                }

                let row = div()
                    .w_full()
                    .min_h(px(54.0))
                    .px(px(20.0))
                    .py(px(10.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(24.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(sp(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(crate::i18n::translate(title_key)),
                            )
                            .child(
                                div()
                                    .mt(px(3.0))
                                    .text_size(sp(12.0))
                                    .line_height(sp(16.0))
                                    .text_color(theme.text_secondary)
                                    .child(crate::i18n::translate(desc_key)),
                            ),
                    )
                    .child(
                        div()
                            .h(px(24.0))
                            .min_w(px(32.0))
                            .px(px(8.0))
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(theme.border_strong)
                            .bg(theme.surface)
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(sp(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_secondary)
                            .child(*shortcut),
                    );

                section_card = section_card.child(row);
            }

            let section_block = div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .px(px(4.0))
                        .text_size(sp(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_secondary)
                        .child(section_title),
                )
                .child(section_card);

            content = content.child(section_block);
        }

        if !matched_any {
            content = content.child(
                div()
                    .w_full()
                    .py(px(32.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(sp(13.0))
                    .text_color(theme.text_tertiary)
                    .child(tr!("keybindings.no_results")),
            );
        }

        content.into_any_element()
    }
}
