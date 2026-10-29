//! How the editor lays the document out.

use gpui::{App, Global};

/// What the editor lays the document out with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// How far the document's text sits inside the editor's bounding box.
    pub text_inset: f32,
}

impl Layout {
    /// Retrieves the current layout from the global app context, or default.
    pub fn of(cx: &App) -> Self {
        cx.try_global::<Installed>()
            .map_or_else(Self::default, |installed| installed.0)
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self { text_inset: 22.0 }
    }
}

struct Installed(Layout);

impl Global for Installed {}

/// Sets the global editor layout.
pub fn set_layout(cx: &mut App, layout: Layout) {
    cx.set_global(Installed(layout));
}
