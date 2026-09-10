use super::*;

impl RightPanelSurface {
    pub(crate) fn new_browser() -> Self {
        Self::Browser(Uuid::new_v4())
    }

    pub(crate) fn new_terminal() -> Self {
        Self::Terminal(Uuid::new_v4())
    }

    pub(crate) fn terminal_id(&self) -> Option<Uuid> {
        match self {
            Self::Terminal(id) => Some(*id),
            _ => None,
        }
    }

    pub(crate) fn browser_id(&self) -> Option<Uuid> {
        match self {
            Self::Browser(id) => Some(*id),
            _ => None,
        }
    }

    pub(crate) fn label(&self) -> String {
        match self {
            Self::Browser(_) => tr!("right_panel.browser"),
            Self::Terminal(_) => tr!("right_panel.terminal"),
            Self::BackgroundWork { key, title } => {
                if title.is_empty() {
                    match key.kind {
                        BackgroundWorkKind::Process => tr!("background.process"),
                        BackgroundWorkKind::Monitor => tr!("background.monitor"),
                        BackgroundWorkKind::Subagent => tr!("background.subagent"),
                    }
                } else {
                    title.clone()
                }
            }
            Self::Files => tr!("right_panel.files"),
            Self::Diff => tr!("right_panel.diff"),
            Self::File(path) => path.rsplit('/').next().unwrap_or(path).to_owned(),
        }
    }

    pub(crate) fn icon_path(&self) -> &'static str {
        match self {
            Self::Browser(_) => "icons/globe.svg",
            Self::Terminal(_) => "icons/terminal.svg",
            Self::BackgroundWork { key, .. } => work_kind_icon(key.kind),
            Self::Files => "icons/folder.svg",
            Self::Diff => "icons/file-diff.svg",
            Self::File(path) => file_icon_for_path(path),
        }
    }

    pub(crate) fn shortcut(&self) -> Option<&'static str> {
        match self {
            Self::Browser(_) => Some(crate::platform::primary_shortcut("⌥⌘B", "Ctrl+Alt+B")),
            Self::Terminal(_) => Some(crate::platform::primary_shortcut("⌘T", "Ctrl+T")),
            Self::Files => Some(crate::platform::primary_shortcut("⇧⌘E", "Ctrl+Shift+E")),
            Self::Diff => Some(crate::platform::primary_shortcut("⌘D", "Ctrl+D")),
            _ => None,
        }
    }
}

pub(crate) fn right_panel_tab_label(
    surface: &RightPanelSurface,
    files_selected_path: Option<&str>,
) -> String {
    let label = match surface {
        RightPanelSurface::Files => files_selected_path
            .and_then(|path| Path::new(path).file_name())
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| tr!("right_panel.files")),
        _ => surface.label(),
    };
    single_line_label(&label)
}

pub(crate) fn right_panel_tab_icon(
    surface: &RightPanelSurface,
    files_selected_path: Option<&str>,
) -> &'static str {
    match surface {
        RightPanelSurface::Files => files_selected_path
            .map(file_icon_for_path)
            .unwrap_or_else(|| surface.icon_path()),
        _ => surface.icon_path(),
    }
}

pub(crate) fn reusable_surface_index(
    surfaces: &[RightPanelSurface],
    requested: &RightPanelSurface,
) -> Option<usize> {
    match requested {
        RightPanelSurface::Browser(_) | RightPanelSurface::Terminal(_) => None,
        RightPanelSurface::BackgroundWork { key, .. } => surfaces.iter().position(|surface| {
            matches!(surface, RightPanelSurface::BackgroundWork { key: candidate, .. } if candidate == key)
        }),
        RightPanelSurface::Files | RightPanelSurface::Diff | RightPanelSurface::File(_) => {
            surfaces.iter().position(|surface| surface == requested)
        }
    }
}

/// Fullscreen tab order: Conversation (`None`) first, then every surface
/// index in tab-strip order. Pure so tab cycling wraps are pinnable in tests
/// without a window.
pub(crate) fn fullscreen_tab_order(surface_count: usize) -> Vec<Option<usize>> {
    let mut order = Vec::with_capacity(surface_count + 1);
    order.push(None);
    order.extend((0..surface_count).map(Some));
    order
}

/// Next position when cycling the fullscreen order; wraps both directions.
pub(crate) fn fullscreen_cycle_next(
    position: usize,
    surface_count: usize,
    direction: isize,
) -> usize {
    let len = surface_count + 1;
    (position as isize + direction).rem_euclid(len as isize) as usize
}

#[derive(Clone, Copy)]
pub(crate) enum TabScrollFadeSide {
    Left,
    Right,
}

pub(crate) fn tab_scroll_fade_visibility(offset_x: Pixels, max_offset: Pixels) -> (bool, bool) {
    let scrolled = -offset_x;
    let threshold = px(0.5);
    (scrolled > threshold, max_offset - scrolled > threshold)
}

pub(crate) fn fade_safe_tab_offset(
    current_offset: Pixels,
    max_offset: Pixels,
    item_left: Pixels,
    item_right: Pixels,
    viewport_left: Pixels,
    viewport_right: Pixels,
) -> Pixels {
    let inset = px(TAB_SCROLL_FADE_WIDTH);
    let mut offset = current_offset;
    let visible_left = item_left + offset;
    let visible_right = item_right + offset;
    if visible_left < viewport_left + inset {
        offset += viewport_left + inset - visible_left;
    } else if visible_right > viewport_right - inset {
        offset -= visible_right - (viewport_right - inset);
    }
    offset.clamp(-max_offset, px(0.0))
}

pub(crate) fn tab_scroll_reveal_guard(
    scroll_handle: ScrollHandle,
    tab_index: usize,
    padu: WeakEntity<Padu>,
) -> impl IntoElement {
    canvas(
        move |_, window, _| {
            if let Some(item) = scroll_handle.bounds_for_item(tab_index) {
                let viewport = scroll_handle.bounds();
                let offset = scroll_handle.offset();
                let safe_offset = fade_safe_tab_offset(
                    offset.x,
                    scroll_handle.max_offset().x,
                    item.left(),
                    item.right(),
                    viewport.left(),
                    viewport.right(),
                );
                if safe_offset != offset.x {
                    scroll_handle.set_offset(point(safe_offset, offset.y));
                }
            }

            window.on_next_frame(move |_, cx| {
                let _ = padu.update(cx, |this, cx| {
                    if this.right_panel_pending_tab_reveal == Some(tab_index) {
                        this.right_panel_pending_tab_reveal = None;
                        cx.notify();
                    }
                });
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

pub(crate) fn tab_scroll_fade(
    scroll_handle: ScrollHandle,
    side: TabScrollFadeSide,
    surface: Hsla,
) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            let (show_left, show_right) =
                tab_scroll_fade_visibility(scroll_handle.offset().x, scroll_handle.max_offset().x);
            let visible = match side {
                TabScrollFadeSide::Left => show_left,
                TabScrollFadeSide::Right => show_right,
            };
            visible.then(|| {
                let transparent = surface.opacity(0.0);
                let background = match side {
                    TabScrollFadeSide::Left => linear_gradient(
                        90.0,
                        linear_color_stop(surface, 0.0),
                        linear_color_stop(transparent, 1.0),
                    ),
                    TabScrollFadeSide::Right => linear_gradient(
                        90.0,
                        linear_color_stop(transparent, 0.0),
                        linear_color_stop(surface, 1.0),
                    ),
                };
                fill(bounds, background)
            })
        },
        |_, fade, window, _| {
            if let Some(fade) = fade {
                window.paint_quad(fade);
            }
        },
    )
    .absolute()
    .top_0()
    .bottom_0()
    .when(matches!(side, TabScrollFadeSide::Left), |element| {
        element.left_0()
    })
    .when(matches!(side, TabScrollFadeSide::Right), |element| {
        element.right_0()
    })
    .w(px(TAB_SCROLL_FADE_WIDTH))
}

impl Padu {
    pub(crate) fn open_transcript_link(&mut self, target: &str, cx: &mut Context<Self>) -> bool {
        match transcript_link_route(target, self.selected_workspace_path()) {
            TranscriptLinkRoute::ProjectFile(relative_path) => {
                self.open_right_panel_surface(RightPanelSurface::Files, cx);
                self.open_right_panel_file(relative_path, cx);
            }
            TranscriptLinkRoute::Finder(path) => {
                if self.daemon.is_remote() {
                    self.show_toast(tr!("errors.remote_host_path"));
                    cx.notify();
                } else {
                    crate::platform::reveal_in_file_manager(&path, cx);
                }
            }
            TranscriptLinkRoute::External => return false,
        }
        true
    }

    /// Open a path a tool reported, from an activity in the transcript.
    ///
    /// Providers name a changed file however they like — absolute, or relative
    /// to the session's workspace — so resolve it before routing. Inside the
    /// workspace it opens in the file viewer; anywhere else it goes to the file
    /// manager, the same split a file link in the transcript takes.
    pub(crate) fn open_activity_file(&mut self, path: &str, cx: &mut Context<Self>) {
        let path = Path::new(path.trim());
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else if let Some(workspace) = self.selected_workspace_path() {
            workspace.join(path)
        } else {
            return;
        };
        self.open_transcript_link(&resolved.to_string_lossy(), cx);
    }

    pub(crate) fn store_selected_right_panel_state(&mut self) {
        let Some(session_id) = self.state.selected_session else {
            return;
        };
        let state = self.take_active_right_panel_state();
        self.right_panel_session_states.insert(session_id, state);
    }

    pub(crate) fn restore_right_panel_state(&mut self, session_id: Uuid, cx: &mut Context<Self>) {
        let state = RightPanelSessionState::take_or_closed(
            &mut self.right_panel_session_states,
            session_id,
        );
        self.replace_active_right_panel_state(state);
        self.sync_right_panel_diff_tree_rows(cx);
        // A read in flight when this session was switched away from had its
        // result dropped, and the flag it left behind would stop the editor
        // ever asking again. Clear it and read afresh, which also picks up
        // edits made while another session was on screen.
        for editor in self.right_panel_file_editors.values_mut() {
            editor.reading = false;
        }
        // The find bar pointed into the editors that were just swapped out;
        // its match list means nothing here, and restored editors may carry
        // washes stored mid-search.
        self.reset_file_search_for_session(cx);
        self.reload_clean_right_panel_file_editors(cx);
        self.state.right_panel_visible = self.right_panel_visible;
        if self.active_right_panel_surface() == Some(&RightPanelSurface::Diff) {
            self.refresh_right_panel_diff(cx);
        }
        if matches!(
            self.active_right_panel_surface(),
            Some(RightPanelSurface::Files | RightPanelSurface::File(_))
        ) {
            self.refresh_right_panel_working_tree(cx);
        }
        self.ensure_right_panel_terminals(cx);
        self.retain_right_panel_browsers(cx);
        if self.right_panel_visible {
            self.request_active_terminal_focus();
            self.request_active_browser_focus();
        }
    }

    pub(crate) fn remove_right_panel_session_state(
        &mut self,
        session_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        let state = if self.state.selected_session == Some(session_id) {
            let state = self.take_active_right_panel_state();
            self.replace_active_right_panel_state(RightPanelSessionState::empty(false));
            Some(state)
        } else {
            self.right_panel_session_states.remove(&session_id)
        };
        if let Some(state) = state {
            for surface in &state.surfaces {
                if let Some(terminal_id) = surface.terminal_id() {
                    self.right_panel_terminals.remove(&terminal_id);
                }
                if let Some(browser_id) = surface.browser_id() {
                    self.destroy_right_panel_browser(browser_id, cx);
                }
            }
        }
    }

    pub(crate) fn take_active_right_panel_state(&mut self) -> RightPanelSessionState {
        RightPanelSessionState {
            visible: self.right_panel_visible,
            fullscreen: self.right_panel_fullscreen,
            fullscreen_conversation: self.right_panel_fullscreen_conversation,
            surfaces: std::mem::take(&mut self.right_panel_surfaces),
            active_surface: self.right_panel_active_surface.take(),
            tabs_scroll_handle: std::mem::replace(
                &mut self.right_panel_tabs_scroll_handle,
                ScrollHandle::new(),
            ),
            pending_tab_reveal: self.right_panel_pending_tab_reveal.take(),
            expanded_paths: std::mem::take(&mut self.right_panel_expanded_paths),
            files_selected_path: self.right_panel_files_selected_path.take(),
            file_tree_width: self.right_panel_file_tree_width,
            file_editors: std::mem::take(&mut self.right_panel_file_editors),
            diff_source: self.right_panel_diff_source,
            diff_snapshot: self.right_panel_diff_snapshot.take(),
            diff_selected_file: self.right_panel_diff_selected_file.take(),
            diff_expanded_paths: std::mem::take(&mut self.right_panel_diff_expanded_paths),
            diff_file_layout: self.right_panel_diff_file_layout,
            diff_files_visible: self.right_panel_diff_files_visible,
        }
    }

    pub(crate) fn replace_active_right_panel_state(&mut self, state: RightPanelSessionState) {
        self.right_panel_visible = state.visible;
        self.right_panel_fullscreen = state.fullscreen;
        self.right_panel_fullscreen_conversation = state.fullscreen_conversation;
        self.right_panel_surfaces = state.surfaces;
        self.right_panel_active_surface = state.active_surface;
        self.right_panel_tabs_scroll_handle = state.tabs_scroll_handle;
        self.right_panel_pending_tab_reveal = state.pending_tab_reveal;
        self.right_panel_expanded_paths = state.expanded_paths;
        self.right_panel_files_selected_path = state.files_selected_path;
        self.right_panel_file_tree_width = state.file_tree_width;
        self.right_panel_file_editors = state.file_editors;
        self.right_panel_diff_generation = self.right_panel_diff_generation.wrapping_add(1);
        self.right_panel_diff_selection.clear();
        self.right_panel_diff_source = state.diff_source;
        self.right_panel_diff_snapshot = state.diff_snapshot;
        self.right_panel_diff_loading = false;
        self.right_panel_diff_error = None;
        self.right_panel_diff_selected_file = state.diff_selected_file;
        self.right_panel_diff_expanded_paths = state.diff_expanded_paths;
        self.right_panel_diff_file_layout = state.diff_file_layout;
        self.right_panel_diff_files_visible = state.diff_files_visible;
        self.right_panel_diff_tree_cursor = None;
        self.right_panel_diff_tree_rows.borrow_mut().clear();
        self.right_panel_diff_tree_list_state.reset(0);
        let line_count = self
            .right_panel_diff_snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.lines.len());
        self.right_panel_diff_list_state.reset(line_count);
    }

    pub(crate) fn reveal_right_panel_tab(&mut self, index: usize) {
        self.right_panel_pending_tab_reveal = Some(index);
        self.right_panel_tabs_scroll_handle.scroll_to_item(index);
    }

    pub(crate) fn active_right_panel_surface(&self) -> Option<&RightPanelSurface> {
        self.right_panel_active_surface
            .and_then(|index| self.right_panel_surfaces.get(index))
    }

    /// Whether a surface counts toward the fullscreen expand button: the four
    /// primary surfaces. Background work keeps working in fullscreen but never
    /// gates the button on its own.
    pub(crate) fn right_panel_surface_is_expandable(surface: &RightPanelSurface) -> bool {
        matches!(
            surface,
            RightPanelSurface::Browser(_)
                | RightPanelSurface::Terminal(_)
                | RightPanelSurface::Files
                | RightPanelSurface::File(_)
                | RightPanelSurface::Diff
        )
    }

    /// The expand button shows while the panel is open with at least one of
    /// browser, terminal, files, or review on screen.
    pub(crate) fn right_panel_can_expand(&self) -> bool {
        self.right_panel_visible
            && self
                .right_panel_surfaces
                .iter()
                .any(Self::right_panel_surface_is_expandable)
    }

    /// Whether the fullscreen takeover is actually on screen. The flag alone
    /// is not enough: closing the last expandable surface exits implicitly.
    pub(crate) fn right_panel_fullscreen_active(&self) -> bool {
        self.right_panel_visible && self.right_panel_fullscreen && self.right_panel_can_expand()
    }

    /// Ordered fullscreen tabs: Conversation first, then every surface index
    /// in tab-strip order. Cycling wraps over this list.
    pub(crate) fn right_panel_fullscreen_order(&self) -> Vec<Option<usize>> {
        fullscreen_tab_order(self.right_panel_surfaces.len())
    }

    /// Current position inside the fullscreen order: Conversation when the
    /// transcript is showing, otherwise the active surface index.
    pub(crate) fn right_panel_fullscreen_position(&self) -> Option<usize> {
        if self.right_panel_fullscreen_conversation {
            Some(0)
        } else {
            self.right_panel_active_surface.and_then(|active| {
                self.right_panel_fullscreen_order()
                    .iter()
                    .position(|slot| *slot == Some(active))
            })
        }
    }

    pub(crate) fn set_right_panel_fullscreen(&mut self, fullscreen: bool, cx: &mut Context<Self>) {
        if fullscreen && !self.right_panel_can_expand() {
            return;
        }
        if self.right_panel_fullscreen == fullscreen {
            return;
        }
        self.right_panel_fullscreen = fullscreen;
        if fullscreen {
            // Entering lands on the active surface; a missing active index
            // falls back to the first expandable tab.
            self.right_panel_fullscreen_conversation = false;
            if self
                .active_right_panel_surface()
                .is_none_or(|surface| !Self::right_panel_surface_is_expandable(surface))
                && let Some(index) = self
                    .right_panel_surfaces
                    .iter()
                    .position(Self::right_panel_surface_is_expandable)
            {
                self.right_panel_active_surface = Some(index);
                self.reveal_right_panel_tab(index);
            }
            self.request_active_terminal_focus();
            self.request_active_browser_focus();
        } else {
            self.right_panel_fullscreen_conversation = false;
        }
        cx.notify();
    }

    pub(crate) fn toggle_right_panel_fullscreen_action(
        &mut self,
        _: &ToggleRightPanelFullscreen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.right_panel_fullscreen_active() && !self.right_panel_can_expand() {
            return;
        }
        let next = !self.right_panel_fullscreen_active();
        // Toggling back on from a collapsed-but-flagged state must land on a
        // surface, never on a stale conversation view.
        if next && self.right_panel_fullscreen && self.right_panel_fullscreen_conversation {
            self.right_panel_fullscreen_conversation = false;
        }
        self.set_right_panel_fullscreen(next, cx);
        if next {
            self.focus_active_surface(window, cx);
        }
    }

    pub(crate) fn focus_active_surface(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.active_right_panel_surface() {
            Some(RightPanelSurface::Diff) => {
                let focus = self.transcript_control_focus("right-panel-diff", cx);
                window.focus(&focus, cx);
                let next_focus = focus.clone();
                window.on_next_frame(move |window, cx| {
                    window.focus(&next_focus, cx);
                });
            }
            Some(RightPanelSurface::Files | RightPanelSurface::File(_)) => {
                if let Some(path) = self.visible_right_panel_file_path()
                    && let Some(editor) = self.right_panel_file_editors.get(&path)
                {
                    let focus = editor.state.read(cx).focus();
                    window.focus(&focus, cx);
                    let next_focus = focus.clone();
                    window.on_next_frame(move |window, cx| {
                        window.focus(&next_focus, cx);
                    });
                } else {
                    let focus = self.transcript_control_focus("right-panel-working-tree", cx);
                    window.focus(&focus, cx);
                    let next_focus = focus.clone();
                    window.on_next_frame(move |window, cx| {
                        window.focus(&next_focus, cx);
                    });
                }
            }
            Some(RightPanelSurface::Terminal(_)) => {
                self.request_active_terminal_focus();
            }
            Some(RightPanelSurface::Browser(_)) => {
                self.request_active_browser_focus();
            }
            _ => {}
        }
    }

    pub(crate) fn cycle_right_panel_fullscreen(
        &mut self,
        direction: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.right_panel_fullscreen_active() {
            return;
        }
        let order = self.right_panel_fullscreen_order();
        if order.is_empty() {
            return;
        }
        let current = self.right_panel_fullscreen_position().unwrap_or(0);
        let next =
            order[fullscreen_cycle_next(current, self.right_panel_surfaces.len(), direction)];
        match next {
            None => {
                self.right_panel_fullscreen_conversation = true;
                let composer_focus = self.composer_focus(cx);
                window.focus(&composer_focus, cx);
                let next_focus = composer_focus.clone();
                window.on_next_frame(move |window, cx| {
                    window.focus(&next_focus, cx);
                });
                cx.notify();
            }
            Some(index) => {
                self.right_panel_fullscreen_conversation = false;
                self.right_panel_active_surface = Some(index);
                self.reveal_right_panel_tab(index);
                self.focus_active_surface(window, cx);
                cx.notify();
            }
        }
    }

    pub(crate) fn next_right_panel_tab_action(
        &mut self,
        _: &NextRightPanelTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.right_panel_fullscreen_active() {
            self.cycle_right_panel_fullscreen(1, window, cx);
        } else if self.right_panel_visible && self.right_panel_surfaces.len() > 1 {
            self.cycle_right_panel_docked(1, window, cx);
        }
    }

    pub(crate) fn prev_right_panel_tab_action(
        &mut self,
        _: &PrevRightPanelTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.right_panel_fullscreen_active() {
            self.cycle_right_panel_fullscreen(-1, window, cx);
        } else if self.right_panel_visible && self.right_panel_surfaces.len() > 1 {
            self.cycle_right_panel_docked(-1, window, cx);
        }
    }

    pub(crate) fn cycle_right_panel_docked(
        &mut self,
        direction: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.right_panel_surfaces.len() <= 1 {
            return;
        }
        let current = self.right_panel_active_surface.unwrap_or(0);
        let len = self.right_panel_surfaces.len() as isize;
        let next = ((current as isize + direction).rem_euclid(len)) as usize;
        self.right_panel_active_surface = Some(next);
        self.reveal_right_panel_tab(next);
        self.focus_active_surface(window, cx);
        cx.notify();
    }

    pub(crate) fn select_right_panel_fullscreen_conversation(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if !self.right_panel_fullscreen_active() {
            return;
        }
        self.right_panel_fullscreen_conversation = true;
        if let Some(window) = window {
            let composer_focus = self.composer_focus(cx);
            window.focus(&composer_focus, cx);
            let next_focus = composer_focus.clone();
            window.on_next_frame(move |window, cx| {
                window.focus(&next_focus, cx);
            });
        }
        cx.notify();
    }

    pub(crate) fn request_active_terminal_focus(&mut self) {
        self.right_panel_pending_terminal_focus = self
            .active_right_panel_surface()
            .and_then(RightPanelSurface::terminal_id);
    }

    pub(crate) fn request_active_browser_focus(&mut self) {
        self.right_panel_pending_browser_focus = self
            .active_right_panel_surface()
            .and_then(RightPanelSurface::browser_id);
    }

    /// The file the active editor surface is showing, whether via a File tab
    /// or the Files browser's selection — regardless of whether the panel is
    /// currently visible, which is a per-caller decision: save works on a
    /// hidden panel, find does not.
    pub(crate) fn visible_right_panel_file_path(&self) -> Option<String> {
        match self.active_right_panel_surface() {
            Some(RightPanelSurface::Files) => self.right_panel_files_selected_path.clone(),
            Some(RightPanelSurface::File(path)) => Some(path.clone()),
            _ => None,
        }
    }

    pub(crate) fn right_panel_file_is_dirty(&self, relative_path: &str) -> bool {
        self.right_panel_file_editors
            .get(relative_path)
            .is_some_and(|editor| editor.dirty)
    }

    pub(crate) fn right_panel_surface_is_dirty(&self, surface: &RightPanelSurface) -> bool {
        match surface {
            RightPanelSurface::Files => self
                .right_panel_files_selected_path
                .as_deref()
                .is_some_and(|path| self.right_panel_file_is_dirty(path)),
            RightPanelSurface::File(path) => self.right_panel_file_is_dirty(path),
            _ => false,
        }
    }

    pub(crate) fn ensure_initial_right_panel_file_editor_width(&mut self) {
        if self.right_panel_file_editors.is_empty() {
            self.right_panel_width = widened_panel_width_for_file_editor(
                self.right_panel_width,
                self.right_panel_file_tree_width,
            );
        }
    }

    pub(crate) fn open_right_panel_surface(
        &mut self,
        surface: RightPanelSurface,
        cx: &mut Context<Self>,
    ) {
        let reusable_index = reusable_surface_index(&self.right_panel_surfaces, &surface);
        if matches!(&surface, RightPanelSurface::File(_)) {
            self.ensure_initial_right_panel_file_editor_width();
        }
        if surface == RightPanelSurface::Diff {
            if reusable_index.is_none() {
                self.right_panel_width = widened_panel_width_for_review(self.right_panel_width);
            }
            self.refresh_right_panel_diff(cx);
        }
        if matches!(
            surface,
            RightPanelSurface::Files | RightPanelSurface::File(_)
        ) {
            self.refresh_right_panel_working_tree(cx);
        }
        if let Some(terminal_id) = surface.terminal_id() {
            self.ensure_right_panel_terminal(terminal_id, cx);
        }
        // Browser views are created on the surface's first render, which has
        // the `Window` their webview must attach to.
        let index = match reusable_index {
            Some(index) => index,
            None => {
                self.right_panel_surfaces.push(surface);
                self.right_panel_surfaces.len() - 1
            }
        };
        self.right_panel_active_surface = Some(index);
        self.reveal_right_panel_tab(index);
        self.request_active_terminal_focus();
        self.request_active_browser_focus();
        // Opening a surface always lands on that surface, never on the
        // fullscreen Conversation view.
        self.right_panel_fullscreen_conversation = false;
        self.set_right_panel_visible(true, cx);
        cx.notify();
    }

    pub(crate) fn open_browser_action(
        &mut self,
        _: &OpenBrowser,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_or_open_browser_surface(Some(window), cx);
    }

    pub(crate) fn open_terminal_action(
        &mut self,
        _: &OpenTerminal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_or_open_terminal_surface(Some(window), cx);
    }

    pub(crate) fn open_files_action(
        &mut self,
        _: &OpenFiles,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_or_open_files_surface(Some(window), cx);
    }

    pub(crate) fn open_review_action(
        &mut self,
        _: &OpenReview,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_or_open_review_surface(Some(window), cx);
    }

    pub(crate) fn toggle_or_open_browser_surface(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if let Some((index, _)) = self
            .right_panel_surfaces
            .iter()
            .enumerate()
            .find(|(_, surface)| matches!(surface, RightPanelSurface::Browser(_)))
        {
            if self.right_panel_fullscreen_active() {
                if !self.right_panel_fullscreen_conversation
                    && self.right_panel_active_surface == Some(index)
                {
                    self.select_right_panel_fullscreen_conversation(window, cx);
                } else {
                    self.right_panel_fullscreen_conversation = false;
                    self.right_panel_active_surface = Some(index);
                    self.reveal_right_panel_tab(index);
                    self.request_active_browser_focus();
                    if let Some(window) = window {
                        self.focus_active_surface(window, cx);
                    }
                    cx.notify();
                }
            } else if self.right_panel_visible && self.right_panel_active_surface == Some(index) {
                self.set_right_panel_visible(false, cx);
            } else {
                self.right_panel_active_surface = Some(index);
                self.reveal_right_panel_tab(index);
                self.request_active_browser_focus();
                self.set_right_panel_visible(true, cx);
                if let Some(window) = window {
                    self.focus_active_surface(window, cx);
                }
                cx.notify();
            }
        } else {
            self.open_right_panel_surface(RightPanelSurface::new_browser(), cx);
            if let Some(window) = window {
                self.focus_active_surface(window, cx);
            }
        }
    }

    pub(crate) fn toggle_or_open_terminal_surface(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if let Some((index, surface)) = self
            .right_panel_surfaces
            .iter()
            .enumerate()
            .find(|(_, surface)| matches!(surface, RightPanelSurface::Terminal(_)))
        {
            if self.right_panel_fullscreen_active() {
                if !self.right_panel_fullscreen_conversation
                    && self.right_panel_active_surface == Some(index)
                {
                    self.select_right_panel_fullscreen_conversation(window, cx);
                } else {
                    self.right_panel_fullscreen_conversation = false;
                    if let Some(terminal_id) = surface.terminal_id() {
                        self.ensure_right_panel_terminal(terminal_id, cx);
                    }
                    self.right_panel_active_surface = Some(index);
                    self.reveal_right_panel_tab(index);
                    self.request_active_terminal_focus();
                    if let Some(window) = window {
                        self.focus_active_surface(window, cx);
                    }
                    cx.notify();
                }
            } else if self.right_panel_visible && self.right_panel_active_surface == Some(index) {
                self.set_right_panel_visible(false, cx);
            } else {
                if let Some(terminal_id) = surface.terminal_id() {
                    self.ensure_right_panel_terminal(terminal_id, cx);
                }
                self.right_panel_active_surface = Some(index);
                self.reveal_right_panel_tab(index);
                self.request_active_terminal_focus();
                self.set_right_panel_visible(true, cx);
                if let Some(window) = window {
                    self.focus_active_surface(window, cx);
                }
                cx.notify();
            }
        } else {
            self.open_right_panel_surface(RightPanelSurface::new_terminal(), cx);
            if let Some(window) = window {
                self.focus_active_surface(window, cx);
            }
        }
    }

    pub(crate) fn toggle_or_open_files_surface(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if self.active_project().is_none() {
            return;
        }
        if let Some((index, _)) =
            self.right_panel_surfaces
                .iter()
                .enumerate()
                .find(|(_, surface)| {
                    matches!(
                        surface,
                        RightPanelSurface::Files | RightPanelSurface::File(_)
                    )
                })
        {
            if self.right_panel_fullscreen_active() {
                if !self.right_panel_fullscreen_conversation
                    && self.right_panel_active_surface == Some(index)
                {
                    self.select_right_panel_fullscreen_conversation(window, cx);
                } else {
                    self.right_panel_fullscreen_conversation = false;
                    self.refresh_right_panel_working_tree(cx);
                    self.right_panel_active_surface = Some(index);
                    self.reveal_right_panel_tab(index);
                    if let Some(window) = window {
                        self.focus_active_surface(window, cx);
                    }
                    cx.notify();
                }
            } else if self.right_panel_visible && self.right_panel_active_surface == Some(index) {
                self.set_right_panel_visible(false, cx);
            } else {
                self.refresh_right_panel_working_tree(cx);
                self.right_panel_active_surface = Some(index);
                self.reveal_right_panel_tab(index);
                self.set_right_panel_visible(true, cx);
                if let Some(window) = window {
                    self.focus_active_surface(window, cx);
                }
                cx.notify();
            }
        } else {
            self.open_right_panel_surface(RightPanelSurface::Files, cx);
            if let Some(window) = window {
                self.focus_active_surface(window, cx);
            }
        }
    }

    pub(crate) fn toggle_or_open_review_surface(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if self.active_project().is_none() {
            return;
        }
        if let Some((index, _)) = self
            .right_panel_surfaces
            .iter()
            .enumerate()
            .find(|(_, surface)| matches!(surface, RightPanelSurface::Diff))
        {
            if self.right_panel_fullscreen_active() {
                if !self.right_panel_fullscreen_conversation
                    && self.right_panel_active_surface == Some(index)
                {
                    self.select_right_panel_fullscreen_conversation(window, cx);
                } else {
                    self.right_panel_fullscreen_conversation = false;
                    self.refresh_right_panel_diff(cx);
                    self.right_panel_active_surface = Some(index);
                    self.reveal_right_panel_tab(index);
                    if let Some(window) = window {
                        self.focus_active_surface(window, cx);
                    }
                    cx.notify();
                }
            } else if self.right_panel_visible && self.right_panel_active_surface == Some(index) {
                self.set_right_panel_visible(false, cx);
            } else {
                self.refresh_right_panel_diff(cx);
                self.right_panel_active_surface = Some(index);
                self.reveal_right_panel_tab(index);
                self.set_right_panel_visible(true, cx);
                if let Some(window) = window {
                    self.focus_active_surface(window, cx);
                }
                cx.notify();
            }
        } else {
            self.open_right_panel_surface(RightPanelSurface::Diff, cx);
            if let Some(window) = window {
                self.focus_active_surface(window, cx);
            }
        }
    }

    pub(crate) fn open_turn_diff(&mut self, turn_id: Uuid, cx: &mut Context<Self>) {
        let Some((session_id, turn_count)) = self.selected_session().and_then(|session| {
            session
                .turns
                .iter()
                .find(|turn| turn.id == turn_id)
                .map(|turn| (session.id, turn.turn_count))
        }) else {
            return;
        };
        self.right_panel_diff_source = ReviewDiffSource::LastTurn {
            session_id,
            turn_id,
            turn_count,
        };
        self.right_panel_diff_selection.clear();
        self.right_panel_diff_snapshot = None;
        self.right_panel_diff_selected_file = None;
        self.open_right_panel_surface(RightPanelSurface::Diff, cx);
    }

    pub(crate) fn open_right_panel_file(&mut self, relative_path: String, cx: &mut Context<Self>) {
        self.ensure_initial_right_panel_file_editor_width();
        let Some(active) = self.right_panel_active_surface else {
            self.open_right_panel_surface(RightPanelSurface::File(relative_path), cx);
            return;
        };
        match self.right_panel_surfaces.get(active).cloned() {
            Some(RightPanelSurface::Files) => {
                let dirty_file_would_be_replaced = self
                    .right_panel_files_selected_path
                    .as_deref()
                    .is_some_and(|current_path| {
                        current_path != relative_path
                            && self.right_panel_file_is_dirty(current_path)
                    });
                if dirty_file_would_be_replaced {
                    self.open_right_panel_surface(RightPanelSurface::File(relative_path), cx);
                    return;
                }

                if let Some(pos) = self
                    .right_panel_working_tree
                    .iter()
                    .position(|e| e.relative_path == relative_path)
                {
                    self.right_panel_files_cursor = Some(pos);
                }
                self.right_panel_files_selected_path = Some(relative_path);
                self.set_right_panel_visible(true, cx);
                cx.notify();
            }
            Some(RightPanelSurface::File(current_path)) => {
                if current_path == relative_path {
                    return;
                }
                if self.right_panel_file_is_dirty(&current_path) {
                    self.open_right_panel_surface(RightPanelSurface::File(relative_path), cx);
                    return;
                }

                let requested = RightPanelSurface::File(relative_path);
                if let Some(existing) =
                    reusable_surface_index(&self.right_panel_surfaces, &requested)
                {
                    self.right_panel_surfaces.remove(active);
                    let existing = if existing > active {
                        existing - 1
                    } else {
                        existing
                    };
                    self.right_panel_active_surface = Some(existing);
                    self.reveal_right_panel_tab(existing);
                } else {
                    self.right_panel_surfaces[active] = requested;
                    self.reveal_right_panel_tab(active);
                }
                self.set_right_panel_visible(true, cx);
                cx.notify();
            }
            _ => self.open_right_panel_surface(RightPanelSurface::File(relative_path), cx),
        }
    }

    pub(crate) fn close_right_panel_surface(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.right_panel_surfaces.len() {
            return;
        }
        if let Some(terminal_id) = self.right_panel_surfaces[index].terminal_id() {
            self.right_panel_terminals.remove(&terminal_id);
        }
        if let Some(browser_id) = self.right_panel_surfaces[index].browser_id() {
            self.destroy_right_panel_browser(browser_id, cx);
        }
        self.right_panel_surfaces.remove(index);
        self.right_panel_active_surface = if self.right_panel_surfaces.is_empty() {
            None
        } else {
            Some(match self.right_panel_active_surface {
                Some(active) if active > index => active - 1,
                Some(active) if active == index => index.saturating_sub(1),
                Some(active) => active.min(self.right_panel_surfaces.len() - 1),
                None => 0,
            })
        };
        if let Some(active) = self.right_panel_active_surface {
            self.reveal_right_panel_tab(active);
            self.request_active_terminal_focus();
            self.request_active_browser_focus();
        } else {
            self.right_panel_pending_tab_reveal = None;
            self.right_panel_pending_terminal_focus = None;
            self.right_panel_pending_browser_focus = None;
            self.set_right_panel_visible(false, cx);
        }
        // Closing the last expandable surface exits fullscreen; otherwise a
        // flagged-but-empty state would linger into the next open.
        if !self.right_panel_can_expand() {
            self.right_panel_fullscreen = false;
            self.right_panel_fullscreen_conversation = false;
        }
        cx.notify();
    }

    pub(crate) fn close_window_or_right_panel_tab_action(
        &mut self,
        _: &CloseWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(active) = self.right_panel_active_surface {
            self.close_right_panel_surface(active, cx);
            if self.right_panel_surfaces.is_empty() {
                let focus_handle = self.composer_focus(cx);
                window.focus(&focus_handle, cx);
            }
        } else {
            crate::platform::hide_window(window);
        }
    }
}
