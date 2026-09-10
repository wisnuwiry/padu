use super::*;

impl Padu {
    pub(crate) fn render_right_panel_toggle(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = Theme::current(cx);
        div()
            .id("toggle-right-panel")
            .w(px(26.0))
            .h(px(26.0))
            .flex_none()
            .rounded(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|element| element.bg(theme.overlay))
            .active(|element| element.bg(theme.overlay_strong))
            .child(icon("icons/panel-right.svg", 14.0, theme.text_tertiary))
            .tooltip(Tooltip::with_shortcut(
                tr!("right_panel.toggle"),
                crate::platform::primary_shortcut("⇧⌘B", "Ctrl+Shift+B"),
            ))
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.set_right_panel_visible(!this.right_panel_visible, cx);
            }))
    }

    pub(crate) fn render_right_panel(
        &mut self,
        width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = Theme::current(cx);
        let active_terminal_id = self
            .active_right_panel_surface()
            .and_then(RightPanelSurface::terminal_id);
        if self.right_panel_pending_terminal_focus == active_terminal_id
            && let Some(terminal_id) = active_terminal_id
            && let Some(terminal) = self.right_panel_terminals.get(&terminal_id)
        {
            let focus_handle = terminal.read(cx).focus_handle(cx);
            window.focus(&focus_handle, cx);
            self.right_panel_pending_terminal_focus = None;
        }
        let body = match self.active_right_panel_surface().cloned() {
            None => self.render_right_panel_chooser(cx).into_any_element(),
            Some(RightPanelSurface::BackgroundWork { key, .. }) => self
                .render_background_work_surface(&key, cx)
                .into_any_element(),
            Some(RightPanelSurface::Files) => self
                .render_right_panel_files(width, window, cx)
                .into_any_element(),
            Some(RightPanelSurface::Diff) => self
                .render_right_panel_diff(width, window, cx)
                .into_any_element(),
            Some(RightPanelSurface::Terminal(terminal_id)) => self
                .right_panel_terminals
                .get(&terminal_id)
                .cloned()
                .inspect(|terminal| {
                    terminal.update(cx, |terminal, _| terminal.set_panel_width(width));
                })
                .map(IntoElement::into_any_element)
                .unwrap_or_else(|| {
                    self.render_right_panel_empty_message(
                        tr!("right_panel.terminal_unavailable"),
                        tr!("right_panel.terminal_unavailable_description"),
                        cx,
                    )
                    .into_any_element()
                }),
            Some(RightPanelSurface::File(path)) => self
                .render_right_panel_file(path, width, window, cx)
                .into_any_element(),
            Some(RightPanelSurface::Browser(browser_id)) => {
                let browser = self.ensure_right_panel_browser(browser_id, window, cx);
                if self
                    .right_panel_pending_browser_focus
                    .take_if(|pending| *pending == browser_id)
                    .is_some()
                {
                    browser.update(cx, |view, cx| view.focus_default(window, cx));
                }
                browser.into_any_element()
            }
        };

        div()
            .id("right-panel")
            .when(!self.right_panel_fullscreen_active(), |element| {
                element
                    .w(px(width))
                    .flex_none()
                    .border_l_1()
                    .border_color(theme.border_strong)
            })
            .when(self.right_panel_fullscreen_active(), |element| {
                element.flex_1().min_w_0()
            })
            .h_full()
            .flex()
            .flex_col()
            .min_w_0()
            .bg(theme.surface)
            .relative()
            .child(self.render_right_panel_header(window, cx))
            .child(body)
            .when(!self.right_panel_fullscreen_active(), |element| {
                element.child(self.render_panel_resize_handle(
                    "right-panel-resize-handle",
                    PanelResizeTarget::RightPanel,
                    cx,
                ))
            })
    }

    pub(crate) fn ensure_right_panel_browser(
        &mut self,
        browser_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<crate::browser::BrowserView> {
        if let Some(browser) = self.right_panel_browsers.get(&browser_id) {
            return browser.clone();
        }
        let browser = cx.new(|cx| crate::browser::BrowserView::new(window, cx));
        // Tab titles and toolbar state live on the browser entity; the panel
        // chrome re-renders when they move.
        cx.observe(&browser, |_, _, cx| cx.notify()).detach();
        self.right_panel_browsers
            .insert(browser_id, browser.clone());
        browser
    }

    /// Drop browser views whose tab no longer exists in any session.
    pub(crate) fn retain_right_panel_browsers(&mut self, cx: &mut Context<Self>) {
        let retained_browser_ids = self
            .right_panel_surfaces
            .iter()
            .filter_map(RightPanelSurface::browser_id)
            .chain(self.right_panel_session_states.values().flat_map(|state| {
                state
                    .surfaces
                    .iter()
                    .filter_map(RightPanelSurface::browser_id)
            }))
            .collect::<HashSet<_>>();
        let stale_ids = self
            .right_panel_browsers
            .keys()
            .filter(|browser_id| !retained_browser_ids.contains(browser_id))
            .copied()
            .collect::<Vec<_>>();
        for browser_id in stale_ids {
            self.destroy_right_panel_browser(browser_id, cx);
        }
    }

    pub(crate) fn destroy_right_panel_browser(&mut self, browser_id: Uuid, cx: &mut Context<Self>) {
        let Some(browser) = self.right_panel_browsers.remove(&browser_id) else {
            return;
        };
        browser.update(cx, |browser, cx| browser.shutdown(cx));
    }

    /// Whether any GPUI overlay that could float above the right panel is
    /// open. The native webview always draws over GPUI, so while this holds
    /// the live page swaps for a frozen snapshot.
    pub(crate) fn any_overlay_open(&self, cx: &App) -> bool {
        self.menus.borrow().values().any(ContextMenuHandle::is_open)
            || self.command_palette.is_open()
            || self.task_switcher.is_open()
            || self.commit_dialog.is_some()
            || self.image_preview.is_some()
            || self.composer.read(cx).context_menu_open(cx)
            || self
                .right_panel_browsers
                .values()
                .any(|browser| browser.read(cx).overlay_open(cx))
    }

    /// Once per frame, from the very top of the app's render: push down to
    /// every browser whether its native view belongs on screen. This is the
    /// single authority — tab switches, panel toggles, session switches, the
    /// settings page and overlay menus all funnel through here, so a webview
    /// can never linger over unrelated UI.
    pub(crate) fn sync_browser_webviews(&mut self, cx: &mut Context<Self>) {
        if self.right_panel_browsers.is_empty() {
            return;
        }
        // With the scene overlay compositing GPUI's deferred draws above
        // native views, open menus never occlude the webview — the snapshot
        // swap is purely the fallback for a window where enabling it failed.
        let overlay_open = !self.scene_overlay_enabled && self.any_overlay_open(cx);
        // A webview composites above the GPUI scene, so the panel's clip does
        // not apply to it: shown mid-slide it would hang over the transcript
        // at full width. Keep it down until the panel has finished moving.
        // Fullscreen conversation shows the transcript, so its browser stays
        // down the same way a hidden panel does.
        let logically_active_browser = if self.settings_page.is_none() {
            self.active_right_panel_surface()
                .and_then(RightPanelSurface::browser_id)
        } else {
            None
        };
        let visible_browser = if self.settings_page.is_none()
            && self.right_panel_visible
            && self.right_panel_slide.is_none()
            && !(self.right_panel_fullscreen_active() && self.right_panel_fullscreen_conversation)
        {
            logically_active_browser
        } else {
            None
        };
        for (browser_id, browser) in &self.right_panel_browsers {
            let logically_active = logically_active_browser == Some(*browser_id);
            let surface_visible = visible_browser == Some(*browser_id);
            browser.update(cx, |view, cx| {
                view.sync_native_state(logically_active, surface_visible, overlay_open, cx);
            });
        }
    }

    pub(crate) fn ensure_right_panel_terminal(
        &mut self,
        terminal_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        if self.daemon.is_remote() {
            // A desktop PTY would interpret the daemon's cwd on the wrong
            // machine. Keep the surface unavailable until the protocol grows
            // a daemon-owned streaming terminal.
            self.right_panel_terminals.remove(&terminal_id);
            return;
        }
        let Some(working_directory) = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf)
        else {
            self.right_panel_terminals.remove(&terminal_id);
            return;
        };
        let matches_project = self
            .right_panel_terminals
            .get(&terminal_id)
            .is_some_and(|terminal| terminal.read(cx).working_directory() == working_directory);
        if !matches_project {
            self.right_panel_terminals.insert(
                terminal_id,
                cx.new(|cx| TerminalView::new(working_directory.clone(), cx)),
            );
        }
    }

    pub(crate) fn ensure_right_panel_terminals(&mut self, cx: &mut Context<Self>) {
        let active_terminal_ids = self
            .right_panel_surfaces
            .iter()
            .filter_map(RightPanelSurface::terminal_id)
            .collect::<Vec<_>>();
        let retained_terminal_ids = active_terminal_ids
            .iter()
            .copied()
            .chain(self.right_panel_session_states.values().flat_map(|state| {
                state
                    .surfaces
                    .iter()
                    .filter_map(RightPanelSurface::terminal_id)
            }))
            .collect::<HashSet<_>>();
        self.right_panel_terminals
            .retain(|terminal_id, _| retained_terminal_ids.contains(terminal_id));
        for terminal_id in active_terminal_ids {
            self.ensure_right_panel_terminal(terminal_id, cx);
        }
    }

    pub(crate) fn render_right_panel_header(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = Theme::current(cx);
        let fullscreen = self.right_panel_fullscreen_active();
        let showing_conversation = fullscreen && self.right_panel_fullscreen_conversation;
        let active_surface = self.right_panel_active_surface;
        let mut tabs = div()
            .id("right-panel-tabs")
            .h_full()
            .min_w_0()
            .flex_1()
            .flex()
            .items_center()
            .gap(px(4.0))
            .overflow_x_scroll()
            .track_scroll(&self.right_panel_tabs_scroll_handle);
        // Fullscreen surfaces read like tabs with Conversation first, so the
        // transcript stays one ←/→ step away from every surface.
        if fullscreen {
            let active = showing_conversation;
            tabs = tabs.child(
                div()
                    .id("right-panel-tab-conversation")
                    .h(px(28.0))
                    .min_w(px(100.0))
                    .max_w(px(176.0))
                    .px(px(8.0))
                    .rounded(px(6.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .when(active, |element| element.bg(theme.overlay_strong))
                    .when(!active, |element| {
                        element.hover(|element| element.bg(theme.overlay))
                    })
                    .child(icon("icons/compose.svg", 13.0, theme.text_secondary))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .text_size(sp(12.5))
                            .text_color(if active {
                                theme.text
                            } else {
                                theme.text_secondary
                            })
                            .child(tr!("right_panel.conversation")),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_right_panel_fullscreen_conversation(Some(window), cx);
                    })),
            );
        }
        for (index, surface) in self.right_panel_surfaces.iter().cloned().enumerate() {
            let active = !showing_conversation && active_surface == Some(index);
            let dirty = self.right_panel_surface_is_dirty(&surface);
            let label = SharedString::from(match &surface {
                // Browser tabs read like browser tabs: the page title once
                // known, the address until then.
                RightPanelSurface::Browser(browser_id) => self
                    .right_panel_browsers
                    .get(browser_id)
                    .and_then(|browser| browser.read(cx).tab_label())
                    .unwrap_or_else(|| surface.label()),
                _ => {
                    right_panel_tab_label(&surface, self.right_panel_files_selected_path.as_deref())
                }
            });
            let icon_path =
                right_panel_tab_icon(&surface, self.right_panel_files_selected_path.as_deref());
            let uses_file_icon = matches!(&surface, RightPanelSurface::File(_))
                || matches!(&surface, RightPanelSurface::Files)
                    && self.right_panel_files_selected_path.is_some();
            let close_weak = cx.entity().downgrade();
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("right-panel-tab-{index}")))
                    .h(px(28.0))
                    .min_w(px(100.0))
                    .max_w(px(176.0))
                    .px(px(8.0))
                    .rounded(px(6.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_mouse_down(MouseButton::Middle, {
                        let close_weak = close_weak.clone();
                        move |_, _, cx| {
                            cx.stop_propagation();
                            let _ = close_weak.update(cx, |this, cx| {
                                this.close_right_panel_surface(index, cx);
                            });
                        }
                    })
                    .when(active, |element| element.bg(theme.overlay_strong))
                    .when(!active, |element| {
                        element.hover(|element| element.bg(theme.overlay))
                    })
                    .child(if uses_file_icon {
                        file_icon(icon_path, 13.0).into_any_element()
                    } else {
                        icon(icon_path, 13.0, theme.text_secondary).into_any_element()
                    })
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .text_size(sp(12.5))
                            .text_color(if active {
                                theme.text
                            } else {
                                theme.text_secondary
                            })
                            .child(label),
                    )
                    .when(dirty, |element| {
                        element.child(
                            div()
                                .id(SharedString::from(format!("right-panel-tab-dirty-{index}")))
                                .size(px(7.0))
                                .flex_none()
                                .rounded_full()
                                .bg(theme.warning)
                                .tooltip(|window, cx| {
                                    Tooltip::new(tr!(
                                        "files.unsaved_changes",
                                        shortcut =
                                            crate::platform::primary_shortcut("⌘S", "Ctrl+S")
                                    ))
                                    .build(window, cx)
                                }),
                        )
                    })
                    .child(
                        div()
                            .id(SharedString::from(format!("close-right-panel-tab-{index}")))
                            .w(px(16.0))
                            .h(px(16.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|element| element.bg(theme.overlay_strong))
                            .child(icon("icons/x.svg", 10.0, theme.text_tertiary))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.close_right_panel_surface(index, cx);
                            })),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.right_panel_active_surface = Some(index);
                        this.right_panel_fullscreen_conversation = false;
                        this.reveal_right_panel_tab(index);
                        this.focus_active_surface(window, cx);
                        cx.notify();
                    })),
            );
        }
        tabs = tabs
            .child(div().w(px(TAB_SCROLL_FADE_WIDTH)).h(px(1.0)).flex_none())
            .when(fullscreen, |element| element.justify_center());

        let left_window_controls = (fullscreen && !self.sidebar_visible)
            .then(|| {
                self.render_client_window_controls(
                    super::window_chrome::WindowControlSide::Left,
                    window,
                    cx,
                )
            })
            .flatten();

        let mut header = div()
            .id("right-panel-header")
            .h(px(48.0))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.0))
            .children(left_window_controls)
            .pl(if fullscreen && !self.sidebar_visible {
                if cfg!(target_os = "macos") {
                    px(0.0)
                } else {
                    px(10.0)
                }
            } else {
                px(10.0)
            })
            .pr(px(14.0))
            .when(fullscreen && !self.sidebar_visible, |element| {
                element
                    .child(
                        self.window_drag_region(
                            div()
                                .id("right-panel-traffic-light-drag-region")
                                .w(px((TRAFFIC_LIGHT_CLEARANCE - 8.0).max(0.0)))
                                .h_full()
                                .flex_none(),
                            cx,
                        ),
                    )
                    .child(self.render_sidebar_navigation_controls(showing_conversation, cx))
            })
            .when(showing_conversation, |element| {
                let session = self.selected_session();
                let title = session
                    .map(sidebar::localized_session_title)
                    .unwrap_or_else(|| tr!("session.new_task"));
                let agent_preset_label = session
                    .filter(|session| {
                        session.provider == ProviderKind::DeepSeek && session.has_started()
                    })
                    .and_then(|session| self.agent_preset_label_for_session(session));
                element.child(
                    div()
                        .id("fullscreen-conversation-title")
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .min_w_0()
                        .max_w(px(220.0))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(sp(13.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(SharedString::from(title)),
                        )
                        .children(agent_preset_label.map(|label| {
                            div()
                                .h(px(20.0))
                                .max_w(px(120.0))
                                .px(px(6.0))
                                .rounded(px(4.0))
                                .flex_none()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .bg(theme.overlay)
                                .text_size(sp(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text_secondary)
                                .child(icon("icons/bot.svg", 10.0, theme.text_tertiary))
                                .child(div().min_w_0().truncate().child(SharedString::from(label)))
                        })),
                )
            })
            .child(
                div()
                    .relative()
                    .h_full()
                    .min_w_0()
                    .flex_1()
                    .overflow_hidden()
                    .when(fullscreen, |element| {
                        element.flex().items_center().justify_center()
                    })
                    .child(tabs)
                    .when_some(self.right_panel_pending_tab_reveal, |element, tab_index| {
                        element.child(tab_scroll_reveal_guard(
                            self.right_panel_tabs_scroll_handle.clone(),
                            tab_index,
                            cx.entity().downgrade(),
                        ))
                    })
                    .child(tab_scroll_fade(
                        self.right_panel_tabs_scroll_handle.clone(),
                        TabScrollFadeSide::Left,
                        theme.surface,
                    ))
                    .child(tab_scroll_fade(
                        self.right_panel_tabs_scroll_handle.clone(),
                        TabScrollFadeSide::Right,
                        theme.surface,
                    )),
            );

        if showing_conversation {
            header = header.child(self.render_background_work_summary(cx));
        }

        if !self.right_panel_surfaces.is_empty() {
            let weak = cx.entity().downgrade();
            let existing_surfaces = self.right_panel_surfaces.clone();
            let has_project = self.active_project().is_some();
            let mut options = vec![
                RightPanelSurface::new_browser(),
                RightPanelSurface::new_terminal(),
            ];
            if has_project {
                options.push(RightPanelSurface::Files);
                options.push(RightPanelSurface::Diff);
            }
            let handle = self.menu_handle("add-right-panel-surface", cx);
            header = header.child(
                div()
                    .flex_none()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(dropdown_menu(
                        icon_button("add-right-panel-surface", "icons/plus.svg", theme),
                        "add-right-panel-surface-menu",
                        &handle,
                        MenuAlign::BelowRight,
                        move |_| {
                            options
                                .clone()
                                .into_iter()
                                .map(|surface| {
                                    let weak = weak.clone();
                                    let open_surface = surface.clone();
                                    let already_open =
                                        reusable_surface_index(&existing_surfaces, &surface)
                                            .is_some();
                                    MenuItem::new(surface.label(), move |_, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.open_right_panel_surface(open_surface.clone(), cx);
                                        });
                                    })
                                    .icon(surface.icon_path())
                                    .selected(already_open)
                                })
                                .collect()
                        },
                    )),
            );
        }

        // Fullscreen expand: only while an expandable surface (browser,
        // terminal, files, review) is open. Keyboard reachable via tab stop
        // plus ⌘J; ←/→ then cycle Conversation and surfaces like tabs.
        if self.right_panel_can_expand() {
            let fullscreen = self.right_panel_fullscreen_active();
            let focus = self.transcript_control_focus("right-panel-fullscreen-toggle", cx);
            let (icon_path, label) = if fullscreen {
                ("icons/minimize.svg", tr!("right_panel.collapse"))
            } else {
                ("icons/maximize.svg", tr!("right_panel.expand"))
            };
            header = header.child(
                div()
                    .id("toggle-right-panel-fullscreen")
                    .track_focus(&focus)
                    .tab_index(0)
                    .size(px(22.0))
                    .rounded(px(6.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .focus_visible(|style| style.border_1().border_color(theme.accent))
                    .hover(|element| element.bg(theme.overlay))
                    .active(|element| element.bg(theme.overlay_strong))
                    .child(icon(icon_path, 13.0, theme.text_tertiary))
                    .tooltip(Tooltip::with_shortcut(
                        label.clone(),
                        crate::platform::primary_shortcut("⌘J", "Ctrl+J"),
                    ))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.toggle_right_panel_fullscreen_action(
                            &ToggleRightPanelFullscreen,
                            window,
                            cx,
                        );
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            cx.stop_propagation();
                            this.toggle_right_panel_fullscreen_action(
                                &ToggleRightPanelFullscreen,
                                window,
                                cx,
                            );
                        }
                    })),
            );
        }

        self.window_drag_region(
            header
                .when(!fullscreen, |header| {
                    header.child(self.render_right_panel_toggle(cx))
                })
                .children(self.render_client_window_controls(
                    super::window_chrome::WindowControlSide::Right,
                    window,
                    cx,
                )),
            cx,
        )
    }

    pub(crate) fn render_right_panel_chooser(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = Theme::current(cx);
        let has_project = self.active_project().is_some();
        div()
            .id("right-panel-chooser")
            .flex_1()
            .min_h_0()
            .flex()
            .items_center()
            .justify_center()
            .px(px(20.0))
            .pb(px(32.0))
            .child(
                div()
                    .w_full()
                    .max_w(px(420.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        div()
                            .text_size(sp(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(tr!("right_panel.open_surface")),
                    )
                    .child(
                        div()
                            .mt(px(5.0))
                            .text_size(sp(12.5))
                            .text_color(theme.text_tertiary)
                            .child(tr!("right_panel.choose_surface")),
                    )
                    .child(
                        div()
                            .mt(px(18.0))
                            .w_full()
                            .flex()
                            .gap(px(8.0))
                            .child(self.render_right_panel_card(
                                RightPanelSurface::new_browser(),
                                tr!("right_panel.browser_description"),
                                cx,
                            ))
                            .child(self.render_right_panel_card(
                                RightPanelSurface::new_terminal(),
                                tr!("right_panel.terminal_description"),
                                cx,
                            )),
                    )
                    .when(has_project, |chooser| {
                        chooser.child(
                            div()
                                .mt(px(8.0))
                                .w_full()
                                .flex()
                                .gap(px(8.0))
                                .child(self.render_right_panel_card(
                                    RightPanelSurface::Files,
                                    tr!("right_panel.files_description"),
                                    cx,
                                ))
                                .child(self.render_right_panel_card(
                                    RightPanelSurface::Diff,
                                    tr!("right_panel.diff_description"),
                                    cx,
                                )),
                        )
                    }),
            )
    }

    pub(crate) fn render_right_panel_card(
        &self,
        surface: RightPanelSurface,
        description: String,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = Theme::current(cx);
        let icon_path = surface.icon_path();
        let label = surface.label();
        let shortcut = surface.shortcut();
        div()
            .id(SharedString::from(format!(
                "right-panel-card-{}",
                label.to_lowercase()
            )))
            .h(px(114.0))
            .flex_1()
            .min_w_0()
            .p(px(14.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.composer)
            .flex()
            .flex_col()
            .items_start()
            .cursor_pointer()
            .hover(|element| element.bg(theme.raised).border_color(theme.text_ghost))
            .active(|element| element.bg(theme.overlay_strong))
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(icon(icon_path, 18.0, theme.text_secondary))
                    .when_some(shortcut, |row, shortcut| {
                        row.child(kbd_badge(shortcut, &theme))
                    }),
            )
            .child(
                div()
                    .mt(px(12.0))
                    .text_size(sp(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(label),
            )
            .child(
                div()
                    .mt(px(4.0))
                    .text_size(sp(12.5))
                    .line_height(sp(15.0))
                    .text_color(theme.text_tertiary)
                    .whitespace_normal()
                    .line_clamp(2)
                    .text_overflow(gpui::TextOverflow::Truncate("...".into()))
                    .child(description),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_right_panel_surface(surface.clone(), cx);
            }))
    }

    pub(crate) fn render_right_panel_empty_message(
        &self,
        title: String,
        description: String,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = Theme::current(cx);
        div()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    let focus = this.transcript_control_focus("right-panel-diff", cx);
                    window.focus(&focus, cx);
                }),
            )
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .pb(px(32.0))
            .child(
                div()
                    .text_size(sp(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(title),
            )
            .child(
                div()
                    .mt(px(6.0))
                    .max_w(px(300.0))
                    .text_center()
                    .text_size(sp(12.5))
                    .line_height(sp(17.0))
                    .text_color(theme.text_tertiary)
                    .child(description),
            )
    }
}
