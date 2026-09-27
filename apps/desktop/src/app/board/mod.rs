mod actions;
mod column;
mod drawer;
mod header;
mod markdown;
mod modal;
#[cfg(test)]
mod tests;
pub mod types;

use std::collections::HashMap;

use padu_client::kanban::TaskSummary;
use uuid::Uuid;

use super::*;
use crate::model::unix_time;
use crate::theme::Theme;

pub use types::*;

impl Padu {
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

        if selected_task_id.is_some() && !self.board_new_task_modal_open {
            page = page.on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key.as_str() == "escape" {
                    this.select_board_task(None, cx);
                    cx.stop_propagation();
                }
            }));
        }

        // Modal for New Task
        if self.board_new_task_modal_open {
            page = page.child(self.render_new_task_modal(&theme, window, cx));
        }

        page.into_any_element()
    }
}
