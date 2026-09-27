use uuid::Uuid;

use super::super::*;
use crate::theme::{Theme, sp};
use crate::ui::MenuChip;
use crate::ui::dialog::{dialog_backdrop, dialog_cancel_button, dialog_card};
use crate::ui::menu::{MenuAlign, MenuItem as PaduMenuItem, dropdown_menu};
use crate::ui::text_field::TextField;

use super::types::{BOARD_MODAL_PROVIDERS, BoardAgentPickTarget};

impl Padu {
    pub(crate) fn render_new_task_modal(
        &mut self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cancel_focus = self.board_new_task_cancel_focus.clone();
        let title_input = self.board_new_task_title.clone();
        let desc_input = self.board_new_task_description.clone();
        let project_id = self
            .board_new_task_project_id
            .or_else(|| self.current_project_id())
            .unwrap_or_else(Uuid::nil);
        let agent = self.board_new_task_agent;

        let project_modal_handle = self.menu_handle("board-new-task-project", cx);
        let current_modal_project_id = self
            .board_new_task_project_id
            .or_else(|| self.current_project_id());
        let current_modal_project_name = current_modal_project_id
            .and_then(|id| self.state.projects.iter().find(|p| p.id == id))
            .map(|p| p.display_name())
            .unwrap_or_else(|| tr!("board.all_projects"));
        let modal_project_options = self
            .state
            .projects
            .iter()
            .filter(|p| !p.is_projectless())
            .map(|p| (p.id, p.display_name()))
            .collect::<Vec<_>>();
        let weak_modal_project = cx.entity().downgrade();
        let modal_project_selector = dropdown_menu(
            MenuChip::new("modal-project-chip")
                .icon("icons/folder.svg", theme.text_tertiary)
                .label(current_modal_project_name)
                .outlined()
                .background(theme.inset)
                .height(px(28.0))
                .selected(project_modal_handle.is_open())
                .w_full()
                .justify_between(),
            "modal-project-menu",
            &project_modal_handle,
            MenuAlign::BelowLeft,
            move |_| {
                let mut items = Vec::new();
                for (pid, pname) in modal_project_options.clone() {
                    let weak = weak_modal_project.clone();
                    let is_sel = current_modal_project_id == Some(pid);
                    items.push(
                        PaduMenuItem::new(pname, move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.board_new_task_project_id = Some(pid);
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

        let card = dialog_card("board-new-task-dialog", theme, px(480.0))
            .child(
                div()
                    .text_size(sp(16.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child(tr!("board.create_task")),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    // Project selector
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(sp(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text_secondary)
                                    .child(tr!("board.filter_project")),
                            )
                            .child(modal_project_selector),
                    )
                    // Title field
                    .child(
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
                                    .h(px(32.0))
                                    .rounded(px(6.0))
                                    .bg(theme.inset)
                                    .child(TextField::new("new-task-title", title_input.clone())),
                            ),
                    )
                    // Description field with markdown editor
                    .child(self.render_board_markdown_editor(
                        "new-task-description",
                        &self.board_new_task_description,
                        self.board_new_task_preview,
                        px(96.0),
                        px(200.0),
                        theme,
                        window,
                        cx,
                        |this, preview, cx| {
                            this.board_new_task_preview = preview;
                            cx.notify();
                        },
                    ))
                    // Agent selector chips (disabled profiles unselectable)
                    .child(
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
                                agent,
                                "board-new-task",
                                BoardAgentPickTarget::NewTask,
                                &BOARD_MODAL_PROVIDERS,
                                theme,
                                cx,
                            )),
                    )
                    // Model selector chips (only when agent is selected)
                    .when_some(agent, |element, p| {
                        let models = self.board_models_for_provider(p);
                        if models.is_empty() {
                            return element;
                        }
                        let current_model = self.board_new_task_model.clone();
                        let default_selected = current_model.is_none();
                        let mut model_chips = div().flex().flex_wrap().gap(px(6.0));

                        // Default model chip
                        model_chips = model_chips.child(
                            div()
                                .id("board-new-task-model-default")
                                .h(px(24.0))
                                .px(px(8.0))
                                .rounded(px(4.0))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .text_size(sp(11.0))
                                .font_weight(FontWeight::MEDIUM)
                                .when(default_selected, |chip| {
                                    chip.bg(theme.accent.opacity(0.18))
                                        .text_color(theme.accent)
                                        .border_1()
                                        .border_color(theme.accent.opacity(0.4))
                                })
                                .when(!default_selected, |chip| {
                                    chip.bg(theme.inset)
                                        .text_color(theme.text_secondary)
                                        .border_1()
                                        .border_color(theme.border)
                                })
                                .child(tr!("board.default_model"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.board_new_task_model = None;
                                    cx.notify();
                                })),
                        );

                        for (m_id, m_name) in models {
                            let is_selected = current_model.as_deref() == Some(&m_id);
                            let m_id_for_click = m_id.clone();
                            model_chips = model_chips.child(
                                div()
                                    .id(SharedString::from(format!(
                                        "board-new-task-model-{}",
                                        m_id
                                    )))
                                    .h(px(24.0))
                                    .px(px(8.0))
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .text_size(sp(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .when(is_selected, |chip| {
                                        chip.bg(theme.accent.opacity(0.18))
                                            .text_color(theme.accent)
                                            .border_1()
                                            .border_color(theme.accent.opacity(0.4))
                                    })
                                    .when(!is_selected, |chip| {
                                        chip.bg(theme.inset)
                                            .text_color(theme.text_secondary)
                                            .border_1()
                                            .border_color(theme.border)
                                    })
                                    .child(m_name)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.board_new_task_model = Some(m_id_for_click.clone());
                                        cx.notify();
                                    })),
                            );
                        }

                        element.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_size(sp(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text_secondary)
                                        .child(tr!("board.model")),
                                )
                                .child(model_chips),
                        )
                    }),
            )
            // Footer
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(8.0))
                    .mt(px(8.0))
                    .child(dialog_cancel_button(
                        "cancel-new-task",
                        tr!("board.cancel"),
                        &cancel_focus,
                        theme,
                        cx,
                        |this, _, cx| {
                            this.board_new_task_modal_open = false;
                            cx.notify();
                        },
                    ))
                    .child({
                        let title_str = title_input.read(cx).content().to_owned();
                        let desc_str = desc_input.read(cx).content().to_owned();
                        div()
                            .id("create-new-task-submit")
                            .h(px(32.0))
                            .px(px(14.0))
                            .rounded(px(7.0))
                            .bg(theme.inverse)
                            .text_color(theme.on_inverse)
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(sp(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .hover(|s| s.opacity(0.92))
                            .child(tr!("board.create_task"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let model = this.board_new_task_model.clone();
                                this.create_board_task(
                                    project_id,
                                    title_str.clone(),
                                    desc_str.clone(),
                                    agent,
                                    model,
                                    cx,
                                );
                            }))
                    }),
            );

        dialog_backdrop(
            "new-task-dialog-backdrop",
            theme,
            cx,
            |this, _, cx| {
                this.board_new_task_modal_open = false;
                cx.notify();
            },
            card,
        )
    }
}
