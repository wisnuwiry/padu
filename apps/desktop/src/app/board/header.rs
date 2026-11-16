use super::super::*;
use crate::app::TRAFFIC_LIGHT_CLEARANCE;
use crate::model::ProviderKind;
use crate::theme::{Theme, sp};
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::menu::{MenuAlign, MenuItem, dropdown_menu};
use crate::ui::text_field::TextField;
use crate::ui::tooltip::Tooltip;
use crate::ui::{icon, provider_icon};
use bezel::ui::widgets::Content;

impl Padu {
    pub(crate) fn board_filter_agents(&self) -> Vec<ProviderKind> {
        self.profile_row_order()
    }

    pub(crate) fn reset_board_filters(&mut self, cx: &mut Context<Self>) {
        self.board_filter_project = None;
        self.board_filter_agent = None;
        self.board_filter_needs_attention = false;
        self.board_filter_sync_failed = false;
        self.board_search.update(cx, |s, cx| s.set_content("", cx));
        cx.notify();
    }

    pub(super) fn render_board_header(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        self.ensure_agent_profiles(false, cx);
        let filter_agents = self.board_filter_agents();

        let theme = Theme::current(cx);
        let bezel_theme = bezel::theme::Theme::of(cx).clone();

        let active_filter_count = (self.board_filter_project.is_some() as usize)
            + (self.board_filter_agent.is_some() as usize)
            + (self.board_filter_needs_attention as usize)
            + (self.board_filter_sync_failed as usize);
        let is_filtered = active_filter_count > 0;
        let has_search = !self.board_search.read(cx).content().trim().is_empty();
        let has_any_filter = is_filtered || has_search;

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

        let filter_menu_handle = self.menu_handle("board-filter-menu", cx);
        let is_menu_open = filter_menu_handle.is_open();

        let current_project_id = self.board_filter_project;
        let current_agent = self.board_filter_agent;
        let is_attention_active = self.board_filter_needs_attention;
        let is_sync_active = self.board_filter_sync_failed;

        let projects_for_menu: Vec<(Uuid, String)> = self
            .state
            .projects
            .iter()
            .filter(|p| !p.is_projectless())
            .map(|p| (p.id, p.display_name().to_string()))
            .collect();

        let weak = cx.entity().downgrade();

        let filter_trigger = div()
            .id("board-filter-button")
            .h(px(28.0))
            .px(px(8.0))
            .rounded(px(bezel::theme::Theme::button_radius()))
            .border_1()
            .border_color(if is_filtered {
                theme.accent.opacity(0.6)
            } else if is_menu_open {
                theme.accent
            } else {
                theme.border
            })
            .bg(if is_filtered {
                theme.accent.opacity(0.12)
            } else if is_menu_open {
                theme.overlay_strong
            } else {
                theme.inset
            })
            .cursor_pointer()
            .flex()
            .items_center()
            .gap(px(5.0))
            .child(icon(
                "icons/list-filter.svg",
                13.0,
                if is_filtered {
                    theme.accent
                } else {
                    theme.text_secondary
                },
            ))
            .child(
                div()
                    .text_size(sp(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if is_filtered {
                        theme.accent
                    } else {
                        theme.text
                    })
                    .child(tr!("board.filters")),
            )
            .when(is_filtered, |el| {
                el.child(bezel_theme.badge(active_filter_count.to_string()))
            })
            .child(icon(
                "icons/chevron-down.svg",
                11.0,
                if is_filtered {
                    theme.accent
                } else {
                    theme.text_tertiary
                },
            ))
            .hover(|s| {
                s.bg(if is_filtered {
                    theme.accent.opacity(0.18)
                } else {
                    bezel_theme.element_hover
                })
            })
            .focus_visible(|style| style.border_1().border_color(theme.accent))
            .tab_index(0)
            .tooltip(Tooltip::text(tr!("board.filters")));

        let filter_dropdown = dropdown_menu(
            filter_trigger,
            "board-filter-menu",
            &filter_menu_handle,
            MenuAlign::BelowLeft,
            move |_| {
                let weak = weak.clone();
                let mut items = Vec::new();

                // 1. Status Quick-Toggles Header
                items.push(MenuItem::Header(tr!("board.status").into()));

                // Needs Attention
                let weak_attn = weak.clone();
                let attn_active = is_attention_active;
                items.push(
                    MenuItem::new(tr!("board.needs_attention"), move |_, cx| {
                        let _ = weak_attn.update(cx, |this, cx| {
                            this.board_filter_needs_attention = !this.board_filter_needs_attention;
                            cx.notify();
                        });
                    })
                    .icon("icons/alert.svg")
                    .selected(attn_active),
                );

                // Sync Failed
                let weak_sync = weak.clone();
                let sync_active = is_sync_active;
                items.push(
                    MenuItem::new(tr!("board.sync_failed"), move |_, cx| {
                        let _ = weak_sync.update(cx, |this, cx| {
                            this.board_filter_sync_failed = !this.board_filter_sync_failed;
                            cx.notify();
                        });
                    })
                    .icon("icons/sync-failed.svg")
                    .selected(sync_active),
                );

                items.push(MenuItem::Separator);

                // 2. Project Submenu
                let current_project_name = current_project_id
                    .and_then(|id| projects_for_menu.iter().find(|p| p.0 == id))
                    .map(|p| p.1.clone())
                    .unwrap_or_else(|| tr!("board.filter_all_projects"));

                let weak_proj = weak.clone();
                let projects_list = projects_for_menu.clone();
                let selected_proj_id = current_project_id;
                items.push(MenuItem::submenu_with_value(
                    tr!("board.project"),
                    current_project_name,
                    move |_| {
                        let mut proj_items = Vec::new();
                        let weak_all = weak_proj.clone();
                        proj_items.push(
                            MenuItem::new(tr!("board.filter_all_projects"), move |_, cx| {
                                let _ = weak_all.update(cx, |this, cx| {
                                    this.board_filter_project = None;
                                    cx.notify();
                                });
                            })
                            .icon("icons/folder.svg")
                            .selected(selected_proj_id.is_none()),
                        );
                        proj_items.push(MenuItem::Separator);

                        for (pid, pname) in projects_list.clone() {
                            let weak_item = weak_proj.clone();
                            let is_sel = selected_proj_id == Some(pid);
                            proj_items.push(
                                MenuItem::new(pname, move |_, cx| {
                                    let _ = weak_item.update(cx, |this, cx| {
                                        this.board_filter_project = Some(pid);
                                        cx.notify();
                                    });
                                })
                                .icon("icons/folder.svg")
                                .selected(is_sel),
                            );
                        }
                        proj_items
                    },
                ));

                // 3. Agent Submenu
                let current_agent_name = current_agent
                    .map(|a| a.short_name().to_string())
                    .unwrap_or_else(|| tr!("board.filter_all_agents"));

                let weak_agent = weak.clone();
                let selected_agent = current_agent;
                let agents_list = filter_agents.clone();
                items.push(MenuItem::submenu_with_value(
                    tr!("board.assigned_agent"),
                    current_agent_name,
                    move |_| {
                        let mut agent_items = Vec::new();
                        let weak_all = weak_agent.clone();
                        agent_items.push(
                            MenuItem::new(tr!("board.filter_all_agents"), move |_, cx| {
                                let _ = weak_all.update(cx, |this, cx| {
                                    this.board_filter_agent = None;
                                    cx.notify();
                                });
                            })
                            .icon("icons/bot.svg")
                            .selected(selected_agent.is_none()),
                        );
                        agent_items.push(MenuItem::Separator);

                        for &provider in &agents_list {
                            let weak_item = weak_agent.clone();
                            let is_sel = selected_agent == Some(provider);
                            agent_items.push(
                                MenuItem::new(provider.short_name(), move |_, cx| {
                                    let _ = weak_item.update(cx, |this, cx| {
                                        this.board_filter_agent = Some(provider);
                                        cx.notify();
                                    });
                                })
                                .icon(provider_icon(provider))
                                .selected(is_sel),
                            );
                        }
                        agent_items
                    },
                ));

                // 4. Clear Filters item
                if has_any_filter {
                    items.push(MenuItem::Separator);
                    let weak_clear = weak.clone();
                    items.push(
                        MenuItem::new(tr!("board.clear_filters"), move |_, cx| {
                            let _ = weak_clear.update(cx, |this, cx| {
                                this.reset_board_filters(cx);
                            });
                        })
                        .icon("icons/x.svg")
                        .destructive(true),
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
            .gap(px(6.0))
            .pl(if self.sidebar_visible {
                px(10.0)
            } else {
                px(0.0)
            })
            .pr(px(14.0))
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
                    .id("board-header-filters")
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .min_w_0()
                    .child(icon("icons/list-checks.svg", 16.0, theme.text_secondary))
                    .child(
                        div()
                            .text_size(sp(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(tr!("board.title")),
                    )
                    .child(bezel_theme.badge(filtered_count.to_string()))
                    .child(div().w(px(1.0)).h(px(16.0)).flex_none().bg(theme.border))
                    // Search field
                    .child(
                        TextField::new("board-search", self.board_search.clone())
                            .icon("icons/search.svg", 12.0)
                            .w(px(160.0))
                            .flex_shrink_0(),
                    )
                    // Filter dropdown menu
                    .child(filter_dropdown),
            )
            // Drag region
            .child(
                self.window_drag_region(div().id("board-page-drag-region").h_full().flex_1(), cx),
            )
            .child(
                div()
                    .id("board-header-actions")
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .flex_none()
                    // Refresh button
                    .child(
                        Button::new("board-refresh", tr!("common.refresh"))
                            .button_style(ButtonStyle::Ghost)
                            .icon("icons/rotate-cw.svg")
                            .label_hidden()
                            .tooltip(Tooltip::text(tr!("common.refresh")))
                            .on_press(cx.listener(|this, _, _, cx| {
                                this.load_board_tasks_from_daemon(cx);
                            })),
                    )
                    // New Task button
                    .child(
                        Button::new("board-new-task-button", tr!("board.new_task"))
                            .button_style(ButtonStyle::Prominent)
                            .icon("icons/plus.svg")
                            .tooltip(Tooltip::text(tr!("board.new_task")))
                            .on_press(cx.listener(|this, _, _, cx| {
                                this.open_new_task_modal(cx);
                            })),
                    ),
            );

        header
    }
}
