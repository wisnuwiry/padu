use uuid::Uuid;

use super::super::*;
use crate::theme::{Theme, sp};
use crate::ui::MenuChip;
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::editor::{EditorExt as _, EditorSelectionToolbar, RichEditor};
use crate::ui::icon;
use crate::ui::menu::{MenuAlign, MenuItem as PaduMenuItem, dropdown_menu};
use crate::ui::text_field::TextField;
use crate::ui::tooltip::Tooltip;
use bezel::ui::popover::{Side, sheet, sheet_panel};

use super::types::BoardAgentPickTarget;

impl Padu {
    pub(crate) fn render_new_task_modal(
        &mut self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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

        let bezel_theme = bezel::theme::Theme::of(cx);
        let viewport = window.viewport_size();
        let extent = px(440.0).min(viewport.width * 0.9);

        let mut panel = sheet_panel(bezel_theme, Side::Right)
            .id("new-task-sheet-panel")
            .occlude()
            .tab_index(0)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key.as_str() == "escape" {
                    this.board_new_task_modal_open = false;
                    cx.stop_propagation();
                }
            }));

        // Header
        let header = div()
            .id("new-task-sheet-header")
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
                    .text_size(sp(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child(tr!("board.create_task")),
            )
            .child(
                div()
                    .id("new-task-sheet-close")
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
                        this.board_new_task_modal_open = false;
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space" | "escape") {
                            this.board_new_task_modal_open = false;
                            cx.stop_propagation();
                        }
                    })),
            );

        panel = panel.child(header);

        // Body
        let available_providers = self.profile_row_order();
        let body = div()
            .id("new-task-sheet-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(16.0))
            .flex()
            .flex_col()
            .gap(px(14.0))
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
            // Description field with rich text editor
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
                            .child(tr!("board.task_description")),
                    )
                    .child(
                        div()
                            .h(px(180.0))
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .child(RichEditor::new(
                                "new-task-description-editor",
                                self.board_new_task_description.clone(),
                            )),
                    ),
            )
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
                        &available_providers,
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
                            .id(SharedString::from(format!("board-new-task-model-{}", m_id)))
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
            });

        panel = panel.child(body);

        // Footer
        let title_str = title_input.read(cx).content().to_owned();
        let desc_str = desc_input.read(cx).content().to_owned();
        let can_create = !title_str.trim().is_empty();

        let footer = div()
            .id("new-task-sheet-footer")
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
                Button::new("cancel-new-task", tr!("board.cancel"))
                    .button_style(ButtonStyle::Ghost)
                    .on_press(cx.listener(|this, _, _, cx| {
                        this.board_new_task_modal_open = false;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("create-new-task-submit", tr!("board.create_task"))
                    .button_style(ButtonStyle::Prominent)
                    .enabled(can_create)
                    .kbd_icon("icons/corner-down-left.svg")
                    .on_press(cx.listener(move |this, _, _, cx| {
                        let model = this.board_new_task_model.clone();
                        this.create_board_task(
                            project_id,
                            title_str.clone(),
                            desc_str.clone(),
                            agent,
                            model,
                            cx,
                        );
                    })),
            );

        panel = panel.child(footer);

        let entity = cx.entity().downgrade();
        let entity_close = cx.entity().downgrade();
        let selection_toolbar = EditorSelectionToolbar::new(
            "new-task-selection-toolbar",
            self.board_new_task_description.clone(),
            self.board_new_task_turn_open,
            move |_, cx| {
                let _ = entity.update(cx, |this, cx| {
                    this.board_new_task_turn_open = !this.board_new_task_turn_open;
                    cx.notify();
                });
            },
            move |_, cx| {
                let _ = entity_close.update(cx, |this, cx| {
                    this.board_new_task_turn_open = false;
                    cx.notify();
                });
            },
        )
        .render(window, cx);
        panel = panel.children(selection_toolbar);

        let weak = cx.entity().downgrade();
        sheet(
            "board-new-task-sheet",
            viewport,
            Side::Right,
            extent,
            panel.into_any_element(),
            None,
            move |_event, _window, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.board_new_task_modal_open = false;
                    cx.notify();
                });
            },
        )
    }
}
