use super::*;
use crate::ui::Switch;
use crate::ui::button::{Button, ButtonStyle};

impl Padu {
    pub(super) fn render_general_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let updater_available = cx
            .try_global::<crate::updater::UpdaterState>()
            .is_some_and(|updater| updater.0.is_some());
        let analytics_enabled = self.state.analytics_enabled;
        let weak = cx.entity().downgrade();
        let analytics_toggle = Switch::new("anonymous-analytics-toggle", analytics_enabled)
            .on_toggle(move |enabled, _, cx| {
                let _ = weak.update(cx, |this, cx| this.set_analytics_enabled(enabled, cx));
            });
        div()
            .child(
                div()
                    .mt(px(15.0))
                    .w_full()
                    .px(px(20.0))
                    .py(px(14.0))
                    .rounded(px(13.0))
                    .bg(theme.raised)
                    .child(
                        div()
                            .text_size(sp(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(tr!("settings.local_by_default")),
                    )
                    .child(
                        div()
                            .mt(px(5.0))
                            .text_size(sp(12.5))
                            .line_height(sp(18.0))
                            .text_color(theme.text_secondary)
                            .child(tr!("settings.local_by_default_description")),
                    ),
            )
            .child(
                div()
                    .mt(px(15.0))
                    .w_full()
                    .min_h(px(60.0))
                    .px(px(20.0))
                    .py(px(12.0))
                    .rounded(px(13.0))
                    .bg(theme.raised)
                    .flex()
                    .items_center()
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
                                    .child(tr!("settings.share_anonymous_usage_data")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("settings.share_anonymous_usage_data_description")),
                            ),
                    )
                    .child(analytics_toggle),
            )
            .when(updater_available, |column| {
                let enabled = self.automatic_updates_enabled;
                let weak = cx.entity().downgrade();
                let toggle = Switch::new("automatic-updates-toggle", enabled).on_toggle(
                    move |enabled, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.set_automatic_updates_enabled(enabled, cx)
                        });
                    },
                );
                column.child(
                    div()
                        .mt(px(15.0))
                        .w_full()
                        .min_h(px(60.0))
                        .px(px(20.0))
                        .py(px(12.0))
                        .rounded(px(13.0))
                        .bg(theme.raised)
                        .flex()
                        .items_center()
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
                                        .child(tr!("settings.automatic_updates")),
                                )
                                .child(
                                    div()
                                        .mt(px(5.0))
                                        .text_size(sp(12.5))
                                        .line_height(sp(18.0))
                                        .text_color(theme.text_secondary)
                                        .child(tr!("settings.automatic_updates_description")),
                                ),
                        )
                        .child(toggle),
                )
            })
            .child(
                div()
                    .mt(px(15.0))
                    .w_full()
                    .min_h(px(60.0))
                    .px(px(20.0))
                    .py(px(12.0))
                    .rounded(px(13.0))
                    .bg(theme.raised)
                    .flex()
                    .items_center()
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
                                    .child(tr!("onboarding.command_title")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("onboarding.welcome_subtitle")),
                            ),
                    )
                    .child(
                        Button::new("replay-onboarding-btn", tr!("onboarding.replay_button"))
                            .button_style(ButtonStyle::Ghost)
                            .on_press(cx.listener(|this, _, window, cx| {
                                this.settings_page = None;
                                this.open_onboarding(window, cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    fn set_analytics_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.state.analytics_enabled = enabled;
        self.analytics.set_enabled(enabled);
        self.save();
        cx.notify();
    }

    fn set_automatic_updates_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.automatic_updates_enabled = enabled;
        if let Some(updater) = cx
            .try_global::<crate::updater::UpdaterState>()
            .and_then(|updater| updater.0.as_ref())
        {
            updater.set_automatically_checks_for_updates(enabled);
        }
        cx.notify();
    }
}
