pub mod highlight;
pub mod mend;
pub mod parser;
pub mod raw_html;
pub mod render;
pub mod rich;
pub mod selection;
pub mod veil;

use bezel_markdown::AppExt as _;

/// Installs the markdown fence highlighter and supported languages into GPUI.
pub fn init(cx: &mut gpui::App) {
    cx.set_highlighter(
        highlight::highlight_code,
        highlight::HIGHLIGHT_LANGUAGES.iter().copied(),
    );
}
