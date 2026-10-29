pub mod highlight;
pub mod mend;
pub mod parser;
pub mod raw_html;
pub mod render;
pub mod rich;
pub mod selection;
pub mod veil;

use bezel_editor::AppExt as _;
use bezel_markdown::AppExt as _;

/// Installs the markdown fence highlighter, block renderers, and editor menus into GPUI.
pub fn init(cx: &mut gpui::App) {
    cx.set_highlighter(
        highlight::highlight_code,
        highlight::HIGHLIGHT_LANGUAGES.iter().copied(),
    );
    cx.set_block_renderer(render::render_fence_block);

    init_editor_menus(cx);
}

fn init_editor_menus(cx: &mut gpui::App) {
    let mut slash_items = bezel_editor::slash_defaults();
    let custom_rows = vec![
        bezel_editor::SlashRow {
            label: "Mermaid Diagram".into(),
            icon: None,
            action: bezel_editor::SlashAction::Block(bezel_markdown::BlockKind::Code {
                language: Some("mermaid".into()),
                code: bezel_markdown::Text::plain("graph TD\n    A[Start] --> B[End]"),
                height: None,
            }),
        },
        bezel_editor::SlashRow {
            label: "Math Formula".into(),
            icon: None,
            action: bezel_editor::SlashAction::Block(bezel_markdown::BlockKind::Code {
                language: Some("math".into()),
                code: bezel_markdown::Text::plain("E = mc^2"),
                height: None,
            }),
        },
        bezel_editor::SlashRow {
            label: "LaTeX Math".into(),
            icon: None,
            action: bezel_editor::SlashAction::Block(bezel_markdown::BlockKind::Code {
                language: Some("latex".into()),
                code: bezel_markdown::Text::plain(
                    "\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}",
                ),
                height: None,
            }),
        },
    ];

    for row in &custom_rows {
        slash_items.push(bezel_editor::SlashItem::Row(row.clone()));
    }
    cx.set_slash_items(slash_items);

    let mut block_menu_items = bezel_editor::block_menu_defaults();
    for row in custom_rows {
        block_menu_items.push(bezel_editor::SlashItem::Row(row));
    }
    cx.set_block_menu_items(block_menu_items);
}
