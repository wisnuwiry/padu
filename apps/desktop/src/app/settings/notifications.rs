use super::*;
use crate::platform::NotificationPermissionStatus;

impl Padu {
    pub(super) fn render_notifications_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let notifications_enabled = self.state.notifications_enabled;
        let sound_enabled = self.state.notification_sound_enabled;
        let permission = self.notification_permission;

        // --- Permission status badge and action ---
        let (permission_badge, permission_action) = match permission {
            NotificationPermissionStatus::Authorized => {
                let badge = div()
                    .px(px(8.0))
                    .py(px(3.0))
                    .rounded(px(6.0))
                    .bg(theme.success.opacity(0.12))
                    .flex()
                    .items_center()
                    .gap(px(5.0))
                    .text_size(sp(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.success)
                    .child(icon("icons/check.svg", 11.0, theme.success))
                    .child(tr!("notifications.permission_granted"))
                    .into_any_element();
                (badge, None::<AnyElement>)
            }
            NotificationPermissionStatus::Denied => {
                let badge = div()
                    .px(px(8.0))
                    .py(px(3.0))
                    .rounded(px(6.0))
                    .bg(theme.danger.opacity(0.12))
                    .text_size(sp(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.danger)
                    .child(tr!("notifications.permission_denied"))
                    .into_any_element();
                let action = div()
                    .id("open-system-notification-settings")
                    .tab_index(0)
                    .h(px(32.0))
                    .px(px(14.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(theme.border_strong)
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_size(sp(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .hover(|el| el.bg(theme.overlay))
                    .focus_visible(|style| style.border_color(theme.accent))
                    .child(tr!("notifications.open_settings"))
                    .on_click(cx.listener(|_, _, _, _| {
                        crate::platform::open_system_notification_settings();
                    }))
                    .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, _| {
                        if !event.keystroke.modifiers.modified()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            crate::platform::open_system_notification_settings();
                        }
                    }))
                    .into_any_element();
                (badge, Some(action))
            }
            NotificationPermissionStatus::NotDetermined => {
                let badge = div()
                    .px(px(8.0))
                    .py(px(3.0))
                    .rounded(px(6.0))
                    .bg(theme.warning.opacity(0.12))
                    .text_size(sp(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.warning)
                    .child(tr!("notifications.permission_not_determined"))
                    .into_any_element();
                let action = div()
                    .id("request-notification-permission")
                    .tab_index(0)
                    .h(px(32.0))
                    .px(px(14.0))
                    .rounded(px(8.0))
                    .bg(theme.accent)
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_size(sp(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.on_inverse)
                    .hover(|el| el.opacity(0.9))
                    .focus_visible(|style| style.border_1().border_color(theme.accent))
                    .child(tr!("notifications.request_permission"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.request_notification_permission(cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if !event.keystroke.modifiers.modified()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            this.request_notification_permission(cx);
                            cx.stop_propagation();
                        }
                    }))
                    .into_any_element();
                (badge, Some(action))
            }
            NotificationPermissionStatus::Unsupported => {
                let badge = div()
                    .px(px(8.0))
                    .py(px(3.0))
                    .rounded(px(6.0))
                    .bg(theme.overlay)
                    .text_size(sp(12.5))
                    .text_color(theme.text_tertiary)
                    .child(tr!("notifications.permission_unsupported"))
                    .into_any_element();
                (badge, None::<AnyElement>)
            }
        };

        let notifications_toggle = toggle_switch(
            "notifications-enabled-toggle",
            notifications_enabled,
            false,
            theme,
            cx,
            move |this, _, cx| this.set_notifications_enabled(!notifications_enabled, cx),
        );

        let sound_toggle = toggle_switch(
            "notification-sound-toggle",
            sound_enabled,
            false,
            theme,
            cx,
            move |this, _, cx| this.set_notification_sound_enabled(!sound_enabled, cx),
        );

        let preview_button = div()
            .id("preview-notification-sound")
            .tab_index(0)
            .h(px(32.0))
            .px(px(14.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(sp(12.5))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.text_secondary)
            .hover(|el| el.bg(theme.overlay))
            .focus_visible(|style| style.border_color(theme.accent))
            .child(tr!("notifications.preview_sound"))
            .on_click(cx.listener(|_, _, _, _| {
                crate::platform::play_notification_sound();
            }))
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, _| {
                if !event.keystroke.modifiers.modified()
                    && matches!(event.keystroke.key.as_str(), "enter" | "space")
                {
                    crate::platform::play_notification_sound();
                }
            }));

        let test_button = div()
            .id("send-test-notification")
            .tab_index(0)
            .h(px(32.0))
            .px(px(14.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(sp(12.5))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.text)
            .hover(|el| el.bg(theme.overlay))
            .focus_visible(|style| style.border_color(theme.accent))
            .child(tr!("notifications.send_test"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.send_test_notification(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.modified()
                    && matches!(event.keystroke.key.as_str(), "enter" | "space")
                {
                    this.send_test_notification(cx);
                    cx.stop_propagation();
                }
            }));

        div()
            .mt(px(15.0))
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.0))
            // Permission status card
            .child(
                div()
                    .w_full()
                    .px(px(20.0))
                    .py(px(15.0))
                    .rounded(px(13.0))
                    .bg(theme.raised)
                    .child(
                        div()
                            .text_size(sp(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(tr!("notifications.system_permission")),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(sp(12.5))
                            .line_height(sp(18.0))
                            .text_color(theme.text_secondary)
                            .child(tr!("notifications.system_permission_desc")),
                    )
                    .child(
                        div()
                            .mt(px(12.0))
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .child(permission_badge)
                            .children(permission_action),
                    ),
            )
            // Task completion toggle
            .child(
                div()
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
                                    .child(tr!("notifications.task_completion")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("notifications.task_completion_desc")),
                            ),
                    )
                    .child(notifications_toggle),
            )
            // Sound toggle + preview
            .child(
                div()
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
                                    .child(tr!("notifications.sound")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("notifications.sound_desc")),
                            ),
                    )
                    .child(preview_button)
                    .child(sound_toggle),
            )
            // Test notification
            .child(
                div()
                    .w_full()
                    .px(px(20.0))
                    .py(px(15.0))
                    .rounded(px(13.0))
                    .bg(theme.raised)
                    .flex()
                    .items_center()
                    .justify_between()
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
                                    .child(tr!("notifications.test_title")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("notifications.test_desc")),
                            ),
                    )
                    .child(test_button),
            )
            .into_any_element()
    }

    pub(super) fn set_notifications_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.state.notifications_enabled = enabled;
        self.save();
        cx.notify();
    }

    pub(super) fn set_notification_sound_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.state.notification_sound_enabled = enabled;
        self.save();
        cx.notify();
    }

    pub(crate) fn check_and_update_notification_permission(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let (tx, rx) = std::sync::mpsc::channel::<NotificationPermissionStatus>();
            crate::platform::check_notification_permission(move |status| {
                let _ = tx.send(status);
            });
            if let Ok(status) = rx.recv() {
                let _ = this.update(cx, |this, cx| {
                    this.notification_permission = status;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn request_notification_permission(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let (tx, rx) = std::sync::mpsc::channel::<bool>();
            crate::platform::request_notification_permission(move |granted| {
                let _ = tx.send(granted);
            });
            let _ = rx.recv();
            // Re-check permission status after the request resolves
            let (tx2, rx2) = std::sync::mpsc::channel::<NotificationPermissionStatus>();
            crate::platform::check_notification_permission(move |status| {
                let _ = tx2.send(status);
            });
            if let Ok(status) = rx2.recv() {
                let _ = this.update(cx, |this, cx| {
                    this.notification_permission = status;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn send_test_notification(&mut self, cx: &mut Context<Self>) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let tag = format!("padu-test-notification-{timestamp}");
        crate::platform::show_task_notification(
            &tag,
            &tr!("notifications.test_title"),
            &tr!("notifications.test_desc"),
            self.state.notification_sound_enabled,
            cx,
        );
        self.show_success_toast(tr!("notifications.send_test"));
    }
}
