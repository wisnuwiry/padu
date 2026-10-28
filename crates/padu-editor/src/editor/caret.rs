//! Caret blinking, visibility, and cursor following.

use std::time::Duration;

use gpui::Context;

use crate::editor::Editor;
use crate::model::Cursor;

pub const BLINK_INTERVAL: Duration = Duration::from_millis(500);

impl Editor {
    /// Start or restart caret blinking.
    pub fn start_blink(&mut self, cx: &mut Context<Self>) {
        self.caret_visible = true;
        self.caret_epoch += 1;
        let epoch = self.caret_epoch;
        self.blink_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(BLINK_INTERVAL).await;
                let Some(this) = this.upgrade() else { break };
                let stop = this.update(cx, |this, cx| {
                    if this.caret_epoch != epoch {
                        return true;
                    }
                    this.caret_visible = !this.caret_visible;
                    cx.notify();
                    false
                });
                if stop {
                    break;
                }
            }
        }));
    }

    /// Notify that the caret moved, immediately making it visible and resetting blink.
    pub fn caret_moved(&mut self) {
        self.caret_visible = true;
    }

    /// Set caret visibility.
    pub fn set_caret_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.caret_visible != visible {
            self.caret_visible = visible;
            cx.notify();
        }
    }

    /// Returns the current cursor position.
    pub fn cursor(&self) -> Cursor {
        self.selection.head
    }
}
