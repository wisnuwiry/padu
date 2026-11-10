//! The settings page for archived conversations.

use super::*;
use crate::ui::button::{Button, ButtonRole, ButtonStyle};
use bezel::ui::widgets::Scaffolding;

impl Padu {
    fn archived_search_query(&self, cx: &App) -> String {
        self.archived_search
            .read(cx)
            .content()
            .trim()
            .to_lowercase()
    }

    pub(super) fn render_archived_settings(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let bezel_theme = bezel::theme::Theme::of(cx);
        let control_radius = bezel::theme::Theme::control_radius();
        let query = self.archived_search_query(cx);
        let mut archived = self
            .state
            .sessions
            .iter()
            .filter(|session| session.archived_at.is_some())
            .filter(|session| {
                if query.is_empty() {
                    return true;
                }
                let project = self
                    .state
                    .projects
                    .iter()
                    .find(|project| project.id == session.project_id);
                let project_name = project
                    .map(|project| project.display_name())
                    .unwrap_or_default();
                let project_path = project
                    .map(|project| project.path.to_string_lossy().into_owned())
                    .unwrap_or_default();
                session.display_title().to_lowercase().contains(&query)
                    || session.id.to_string().contains(&query)
                    || project_name.to_lowercase().contains(&query)
                    || project_path.to_lowercase().contains(&query)
            })
            .collect::<Vec<_>>();
        archived.sort_by_key(|session| std::cmp::Reverse(session.archived_at.unwrap_or_default()));
        let empty = archived.is_empty();
        let has_query = !query.is_empty();
        let total_count = archived.len();
        let mut group = bezel_theme.group_box().mt(px(16.0)).w_full();
        for (index, session) in archived.into_iter().enumerate() {
            if index > 0 {
                group = group.child(div().mx(px(16.0)).h(px(1.0)).bg(theme.border));
            }
            let session_id = session.id;
            let project = self
                .state
                .projects
                .iter()
                .find(|project| project.id == session.project_id);
            let project_name = project
                .map(|project| project.display_name())
                .unwrap_or_else(|| tr!("project.no_project_name"));
            let project_path = project
                .map(|project| project.path.to_string_lossy().into_owned())
                .unwrap_or_default();
            let row = div()
                .id(SharedString::from(format!("archived-session-{session_id}")))
                .w_full()
                .min_h(px(66.0))
                .px(px(16.0))
                .py(px(10.0))
                .rounded(px(control_radius))
                .flex()
                .items_center()
                .gap(px(12.0))
                .hover(|element| element.bg(theme.sidebar_item_background))
                .child(icon("icons/archive.svg", 14.0, theme.text_tertiary))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .text_size(sp(13.0))
                                .text_color(theme.text)
                                .child(SharedString::from(session.display_title())),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(sp(11.0))
                                .text_color(theme.text_secondary)
                                .child(SharedString::from(format!(
                                    "{} · {}",
                                    tr!("settings.archived_project"),
                                    project_name
                                ))),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(sp(10.0))
                                .text_color(theme.text_tertiary)
                                .child(SharedString::from(if project_path.is_empty() {
                                    session.id.to_string()
                                } else {
                                    format!(
                                        "{} · {}",
                                        tr!("settings.archived_project_path"),
                                        project_path
                                    )
                                })),
                        ),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("restore-archived-{session_id}")),
                        tr!("settings.restore"),
                    )
                    .button_style(ButtonStyle::Ghost)
                    .on_press(cx.listener(
                        move |this, _, _, cx| {
                            this.set_session_archived(session_id, false, cx);
                            this.select_session(session_id, cx);
                        },
                    )),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("delete-archived-{session_id}")),
                        tr!("common.remove"),
                    )
                    .button_style(ButtonStyle::Ghost)
                    .role(ButtonRole::Destructive)
                    .on_press(cx.listener(
                        move |this, _, window, cx| {
                            this.confirm_delete_session(session_id, window, cx);
                        },
                    )),
                );
            group = group.child(row);
        }
        div()
            .pt(px(20.0))
            .w_full()
            .text_size(sp(13.0))
            .text_color(theme.text_secondary)
            .child(
                div()
                    .child(SharedString::from(tr!("settings.archived_description")))
                    .child(
                        div().mt(px(12.0)).child(
                            TextField::new("archived-search-field", self.archived_search.clone())
                                .icon("icons/search.svg", 13.0),
                        ),
                    ),
            )
            .when(empty, |element| {
                element.child(div().mt(px(16.0)).text_color(theme.text_tertiary).child(
                    if has_query {
                        tr!("settings.archived_no_matches")
                    } else {
                        tr!("settings.archived_empty")
                    },
                ))
            })
            .when(!empty, |element| element.child(group))
    }
}
