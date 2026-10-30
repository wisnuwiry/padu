use super::*;
use crate::ui::button::{Button, ButtonStyle};

impl Padu {
    pub(super) fn render_about_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let version = env!("CARGO_PKG_VERSION");
        let status = self.updater_status;
        let is_checking = self.updater_checking;
        let is_updating = status == crate::updater::UpdateStatus::Updating;
        let is_loading = is_checking || is_updating;

        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            // Unified About + Version + Updates Card
            .child(
                div()
                    .mt(px(15.0))
                    .w_full()
                    .px(px(20.0))
                    .py(px(18.0))
                    .rounded(px(13.0))
                    .bg(theme.raised)
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(16.0))
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .w(px(60.0))
                                    .h(px(60.0))
                                    .rounded(px(15.0))
                                    .bg(gpui::black())
                                    .border_1()
                                    .border_color(theme.border)
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon("icons/logo.svg", 36.0, gpui::white())),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .text_size(sp(16.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme.text)
                                                    .child("Padu"),
                                            )
                                            .child(
                                                div()
                                                    .px(px(6.0))
                                                    .py(px(1.5))
                                                    .rounded(px(5.0))
                                                    .bg(theme.surface)
                                                    .border_1()
                                                    .border_color(theme.border)
                                                    .font_family(".SystemUIFontMonospaced")
                                                    .text_size(sp(11.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.text_secondary)
                                                    .child(format!("v{version}")),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .mt(px(3.0))
                                            .text_size(sp(12.0))
                                            .line_height(sp(17.0))
                                            .text_color(theme.text_secondary)
                                            .child(tr!("about.tagline")),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap(px(4.0))
                            .flex_none()
                            .child(if status == crate::updater::UpdateStatus::Available {
                                Button::new("install-update-btn", tr!("about.install_update"))
                                    .button_style(ButtonStyle::Prominent)
                                    .icon("icons/cloud-upload.svg")
                                    .on_press(cx.listener(|this, _, _, cx| {
                                        this.start_available_update(cx);
                                    }))
                            } else if is_loading {
                                Button::new("updating-btn", tr!("about.checking_for_updates"))
                                    .button_style(ButtonStyle::Ghost)
                                    .leading(motion::spin_slow(icon(
                                        "icons/loader-circle.svg",
                                        13.0,
                                        theme.text_secondary,
                                    )))
                                    .enabled(false)
                            } else {
                                Button::new("check-for-updates-btn", tr!("about.check_for_updates"))
                                    .button_style(ButtonStyle::Ghost)
                                    .icon("icons/rotate-cw.svg")
                                    .on_press(cx.listener(|this, _, _, cx| {
                                        this.check_for_updates(cx);
                                    }))
                            })
                            .child(
                                div()
                                    .text_size(sp(11.5))
                                    .text_color(
                                        if status == crate::updater::UpdateStatus::Available {
                                            theme.accent
                                        } else {
                                            theme.text_tertiary
                                        },
                                    )
                                    .child(if is_loading {
                                        tr!("about.checking_for_updates").to_string()
                                    } else {
                                        match status {
                                            crate::updater::UpdateStatus::Updating => {
                                                tr!("about.checking_for_updates").to_string()
                                            }
                                            crate::updater::UpdateStatus::Available => {
                                                tr!("about.update_available").to_string()
                                            }
                                            crate::updater::UpdateStatus::Idle => {
                                                tr!("about.up_to_date").to_string()
                                            }
                                        }
                                    }),
                            ),
                    ),
            )
            // Website, GitHub, Changelog & Sponsor single row of plain buttons, aligned center
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .pt(px(2.0))
                    .child(
                        Button::new("about-changelog-btn", tr!("about.view_changelog"))
                            .button_style(ButtonStyle::Ghost)
                            .icon("icons/sparkle.svg")
                            .on_press(cx.listener(|this, _, window, cx| {
                                this.open_whats_new(window, cx);
                            })),
                    )
                    .child(
                        Button::new("about-website-btn", tr!("about.website"))
                            .button_style(ButtonStyle::Ghost)
                            .icon("icons/globe.svg")
                            .trailing(icon("icons/arrow-up-right.svg", 11.0, theme.text_tertiary))
                            .on_press(|_, _, cx| {
                                cx.open_url("https://padu.dev");
                            }),
                    )
                    .child(
                        Button::new("about-github-btn", "GitHub")
                            .button_style(ButtonStyle::Ghost)
                            .icon("icons/github.svg")
                            .trailing(icon("icons/arrow-up-right.svg", 11.0, theme.text_tertiary))
                            .on_press(|_, _, cx| {
                                cx.open_url("https://github.com/wisnuwiry/padu");
                            }),
                    )
                    .child(
                        Button::new("about-sponsor-btn", "Sponsor")
                            .button_style(ButtonStyle::Ghost)
                            .leading(icon("icons/heart.svg", 14.0, theme.danger))
                            .trailing(icon("icons/arrow-up-right.svg", 11.0, theme.text_tertiary))
                            .on_press(|_, _, cx| {
                                cx.open_url("https://github.com/sponsors/wisnuwiry");
                            }),
                    ),
            )
            .into_any_element()
    }
}
