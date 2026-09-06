use super::*;

impl Padu {
    pub(super) fn render_general_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let updater_available = cx
            .try_global::<crate::updater::UpdaterState>()
            .is_some_and(|updater| updater.0.is_some());
        let analytics_enabled = self.state.analytics_enabled;
        let analytics_toggle = toggle_switch(
            "anonymous-analytics-toggle",
            analytics_enabled,
            false,
            theme,
            cx,
            move |this, _, cx| this.set_analytics_enabled(!analytics_enabled, cx),
        );
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
                let toggle = toggle_switch(
                    "automatic-updates-toggle",
                    enabled,
                    false,
                    theme,
                    cx,
                    move |this, _, cx| this.set_automatic_updates_enabled(!enabled, cx),
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
                        div()
                            .id("replay-onboarding-btn")
                            .tab_index(0)
                            .focus_visible(|style| style.border_1().border_color(theme.accent))
                            .h(px(32.0))
                            .px(px(14.0))
                            .rounded(px(8.0))
                            .bg(theme.overlay_strong)
                            .text_size(sp(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|el| el.bg(theme.overlay))
                            .active(|el| el.opacity(0.85))
                            .child(tr!("onboarding.replay_button"))
                            .on_click(cx.listener(|this, _, window, cx| {
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
