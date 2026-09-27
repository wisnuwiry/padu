use super::super::*;
use crate::app::TRAFFIC_LIGHT_CLEARANCE;
use crate::model::ProviderKind;
use crate::theme::{Theme, sp};
use crate::ui::menu::{MenuAlign, MenuItem, dropdown_menu};
use crate::ui::text_field::TextField;
use crate::ui::{MenuChip, icon, provider_icon};

impl Padu {
    pub(super) fn render_board_header(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
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
        let filtered_count = self
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
                let query = self.board_search.read(cx).content().trim().to_lowercase();
                if !query.is_empty() {
                    let matches_title = t.title.to_lowercase().contains(&query);
                    let matches_desc = t.description_preview.to_lowercase().contains(&query);
                    if !matches_title && !matches_desc {
                        return false;
                    }
                }
                true
            })
            .count();

        let project_filter_selector = dropdown_menu(
            MenuChip::new("board-filter-project-chip")
                .icon(
                    "icons/folder.svg",
                    if current_project_id.is_some() {
                        theme.accent
                    } else {
                        theme.text_tertiary
                    },
                )
                .label(current_project_name)
                .outlined()
                .background(if current_project_id.is_some() {
                    theme.accent.opacity(0.12)
                } else {
                    theme.inset
                })
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
                .background(if current_agent.is_some() {
                    theme.accent.opacity(0.12)
                } else {
                    theme.inset
                })
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

        let flags_group = div()
            .id("board-filter-flags-group")
            .h(px(28.0))
            .p(px(2.0))
            .rounded(px(7.0))
            .bg(theme.inset)
            .border_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .gap(px(2.0))
            .child({
                let active = self.board_filter_needs_attention;
                div()
                    .id("board-filter-attention")
                    .h(px(24.0))
                    .px(px(8.0))
                    .rounded(px(5.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(5.0))
                    .cursor_pointer()
                    .text_size(sp(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .bg(if active {
                        theme.warning.opacity(0.18)
                    } else {
                        gpui::transparent_black()
                    })
                    .text_color(if active {
                        theme.warning
                    } else {
                        theme.text_secondary
                    })
                    .hover(|s| {
                        if !active {
                            s.bg(theme.overlay_strong).text_color(theme.text)
                        } else {
                            s.opacity(0.85)
                        }
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
                    .focus_visible(|style| style.border_1().border_color(theme.accent))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.board_filter_needs_attention = !this.board_filter_needs_attention;
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            this.board_filter_needs_attention = !this.board_filter_needs_attention;
                            cx.notify();
                            cx.stop_propagation();
                        }
                    }))
            })
            .child({
                let active = self.board_filter_sync_failed;
                div()
                    .id("board-filter-sync-failed")
                    .h(px(24.0))
                    .px(px(8.0))
                    .rounded(px(5.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(5.0))
                    .cursor_pointer()
                    .text_size(sp(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .bg(if active {
                        theme.danger.opacity(0.18)
                    } else {
                        gpui::transparent_black()
                    })
                    .text_color(if active {
                        theme.danger
                    } else {
                        theme.text_secondary
                    })
                    .hover(|s| {
                        if !active {
                            s.bg(theme.overlay_strong).text_color(theme.text)
                        } else {
                            s.opacity(0.85)
                        }
                    })
                    .child(icon(
                        "icons/sync-failed.svg",
                        12.0,
                        if active {
                            theme.danger
                        } else {
                            theme.text_tertiary
                        },
                    ))
                    .child(tr!("board.sync_failed"))
                    .tab_index(0)
                    .focus_visible(|style| style.border_1().border_color(theme.accent))
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
            });

        let clear_filters_button = if has_filter {
            Some(
                div()
                    .id("board-clear-filters")
                    .h(px(28.0))
                    .px(px(8.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(4.0))
                    .cursor_pointer()
                    .bg(theme.inset)
                    .border_1()
                    .border_color(theme.border)
                    .text_size(sp(11.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_secondary)
                    .hover(|s| s.bg(theme.overlay_strong).text_color(theme.text))
                    .child(icon("icons/x.svg", 11.0, theme.text_secondary))
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
        } else {
            None
        };

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
                    .id("board-header-left")
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .min_w_0()
                    .overflow_x_scroll()
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
                            .flex_none()
                            .text_size(sp(15.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(tr!("board.title")),
                    )
                    .child(
                        div()
                            .flex_none()
                            .px(px(6.0))
                            .py(px(1.0))
                            .rounded(px(10.0))
                            .bg(theme.inset)
                            .border_1()
                            .border_color(theme.border)
                            .text_size(sp(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_secondary)
                            .child(filtered_count.to_string()),
                    )
                    .child(div().w(px(1.0)).h(px(16.0)).flex_none().bg(theme.border))
                    // Search field
                    .child(
                        div()
                            .w(px(140.0))
                            .flex_shrink_0()
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
                    .child(div().w(px(1.0)).h(px(16.0)).flex_none().bg(theme.border))
                    // Flags group
                    .child(flags_group)
                    // Clear filters button if active
                    .children(clear_filters_button),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .flex_none()
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
                            .border_1()
                            .border_color(theme.border)
                            .text_color(theme.text_secondary)
                            .hover(|s| s.bg(theme.overlay_strong).text_color(theme.text))
                            .child(icon("icons/rotate-cw.svg", 13.0, theme.text_secondary))
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
}
