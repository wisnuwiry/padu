// for a very big json! macro
#![recursion_limit = "256"]

//! Crate for rendering Mermaid diagram strings to SVG strings.
//!
//! Vendored from Zed's `mermaid_render` crate and adapted to Padu's palette:
//! node accent colors come from the Padu theme rather than Zed's player
//! colors, and the `ztracing`/`tracing` instrumentation is dropped.
//!
//! The entrypoint is [`render_to_svg`]. It takes a `&str` and a
//! [`MermaidTheme`]. The output is an SVG with the following properties:
//! - The style matches the provided theme
//! - Nodes are given accent colors, even if none are provided in the mermaid
//!   source.
//! - The SVG has been tweaked based on the assumption that it will be rasterized
//!   using `usvg`/`resvg` (which is what GPUI uses for `ImageFormat::Svg`).
//!   Some bugs/quirks of `usvg`/`resvg` are accounted for in this crate.
//!
//! This module uses the [`merman`] crate for rendering: a headless,
//! parity-focused Rust implementation of Mermaid (parse, layout, render).
//!
//! Since merman 0.6, the generic raster-safe SVG cleanup (HTML labels in
//! `<foreignObject>`, CSS/attribute forms that rasterizers do not handle) is
//! exposed as merman's raster-safe SVG pipeline. Padu opts into that pipeline
//! during rendering, then keeps app-specific theme and accent color rules in
//! this crate. The [`gpui`] dependency is only needed for the [`Hsla`] and
//! [`Rgba`] color types.
//!
//! The [`render_to_svg`] function operates in two stages:
//! - [`render`] the mermaid text to raster-safe SVG using [`merman`].
//! - [`postprocess`] the SVG to add Padu theme and accent styling.
//!
//! Postprocessing is split up into stages. The generated SVG is parsed using
//! [`quick_xml`], which produces an iterator of
//! [`Event<'_>`](quick_xml::events::Event)s. This iterator is then repeatedly
//! transformed, and finally collected back into an SVG string.
//!
//! This approach:
//! - Avoids doing multiple expensive string insertions.
//! - Avoids parsing the SVG multiple times (without needing to put all the
//!   logic in one huge function).
//! - But is quite a bit more complex.
//!
//! The complexity is justified because of the drastic performance impact, as
//! well as the low-risk nature; this code cannot panic, and errors in the
//! output just produce weird-looking diagrams.
//!
//! ## Color handling
//!
//! The theme is matched to Padu's palette, and accent colors are applied to
//! diagrams to make them more visually interesting.
//!
//! There are three parts to color handling:
//!
//! 1. A [`merman::MermaidConfig`] is passed when initially rendering the
//!    diagram. This sets most "normal" colors (background, text, etc.). However,
//!    it's not possible to color nodes individually, and not all parts of the
//!    diagrams are correctly themed.
//! 2. `postprocess::accent_colors` injects custom CSS classes (e.g.
//!    `padu-accent-0`) to specific elements, based on the diagram type and
//!    node.
//! 3. `postprocess::inject_css` injects CSS rules for the classes applied by
//!    `accent_colors`

mod postprocess;
mod render;

use anyhow::Result;
use gpui::{Hsla, Rgba};

#[derive(Debug, Clone, Copy)]
pub struct AccentColor {
    pub foreground: Hsla,
    pub background: Hsla,
}

#[derive(Debug, Clone)]
pub struct MermaidTheme {
    pub dark_mode: bool,
    pub font_family: String,
    pub background: Hsla,
    pub primary_color: Hsla,
    pub primary_text_color: Hsla,
    pub primary_border_color: Hsla,
    pub secondary_color: Hsla,
    pub tertiary_color: Hsla,
    pub line_color: Hsla,
    pub text_color: Hsla,
    pub edge_label_background: Hsla,
    pub cluster_background: Hsla,
    pub cluster_border: Hsla,
    pub note_background: Hsla,
    pub note_border: Hsla,
    pub actor_background: Hsla,
    pub actor_border: Hsla,
    pub activation_background: Hsla,
    pub activation_border: Hsla,
    pub git_branch_colors: [Hsla; 8],
    pub git_branch_label_colors: [Hsla; 8],
    pub er_attr_bg_odd: Hsla,
    pub er_attr_bg_even: Hsla,
    pub error_color: Hsla,
    pub warning_color: Hsla,
    pub accent_colors: Vec<AccentColor>,
}

impl Default for MermaidTheme {
    fn default() -> Self {
        use gpui::{hsla, rgb};
        let git_branch_colors: [Hsla; 8] = [
            hsla(240.0 / 360.0, 1.0, 0.462_745_1, 1.0),
            hsla(60.0 / 360.0, 1.0, 0.435_294_12, 1.0),
            hsla(80.0 / 360.0, 1.0, 0.462_745_1, 1.0),
            hsla(210.0 / 360.0, 1.0, 0.462_745_1, 1.0),
            hsla(180.0 / 360.0, 1.0, 0.462_745_1, 1.0),
            hsla(150.0 / 360.0, 1.0, 0.462_745_1, 1.0),
            hsla(300.0 / 360.0, 1.0, 0.462_745_1, 1.0),
            hsla(0.0, 1.0, 0.462_745_1, 1.0),
        ];
        let git_branch_label_colors: [Hsla; 8] =
            git_branch_colors.map(crate::text_color_for_background);

        Self {
            dark_mode: false,
            font_family: "system-ui".to_string(),
            background: rgb(0xFFFFFF).into(),
            primary_color: rgb(0xF8FAFC).into(),
            primary_text_color: rgb(0x0F172A).into(),
            primary_border_color: rgb(0x94A3B8).into(),
            secondary_color: rgb(0xE2E8F0).into(),
            tertiary_color: rgb(0xFFFFFF).into(),
            line_color: rgb(0x64748B).into(),
            text_color: rgb(0x0F172A).into(),
            edge_label_background: rgb(0xFFFFFF).into(),
            cluster_background: rgb(0xF1F5F9).into(),
            cluster_border: rgb(0xCBD5E1).into(),
            note_background: rgb(0xFFF7ED).into(),
            note_border: rgb(0xFDBA74).into(),
            actor_background: rgb(0xF8FAFC).into(),
            actor_border: rgb(0x94A3B8).into(),
            activation_background: rgb(0xE2E8F0).into(),
            activation_border: rgb(0x94A3B8).into(),
            git_branch_colors,
            git_branch_label_colors,
            er_attr_bg_odd: rgb(0x94A3B8).into(),
            er_attr_bg_even: rgb(0x0F172A).into(),
            error_color: rgb(0xDC2626).into(),
            warning_color: rgb(0xD97706).into(),
            accent_colors: Vec::new(),
        }
    }
}

/// Formats a color as a CSS hex color for embedding in SVG/CSS.
///
/// Emits `#rrggbb` for fully opaque colors and `#rrggbbaa` when the input
/// has any transparency, so translucent theme colors round-trip without
/// silently losing their alpha.
pub(crate) fn css_color(color: Hsla) -> String {
    let rgba = Rgba::from(color);
    let r = (rgba.r.clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (rgba.g.clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (rgba.b.clamp(0.0, 1.0) * 255.0).round() as u8;
    let a = (rgba.a.clamp(0.0, 1.0) * 255.0).round() as u8;
    if a == 0xff {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

pub use postprocess::util::text_color_for_background;

/// See the [module-level docs][crate] for more info.
///
/// This is CPU-heavy (layout + text measurement): callers must run it off the
/// UI thread — in Padu that means the rich-render worker, never `render`.
pub fn render_to_svg(source: &str, theme: &MermaidTheme) -> Result<String> {
    let svg = render::render_mermaid(source, theme)?;
    let svg = postprocess::postprocess(&svg, theme)?;
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_diagram_returns_an_error() {
        let result = render_to_svg(
            "flowchart LR\n  this is not [ valid",
            &MermaidTheme::default(),
        );
        assert!(
            result.is_err(),
            "invalid diagram should error, got {:?}",
            result.map(|svg| svg.len())
        );
    }

    #[test]
    fn mermaid_diagram_with_mixed_weight_combining_marks_does_not_panic() {
        let zalgo = "Ne\u{0301}\u{0302}\u{0303}\u{0304}\u{0306}\u{0307}\u{0308}\u{030a}d";
        let source = format!("flowchart TD\n  A[\"**{zalgo}** {zalgo}\"]");
        let svg = render_to_svg(&source, &MermaidTheme::default())
            .expect("mermaid diagram should render to SVG");
        assert!(
            svg.trim_start().starts_with("<svg"),
            "expected svg, got {}",
            &svg[..svg.len().min(120)]
        );
    }

    /// An ER diagram whose attribute-block tokens begin with a multibyte
    /// UTF-8 character (e.g. CJK type/field names) must not panic while the
    /// lexer probes for the two-character `PK`/`FK`/`UK` keys.
    #[test]
    fn er_multibyte_attribute_does_not_crash() {
        let source = "erDiagram\n顧客 {\n  文字列 名前\n}";
        let _ = render_to_svg(source, &MermaidTheme::default());
    }

    /// A flowchart with mutually nested subgraphs (`A` contains `B` and `B`
    /// contains `A`) is an invalid containment cycle. Rendering it must return
    /// gracefully rather than overflowing the stack and aborting the process.
    #[test]
    fn cyclic_subgraphs_do_not_crash() {
        let source = "flowchart TD\n  subgraph A\n    B\n  end\n  subgraph B\n    A\n  end";
        let result = render_to_svg(source, &MermaidTheme::default());
        if let Err(err) = result {
            let message = format!("{err:#}");
            assert!(
                message.contains("cycle"),
                "expected a cycle-related error, got: {message}"
            );
        }
    }
}
