//! Visual styling and theme tokens for the block editor.

use gpui::{Hsla, hsla};

/// Visual theme colors for the editor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorTheme {
    pub is_dark: bool,
    pub surface: Hsla,
    pub inset: Hsla,
    pub overlay: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub text_secondary: Hsla,
    pub text_ghost: Hsla,
    pub accent: Hsla,
    pub code_wash: Hsla,
}

impl Default for EditorTheme {
    fn default() -> Self {
        Self {
            is_dark: false,
            surface: hsla(0.0, 0.0, 1.0, 1.0),
            inset: hsla(0.0, 0.0, 0.98, 1.0),
            overlay: hsla(0.0, 0.0, 0.95, 1.0),
            border: hsla(0.0, 0.0, 0.85, 1.0),
            text: hsla(0.0, 0.0, 0.12, 1.0),
            text_secondary: hsla(0.0, 0.0, 0.45, 1.0),
            text_ghost: hsla(0.0, 0.0, 0.65, 1.0),
            accent: hsla(0.58, 0.8, 0.5, 1.0),
            code_wash: hsla(0.0, 0.0, 0.0, 0.04),
        }
    }
}
