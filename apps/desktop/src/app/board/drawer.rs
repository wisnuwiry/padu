use padu_client::kanban::TaskStatus;
use uuid::Uuid;

use super::super::*;
use crate::model::{ProviderKind, unix_time};
use crate::theme::{Theme, sp};
use crate::ui::tooltip::Tooltip;
use crate::ui::{TagInput, icon, provider_icon};

use super::types::{
    BOARD_COLUMNS, BoardAgentPickTarget, task_live_badges, task_status_color, task_status_label,
};

impl Padu {
    pub(crate) fn render_task_detail_drawer(
        &mut self,
        task_id: Uuid,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let task_summary = self.board_tasks.iter().find(|t| t.id == task_id).cloned();
        let hydrated = self.board_hydrated_task.clone();

        let title = hydrated
            .as_ref()
            .map(|t| t.title.clone())
            .or_else(|| task_summary.as_ref().map(|t| t.title.clone()))
            .unwrap_or_default();
        let status = hydrated
            .as_ref()
            .map(|t| t.status)
            .or_else(|| task_summary.as_ref().map(|t| t.status))
            .unwrap_or_default();
        let assigned_agent = hydrated
            .as_ref()
            .and_then(|t| t.assigned_agent)
            .or_else(|| task_summary.as_ref().and_then(|t| t.assigned_agent));
        let session_id = hydrated
            .as_ref()
            .and_then(|t| t.session_id)
            .or_else(|| task_summary.as_ref().and_then(|t| t.session_id));
        let needs_attention = hydrated
            .as_ref()
            .map(|t| t.needs_attention)
            .or_else(|| task_summary.as_ref().map(|t| t.needs_attention))
            .unwrap_or_default();
        let sync_failed = hydrated
            .as_ref()
            .and_then(|t| t.sync_failed.clone())
            .or_else(|| task_summary.as_ref().and_then(|t| t.sync_failed.clone()));
        let version = hydrated
            .as_ref()
            .map(|t| t.version)
            .or_else(|| task_summary.as_ref().map(|t| t.version))
            .unwrap_or_default();
        let model = hydrated
            .as_ref()
            .and_then(|t| t.model.clone())
            .or_else(|| task_summary.as_ref().and_then(|t| t.model.clone()));

        let mut drawer = div()
            .id("task-detail-drawer")
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .w(px(380.0))
            .bg(theme.raised)
            .border_l_1()
            .border_color(theme.border_strong)
            .shadow_xl()
            .flex()
            .flex_col()
            .tab_index(0)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key.as_str() == "escape" {
                    this.select_board_task(None, cx);
                    cx.stop_propagation();
                }
            }));

        // Drawer Header: Close button and Title (sticky on top)
        let header = div()
            .id("task-detail-header")
            .flex_none()
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(task_status_color(status, theme)),
                    )
                    .child(
                        div()
                            .text_size(sp(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_secondary)
                            .child(task_status_label(status)),
                    ),
            )
            .child(
                div()
                    .id("task-detail-close")
                    .w(px(24.0))
                    .h(px(24.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|s| s.bg(theme.overlay_strong))
                    .tab_index(0)
                    .focus_visible(|style| style.border_color(theme.accent))
                    .tooltip(Tooltip::text(format!("{} (Esc)", tr!("board.cancel"))))
                    .child(icon("icons/x.svg", 14.0, theme.text_secondary))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_board_task(None, cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space" | "escape") {
                            this.select_board_task(None, cx);
                            cx.stop_propagation();
                        }
                    })),
            );

        drawer = drawer.child(header);

        // Drawer Body: Scrollable middle section
        let mut body = div()
            .id("task-detail-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(16.0))
            .flex()
            .flex_col()
            .gap(px(14.0));

        // Task Title (editable — Save persists via UpdateTask)
        let edit_title = self.board_edit_title.clone();
        let edit_title_click = edit_title.clone();
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(sp(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_secondary)
                        .child(tr!("board.task_title")),
                )
                .child(
                    div()
                        .id("task-edit-title-container")
                        .min_h(px(32.0))
                        .rounded(px(6.0))
                        .bg(theme.inset)
                        .px(px(8.0))
                        .py(px(6.0))
                        .flex()
                        .items_center()
                        .cursor_text()
                        .on_click(cx.listener(move |_, _, window, cx| {
                            let focus = edit_title_click.read(cx).focus();
                            window.focus(&focus, cx);
                        }))
                        .child(div().w_full().child(edit_title)),
                ),
        );

        // Status Changer Buttons Row
        let mut status_row = div().flex().flex_wrap().gap(px(6.0));
        for s in BOARD_COLUMNS {
            let is_current = s == status;
            let is_running = s == TaskStatus::Running;
            let s_label = task_status_label(s);
            let s_color = task_status_color(s, theme);

            status_row = status_row.child(
                div()
                    .id(SharedString::from(format!("move-status-{:?}", s)))
                    .h(px(24.0))
                    .px(px(8.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .text_size(sp(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .when(is_current, |btn| {
                        btn.bg(s_color.opacity(0.18))
                            .text_color(s_color)
                            .border_1()
                            .border_color(s_color.opacity(0.4))
                    })
                    .when(!is_current && !is_running, |btn| {
                        btn.bg(theme.surface)
                            .text_color(theme.text_secondary)
                            .border_1()
                            .border_color(theme.border)
                            .cursor_pointer()
                            .hover(|st| st.bg(theme.overlay_strong).text_color(theme.text))
                            .tab_index(0)
                            .focus_visible(|style| style.border_color(theme.accent))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.move_board_task(task_id, s, version, cx);
                            }))
                            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    this.move_board_task(task_id, s, version, cx);
                                    cx.stop_propagation();
                                }
                            }))
                    })
                    .when(is_running && !is_current, |btn| {
                        btn.bg(theme.inset)
                            .text_color(theme.text_ghost)
                            .border_1()
                            .border_color(theme.border)
                            .cursor_not_allowed()
                    })
                    .child(s_label),
            );
        }

        body = body.child(status_row);

        // Info Badges: Agent & Session
        let mut info_section = div()
            .p(px(10.0))
            .rounded(px(8.0))
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(8.0));

        // Agent row
        info_section = info_section.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(sp(12.0))
                        .text_color(theme.text_secondary)
                        .child(tr!("board.assigned_agent")),
                )
                .child(div().flex().items_center().gap(px(4.0)).child(
                    if let Some(agent) = assigned_agent {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(icon(provider_icon(agent), 12.0, theme.text))
                            .child(
                                div()
                                    .text_size(sp(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(agent.display_name()),
                            )
                    } else {
                        div()
                            .text_size(sp(12.0))
                            .text_color(theme.text_tertiary)
                            .child(tr!("board.no_agent"))
                    },
                )),
        );

        // Model row
        if let Some(m) = model {
            info_section = info_section.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(sp(12.0))
                            .text_color(theme.text_secondary)
                            .child(tr!("board.model")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(icon("icons/sparkle.svg", 11.0, theme.text_secondary))
                            .child(
                                div()
                                    .text_size(sp(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(m),
                            ),
                    ),
            );
        }

        // Live usage row (P1-08): same streamed session join as the cards.
        let live = session_id
            .and_then(|sid| self.state.sessions.iter().find(|s| s.id == sid))
            .map(|session| task_live_badges(Some(session), unix_time()))
            .unwrap_or_default();
        if live.tokens.is_some() || live.duration_secs.is_some() {
            let mut value = String::new();
            if let Some(tokens) = live.tokens {
                value.push_str(&crate::usage::format_tokens(tokens));
            }
            if let Some(duration) = live.duration_secs {
                if !value.is_empty() {
                    value.push_str(" · ");
                }
                value.push_str(&crate::app::transcript::format_working_elapsed(duration));
            }
            info_section = info_section.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(sp(12.0))
                            .text_color(theme.text_secondary)
                            .child(tr!("board.live_usage")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(icon("icons/zap.svg", 11.0, theme.text_secondary))
                            .child(
                                div()
                                    .text_size(sp(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(value),
                            ),
                    ),
            );
        }

        // Linked Session row
        if let Some(sid) = session_id {
            info_section = info_section.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(sp(12.0))
                            .text_color(theme.text_secondary)
                            .child(tr!("board.linked_session")),
                    )
                    .child(
                        div()
                            .id("board-open-session-chat")
                            .h(px(24.0))
                            .px(px(8.0))
                            .rounded(px(4.0))
                            .bg(theme.accent.opacity(0.12))
                            .border_1()
                            .border_color(theme.accent.opacity(0.3))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .text_size(sp(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.accent)
                            .hover(|s| s.bg(theme.accent.opacity(0.2)))
                            .tab_index(0)
                            .focus_visible(|style| style.border_color(theme.accent))
                            .child(icon("icons/external-link.svg", 11.0, theme.accent))
                            .child(tr!("board.open_in_chat"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_session(sid, cx);
                            }))
                            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    this.select_session(sid, cx);
                                    cx.stop_propagation();
                                }
                            })),
                    ),
            );
        }

        body = body.child(info_section);

        // Edit section: assignee chips + labels (title above, description below)
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(sp(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_secondary)
                        .child(tr!("board.assigned_agent")),
                )
                .child(self.render_board_agent_chips(
                    self.board_edit_agent,
                    "task-edit",
                    BoardAgentPickTarget::Edit,
                    &ProviderKind::ALL,
                    theme,
                    cx,
                )),
        );

        // Auto-chip in input labels: using reusable TagInput component
        let entity = cx.entity().clone();
        let tag_input = TagInput::new("task-edit-labels", self.board_edit_labels.clone())
            .bordered(false)
            .tags(self.board_edit_labels_list.clone())
            .on_remove(move |idx, _window, cx| {
                entity.update(cx, |this, cx| {
                    if idx < this.board_edit_labels_list.len() {
                        this.board_edit_labels_list.remove(idx);
                        cx.notify();
                    }
                });
            });

        let labels_section = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(sp(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_secondary)
                    .child(tr!("board.labels")),
            )
            .child(tag_input);

        body = body.child(labels_section);

        // Alert banners
        if needs_attention {
            body = body.child(
                div()
                    .p(px(8.0))
                    .rounded(px(6.0))
                    .bg(theme.warning.opacity(0.12))
                    .border_1()
                    .border_color(theme.warning.opacity(0.3))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(icon("icons/alert.svg", 13.0, theme.warning))
                    .child(
                        div()
                            .text_size(sp(11.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.warning)
                            .child(tr!("board.needs_attention")),
                    ),
            );
        }

        if let Some(err) = sync_failed {
            body = body.child(
                div()
                    .p(px(8.0))
                    .rounded(px(6.0))
                    .bg(theme.danger.opacity(0.12))
                    .border_1()
                    .border_color(theme.danger.opacity(0.3))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(icon("icons/sync-failed.svg", 13.0, theme.danger))
                            .child(
                                div()
                                    .text_size(sp(11.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.danger)
                                    .child(tr!("board.sync_failed")),
                            ),
                    )
                    .child(
                        div()
                            .text_size(sp(11.0))
                            .text_color(theme.text_secondary)
                            .child(err),
                    ),
            );
        }

        // Description editor (markdown editor with Write/Preview tabs & toolbar)
        let can_save = hydrated.is_some()
            && !self.board_edit_saving
            && !self.board_edit_title.read(cx).content().trim().is_empty();
        body = body.child(self.render_board_markdown_editor(
            "task-edit-description",
            &self.board_edit_description,
            self.board_edit_preview,
            px(110.0),
            px(280.0),
            theme,
            window,
            cx,
            |this, preview, cx| {
                this.board_edit_preview = preview;
                cx.notify();
            },
        ));

        drawer = drawer.child(body);

        // Sticky Footer: Delete on the left, Save on the right
        let title_for_delete_click = title.clone();
        let title_for_delete_key = title.clone();
        let footer = div()
            .id("task-detail-footer")
            .flex_none()
            .px(px(16.0))
            .py(px(12.0))
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.raised)
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .id("task-delete-button")
                    .h(px(28.0))
                    .px(px(10.0))
                    .rounded(px(6.0))
                    .bg(theme.danger.opacity(0.12))
                    .border_1()
                    .border_color(theme.danger.opacity(0.3))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .text_size(sp(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.danger)
                    .hover(|s| s.bg(theme.danger.opacity(0.2)))
                    .tab_index(0)
                    .focus_visible(|style| style.border_color(theme.danger))
                    .child(icon("icons/trash.svg", 12.0, theme.danger))
                    .child(tr!("board.delete_task"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.confirm_delete_task(
                            task_id,
                            title_for_delete_click.clone(),
                            version,
                            window,
                            cx,
                        );
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            this.confirm_delete_task(
                                task_id,
                                title_for_delete_key.clone(),
                                version,
                                window,
                                cx,
                            );
                            cx.stop_propagation();
                        }
                    })),
            )
            .child(
                div()
                    .id("task-edit-save")
                    .h(px(28.0))
                    .px(px(14.0))
                    .rounded(px(6.0))
                    .bg(theme.inverse)
                    .text_color(theme.on_inverse)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(sp(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .when(can_save, |element| {
                        element
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.92))
                            .tab_index(0)
                            .focus_visible(|style| style.border_color(theme.accent))
                    })
                    .when(!can_save, |element| element.opacity(0.45))
                    .child(tr!("board.save"))
                    .when(can_save, |element| {
                        element
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.save_board_task_edits(cx);
                            }))
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    this.save_board_task_edits(cx);
                                    cx.stop_propagation();
                                }
                            }))
                    }),
            );

        drawer = drawer.child(footer);

        drawer
    }
}
