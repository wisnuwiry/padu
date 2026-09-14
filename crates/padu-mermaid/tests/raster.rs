//! Raster-level regression tests: string assertions (starts with `<svg>`, has a
//! viewBox, no `<foreignObject>`) cannot catch a diagram that parses yet paints
//! nothing. These tests rasterize through the exact stack GPUI uses for
//! `ImageFormat::Svg` (usvg 0.46 + resvg 0.46, same versions as the desktop
//! build) and assert real pixels land — the QuickJS-era SVGs this renderer
//! replaced rasterized to zero visible pixels, which surfaced in-app as a
//! blank diagram card served forever from the disk cache.

use gpui::{Hsla, rgb};
use padu_mermaid::{AccentColor, MermaidTheme, text_color_for_background};

/// Padu dark-palette-shaped theme: opaque near-black canvas with composited
/// (opaque) borders — mirroring `Palette::mermaid_theme`, which must never
/// pass the translucent washes through raw (their RGB inverts across modes).
fn dark_theme() -> MermaidTheme {
    let canvas: Hsla = rgb(0x1A1A1A).into();
    let border: Hsla = rgb(0x2C2C31).into();
    let inset: Hsla = rgb(0x151515).into();
    let text: Hsla = rgb(0xE2E2E2).into();
    let accent: Hsla = rgb(0x8B5CF6).into();
    let chart = [accent; 8];
    MermaidTheme {
        dark_mode: true,
        font_family: "system-ui".into(),
        background: canvas,
        primary_color: inset,
        primary_text_color: text,
        primary_border_color: border,
        secondary_color: inset,
        tertiary_color: inset,
        line_color: text,
        text_color: text,
        edge_label_background: inset,
        cluster_background: canvas,
        cluster_border: border,
        note_background: inset,
        note_border: border,
        actor_background: inset,
        actor_border: border,
        activation_background: inset,
        activation_border: border,
        git_branch_colors: chart,
        git_branch_label_colors: chart.map(text_color_for_background),
        er_attr_bg_odd: text,
        er_attr_bg_even: text,
        error_color: text,
        warning_color: text,
        accent_colors: vec![AccentColor {
            foreground: accent,
            background: accent,
        }],
    }
}

fn visible_pixel_fraction(svg: &[u8]) -> f64 {
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_data(svg, &opt).expect("svg must parse");
    let size = tree.size();
    assert!(
        size.width() > 0.0 && size.height() > 0.0,
        "svg must have a non-zero intrinsic size, got {size:?}"
    );
    let pixmap_size = size.to_int_size();
    let mut pixmap =
        tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).expect("pixmap");
    resvg::render(&tree, Default::default(), &mut pixmap.as_mut());
    let pixels = pixmap.data();
    let visible = pixels.chunks_exact(4).filter(|pixel| pixel[3] > 8).count();
    visible as f64 / (pixels.len() / 4) as f64
}

fn assert_paints(source: &str, theme: &MermaidTheme) {
    let svg = padu_mermaid::render_to_svg(source, theme).expect("diagram must render");
    let fraction = visible_pixel_fraction(svg.as_bytes());
    assert!(
        fraction > 0.01,
        "diagram painted almost nothing ({fraction:.4} of pixels visible) — blank-card regression: {source:?}"
    );
}

#[test]
fn flowchart_paints_visible_pixels_in_both_themes() {
    let source = "flowchart LR\n  A[Start] --> B{Decision}\n  B -->|yes| C[Ok]\n";
    assert_paints(source, &MermaidTheme::default());
    assert_paints(source, &dark_theme());
}

#[test]
fn sequence_diagram_paints_visible_pixels() {
    let source = "sequenceDiagram\n    Alice->>Bob: Hello\n    Bob-->>Alice: Hi\n";
    assert_paints(source, &MermaidTheme::default());
    assert_paints(source, &dark_theme());
}
