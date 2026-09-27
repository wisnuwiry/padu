use std::collections::HashMap;

use padu_client::kanban::{TaskStatus, TaskSummary};
use uuid::Uuid;

use super::super::*;
use crate::model::ProviderKind;
use crate::theme::{Theme, sp};
use crate::ui::tooltip::Tooltip;
use crate::ui::{icon, provider_icon};

use super::types::{BoardAgentPickTarget, TaskLiveBadges, task_status_color, task_status_label};

impl Padu {
    pub(crate) fn render_board_column(
        &self,
        status: TaskStatus,
        tasks: Vec<TaskSummary>,
        selected_task_id: Option<Uuid>,
        live_badges: &HashMap<Uuid, TaskLiveBadges>,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let status_color = task_status_color(status, theme);
        let label = task_status_label(status);
        let count = tasks.len();

        let is_collapsed = self.board_collapsed_columns.contains(&status);
        if is_collapsed {
            let status_for_toggle = status;
            return div()
                .id(SharedString::from(format!(
                    "board-col-{:?}-collapsed",
                    status
                )))
                .w(px(40.0))
                .min_w(px(40.0))
                .flex_none()
                .h_full()
                .flex()
                .flex_col()
                .items_center()
                .py(px(10.0))
                .gap(px(10.0))
                .rounded(px(10.0))
                .bg(theme.surface)
                .border_1()
                .border_color(theme.border)
                .overflow_hidden()
                .cursor_pointer()
                .hover(|s| s.bg(theme.inset))
                .tab_index(0)
                .focus_visible(|style| style.border_color(theme.accent))
                .tooltip(Tooltip::text(format!(
                    "{} ({}) — {}",
                    label,
                    count,
                    tr!("board.expand_column")
                )))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.board_collapsed_columns.remove(&status_for_toggle);
                    cx.notify();
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        this.board_collapsed_columns.remove(&status_for_toggle);
                        cx.notify();
                        cx.stop_propagation();
                    }
                }))
                .child(
                    div()
                        .w(px(24.0))
                        .h(px(24.0))
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .hover(|s| s.bg(theme.overlay_strong))
                        .child(icon("icons/chevron-right.svg", 13.0, theme.text_secondary)),
                )
                .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(status_color))
                .child(
                    div()
                        .h(px(20.0))
                        .px(px(6.0))
                        .rounded_full()
                        .bg(theme.inset)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(sp(11.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_secondary)
                        .child(count.to_string()),
                );
        }

        let mut column = div()
            .id(SharedString::from(format!("board-col-{:?}", status)))
            .w(px(280.0))
            .min_w(px(280.0))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .rounded(px(10.0))
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border);

        // Column Header
        let status_for_collapse = status;
        let col_header = div()
            .h(px(40.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px(px(12.0))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(status_color))
                    .child(
                        div()
                            .text_size(sp(13.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(label),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .h(px(20.0))
                            .px(px(6.0))
                            .rounded_full()
                            .bg(theme.inset)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(sp(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_secondary)
                            .child(count.to_string()),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("collapse-col-{:?}", status)))
                            .w(px(20.0))
                            .h(px(20.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|s| s.bg(theme.overlay_strong))
                            .tab_index(0)
                            .focus_visible(|style| style.border_color(theme.accent))
                            .tooltip(Tooltip::text(tr!("board.collapse_column")))
                            .child(icon("icons/chevron-left.svg", 12.0, theme.text_tertiary))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.board_collapsed_columns.insert(status_for_collapse);
                                cx.notify();
                            }))
                            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    this.board_collapsed_columns.insert(status_for_collapse);
                                    cx.notify();
                                    cx.stop_propagation();
                                }
                            })),
                    ),
            );

        column = column.child(col_header);

        // Column Body (Card list)
        let mut card_list = div()
            .id(SharedString::from(format!("board-col-cards-{:?}", status)))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(8.0))
            .flex()
            .flex_col()
            .gap(px(8.0));

        if tasks.is_empty() {
            card_list = card_list.child(
                div()
                    .py(px(24.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(sp(12.0))
                            .text_color(theme.text_ghost)
                            .child(tr!("board.empty_column")),
                    ),
            );
        } else {
            for task in &tasks {
                let live = live_badges.get(&task.id).copied().unwrap_or_default();
                card_list = card_list.child(self.render_task_card(
                    task,
                    live,
                    selected_task_id == Some(task.id),
                    theme,
                    window,
                    cx,
                ));
            }
        }

        column.child(card_list)
    }

    pub(crate) fn render_task_card(
        &self,
        task: &TaskSummary,
        live: TaskLiveBadges,
        is_selected: bool,
        theme: &Theme,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let task_id = task.id;
        let task_version = task.version;
        let status = task.status;

        let mut card = div()
            .id(SharedString::from(format!("task-card-{}", task.id)))
            .p(px(10.0))
            .rounded(px(8.0))
            .bg(if is_selected {
                theme.raised
            } else {
                theme.inset
            })
            .border_1()
            .border_color(if is_selected {
                theme.accent
            } else {
                theme.border
            })
            .shadow_sm()
            .cursor_pointer()
            .hover(|s| s.border_color(theme.accent.opacity(0.8)))
            .tab_index(0)
            .focus_visible(|style| style.border_color(theme.accent))
            .flex()
            .flex_col()
            .gap(px(6.0))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_board_task(Some(task_id), cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    this.select_board_task(Some(task_id), cx);
                    cx.stop_propagation();
                }
            }));

        // Title
        card = card.child(
            div()
                .text_size(sp(13.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text)
                .child(task.title.clone()),
        );

        // Description preview
        if !task.description_preview.is_empty() {
            card = card.child(
                div()
                    .text_size(sp(11.5))
                    .text_color(theme.text_secondary)
                    .child(task.description_preview.clone()),
            );
        }

        // Flags & Badges row
        let mut badges_row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .mt(px(2.0));

        // Assigned Agent Badge
        if let Some(agent) = task.assigned_agent {
            badges_row = badges_row.child(
                div()
                    .h(px(18.0))
                    .px(px(5.0))
                    .rounded(px(4.0))
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(icon(provider_icon(agent), 11.0, theme.text_secondary))
                    .child(
                        div()
                            .text_size(sp(10.5))
                            .text_color(theme.text_secondary)
                            .child(agent.short_name()),
                    ),
            );
        }

        // Model Badge
        if let Some(model) = &task.model {
            badges_row = badges_row.child(
                div()
                    .h(px(18.0))
                    .px(px(5.0))
                    .rounded(px(4.0))
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap(px(3.0))
                    .child(icon("icons/sparkle.svg", 10.0, theme.text_tertiary))
                    .child(
                        div()
                            .text_size(sp(10.0))
                            .text_color(theme.text_tertiary)
                            .child(model.clone()),
                    ),
            );
        }

        // Needs Attention Badge
        if task.needs_attention {
            badges_row = badges_row.child(
                div()
                    .h(px(18.0))
                    .px(px(5.0))
                    .rounded(px(4.0))
                    .bg(theme.warning.opacity(0.15))
                    .border_1()
                    .border_color(theme.warning.opacity(0.3))
                    .flex()
                    .items_center()
                    .gap(px(3.0))
                    .child(icon("icons/alert.svg", 10.0, theme.warning))
                    .child(
                        div()
                            .text_size(sp(10.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.warning)
                            .child(tr!("board.needs_attention")),
                    ),
            );
        }

        // Sync Failed Badge
        if task.sync_failed.is_some() {
            badges_row = badges_row.child(
                div()
                    .h(px(18.0))
                    .px(px(5.0))
                    .rounded(px(4.0))
                    .bg(theme.danger.opacity(0.15))
                    .border_1()
                    .border_color(theme.danger.opacity(0.3))
                    .flex()
                    .items_center()
                    .gap(px(3.0))
                    .child(icon("icons/sync-failed.svg", 10.0, theme.danger))
                    .child(
                        div()
                            .text_size(sp(10.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.danger)
                            .child(tr!("board.sync_failed")),
                    ),
            );
        }

        // Linked session indicator
        if task.session_id.is_some() {
            badges_row = badges_row.child(
                div()
                    .h(px(18.0))
                    .px(px(4.0))
                    .rounded(px(4.0))
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(icon("icons/external-link.svg", 10.0, theme.text_tertiary))
                    .child(
                        div()
                            .text_size(sp(10.0))
                            .text_color(theme.text_tertiary)
                            .child(tr!("board.linked_session")),
                    ),
            );
        }

        // Live usage badges (P1-08): tokens + duration from the linked
        // session's streamed usage events. Icon + text, never color alone.
        if let Some(tokens) = live.tokens {
            badges_row = badges_row.child(
                div()
                    .h(px(18.0))
                    .px(px(5.0))
                    .rounded(px(4.0))
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap(px(3.0))
                    .child(icon("icons/zap.svg", 10.0, theme.text_tertiary))
                    .child(
                        div()
                            .text_size(sp(10.0))
                            .text_color(theme.text_secondary)
                            .child(crate::usage::format_tokens(tokens)),
                    ),
            );
        }

        if let Some(duration) = live.duration_secs {
            badges_row = badges_row.child(
                div()
                    .h(px(18.0))
                    .px(px(5.0))
                    .rounded(px(4.0))
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_size(sp(10.0))
                            .text_color(theme.text_secondary)
                            .child(crate::app::transcript::format_working_elapsed(duration)),
                    ),
            );
        }

        card = card.child(badges_row);

        // Quick action button
        let quick_action = match status {
            TaskStatus::Backlog => Some((
                tr!("board.queue_action"),
                "icons/arrow-right.svg",
                TaskStatus::Queued,
            )),
            TaskStatus::Review => {
                Some((tr!("board.mark_done"), "icons/check.svg", TaskStatus::Done))
            }
            TaskStatus::Done => Some((
                tr!("board.reopen"),
                "icons/rotate-cw.svg",
                TaskStatus::Backlog,
            )),
            _ => None,
        };

        if let Some((label, icon_path, next_status)) = quick_action {
            card = card.child(
                div().flex().justify_end().mt(px(4.0)).child(
                    div()
                        .id(SharedString::from(format!("task-action-{}", task.id)))
                        .h(px(22.0))
                        .px(px(7.0))
                        .rounded(px(4.0))
                        .bg(theme.surface)
                        .border_1()
                        .border_color(theme.border)
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .text_size(sp(11.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_secondary)
                        .hover(|s| s.bg(theme.overlay_strong).text_color(theme.text))
                        .tab_index(0)
                        .focus_visible(|style| style.border_color(theme.accent))
                        .child(icon(icon_path, 10.0, theme.text_secondary))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.move_board_task(task_id, next_status, task_version, cx);
                            cx.stop_propagation();
                        }))
                        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                this.move_board_task(task_id, next_status, task_version, cx);
                                cx.stop_propagation();
                            }
                        })),
                ),
            );
        }

        card
    }

    /// Assignee chip row shared by the new-task modal and the detail drawer.
    /// Disabled profiles render dimmed with a re-enable tooltip and are not
    /// selectable (no tab stop, no action). `id_prefix` keeps element ids
    /// unique per surface; `providers` lets the modal keep its curated six
    /// while the drawer offers the full registry like the web client.
    pub(crate) fn render_board_agent_chips(
        &self,
        selected: Option<ProviderKind>,
        id_prefix: &str,
        target: BoardAgentPickTarget,
        providers: &[ProviderKind],
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut row = div().flex().flex_wrap().gap(px(6.0));
        row = row.child(
            div()
                .id(SharedString::from(format!("{id_prefix}-agent-none")))
                .h(px(24.0))
                .px(px(8.0))
                .rounded(px(4.0))
                .cursor_pointer()
                .flex()
                .items_center()
                .text_size(sp(11.0))
                .font_weight(FontWeight::MEDIUM)
                .tab_index(0)
                .focus_visible(|style| style.border_color(theme.accent))
                .when(selected.is_none(), |chip| {
                    chip.bg(theme.accent.opacity(0.18))
                        .text_color(theme.accent)
                        .border_1()
                        .border_color(theme.accent.opacity(0.4))
                })
                .when(selected.is_some(), |chip| {
                    chip.bg(theme.inset)
                        .text_color(theme.text_secondary)
                        .border_1()
                        .border_color(theme.border)
                })
                .child(tr!("board.no_agent"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    match target {
                        BoardAgentPickTarget::NewTask => {
                            this.board_new_task_agent = None;
                            this.board_new_task_model = None;
                        }
                        BoardAgentPickTarget::Edit => {
                            this.board_edit_agent = None;
                        }
                    }
                    cx.notify();
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        match target {
                            BoardAgentPickTarget::NewTask => {
                                this.board_new_task_agent = None;
                                this.board_new_task_model = None;
                            }
                            BoardAgentPickTarget::Edit => {
                                this.board_edit_agent = None;
                            }
                        }
                        cx.notify();
                        cx.stop_propagation();
                    }
                })),
        );
        for provider in providers {
            let provider = *provider;
            let is_selected = selected == Some(provider);
            let is_disabled = self.board_profile_disabled(provider);
            let mut chip = div()
                .id(SharedString::from(format!(
                    "{id_prefix}-agent-{}",
                    provider.id()
                )))
                .h(px(24.0))
                .px(px(8.0))
                .rounded(px(4.0))
                .flex()
                .items_center()
                .gap(px(4.0))
                .text_size(sp(11.0))
                .font_weight(FontWeight::MEDIUM)
                .when(is_selected, |chip| {
                    chip.bg(theme.accent.opacity(0.18))
                        .text_color(theme.accent)
                        .border_1()
                        .border_color(theme.accent.opacity(0.4))
                })
                .when(!is_selected && !is_disabled, |chip| {
                    chip.bg(theme.inset)
                        .text_color(theme.text_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .cursor_pointer()
                })
                .when(is_disabled, |chip| {
                    chip.bg(theme.inset)
                        .text_color(theme.text_ghost)
                        .border_1()
                        .border_color(theme.border)
                        .opacity(0.55)
                        .tooltip(Tooltip::text(tr!("board.agent_disabled_tooltip")))
                })
                .child(icon(
                    provider_icon(provider),
                    11.0,
                    if is_selected {
                        theme.accent
                    } else {
                        theme.text_secondary
                    },
                ))
                .child(provider.short_name());
            if !is_disabled {
                chip = chip
                    .tab_index(0)
                    .focus_visible(|style| style.border_color(theme.accent))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        match target {
                            BoardAgentPickTarget::NewTask => {
                                this.board_new_task_agent = Some(provider);
                                this.board_new_task_model = None;
                            }
                            BoardAgentPickTarget::Edit => {
                                this.board_edit_agent = Some(provider);
                            }
                        }
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            match target {
                                BoardAgentPickTarget::NewTask => {
                                    this.board_new_task_agent = Some(provider);
                                    this.board_new_task_model = None;
                                }
                                BoardAgentPickTarget::Edit => {
                                    this.board_edit_agent = Some(provider);
                                }
                            }
                            cx.notify();
                            cx.stop_propagation();
                        }
                    }));
            }
            row = row.child(chip);
        }
        row
    }
}
