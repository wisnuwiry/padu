//! Grouped dialog implementations and unified lifecycle management.

pub mod commit;
pub mod confirm;
pub mod goal;
pub mod host;

use gpui::{AnyElement, App, Context, Window};

use crate::app::Padu;

/// Initialize all keyboard shortcuts for modal dialogs.
pub fn init(cx: &mut App) {
    confirm::init(cx);
    commit::init(cx);
    goal::init(cx);
    host::init(cx);
}

impl Padu {
    /// Renders the currently active modal dialog in precedence order.
    ///
    /// Since modal dialogs occlude the full window and capture focus, only the
    /// top-most active dialog needs to be painted per frame.
    pub(super) fn render_active_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if let Some(element) = self.render_confirm_dialog(window, cx) {
            return Some(element);
        }
        if let Some(element) = self.render_commit_dialog(cx) {
            return Some(element);
        }
        if let Some(element) = self.render_goal_dialog(window, cx) {
            return Some(element);
        }
        if let Some(element) = self.render_host_dialog(window, cx) {
            return Some(element);
        }
        if let Some(element) = self.render_file_operation_dialog(window, cx) {
            return Some(element);
        }
        None
    }
}
