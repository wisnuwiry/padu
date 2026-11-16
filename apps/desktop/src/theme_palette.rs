// Generated from themes.json by scripts/generate-themes.ts. Do not edit directly.

use gpui::Hsla;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorScheme {
    #[default]
    Purple,
    Blue,
    Green,
    Amber,
    Coral,
    Rose,
    Neutral,
}

#[allow(dead_code)]
impl ColorScheme {
    pub const ALL: [Self; 7] = [
        ColorScheme::Purple,
        ColorScheme::Blue,
        ColorScheme::Green,
        ColorScheme::Amber,
        ColorScheme::Coral,
        ColorScheme::Rose,
        ColorScheme::Neutral,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            ColorScheme::Purple => "purple",
            ColorScheme::Blue => "blue",
            ColorScheme::Green => "green",
            ColorScheme::Amber => "amber",
            ColorScheme::Coral => "coral",
            ColorScheme::Rose => "rose",
            ColorScheme::Neutral => "neutral",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            ColorScheme::Purple => "Purple",
            ColorScheme::Blue => "Blue",
            ColorScheme::Green => "Green",
            ColorScheme::Amber => "Amber",
            ColorScheme::Coral => "Coral",
            ColorScheme::Rose => "Rose",
            ColorScheme::Neutral => "Neutral",
        }
    }

    pub const fn tint(self) -> bezel::theme::Tint {
        match self {
            ColorScheme::Purple => bezel::theme::Tint::new(276.935, 0.182),
            ColorScheme::Blue => bezel::theme::Tint::new(250.000, 0.180),
            ColorScheme::Green => bezel::theme::Tint::new(155.000, 0.160),
            ColorScheme::Amber => bezel::theme::Tint::new(85.000, 0.160),
            ColorScheme::Coral => bezel::theme::Tint::new(35.000, 0.170),
            ColorScheme::Rose => bezel::theme::Tint::new(15.000, 0.190),
            ColorScheme::Neutral => bezel::theme::Tint::NONE,
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "purple" | "Purple" => Some(ColorScheme::Purple),
            "blue" | "Blue" => Some(ColorScheme::Blue),
            "green" | "Green" => Some(ColorScheme::Green),
            "amber" | "Amber" => Some(ColorScheme::Amber),
            "coral" | "Coral" => Some(ColorScheme::Coral),
            "rose" | "Rose" => Some(ColorScheme::Rose),
            "neutral" | "Neutral" => Some(ColorScheme::Neutral),
            _ => None,
        }
    }

    /// Returns (accent, code_text) for dark theme.
    pub fn dark_tokens(self) -> (Hsla, Hsla) {
        match self {
            ColorScheme::Purple => (gpui::rgb(0x8B5CF6).into(), gpui::rgb(0xC4B5FD).into()),
            ColorScheme::Blue => (gpui::rgb(0x3B82F6).into(), gpui::rgb(0x93C5FD).into()),
            ColorScheme::Green => (gpui::rgb(0x10B981).into(), gpui::rgb(0x6EE7B7).into()),
            ColorScheme::Amber => (gpui::rgb(0xF59E0B).into(), gpui::rgb(0xFCD34D).into()),
            ColorScheme::Coral => (gpui::rgb(0xE2795B).into(), gpui::rgb(0xE0A882).into()),
            ColorScheme::Rose => (gpui::rgb(0xF43F5E).into(), gpui::rgb(0xFDA4AF).into()),
            ColorScheme::Neutral => (gpui::rgb(0xE4E4E7).into(), gpui::rgb(0xF4F4F5).into()),
        }
    }

    /// Returns (accent, code_text) for light theme.
    pub fn light_tokens(self) -> (Hsla, Hsla) {
        match self {
            ColorScheme::Purple => (gpui::rgb(0x7C3AED).into(), gpui::rgb(0x6D28D9).into()),
            ColorScheme::Blue => (gpui::rgb(0x2563EB).into(), gpui::rgb(0x1D4ED8).into()),
            ColorScheme::Green => (gpui::rgb(0x059669).into(), gpui::rgb(0x047857).into()),
            ColorScheme::Amber => (gpui::rgb(0xD97706).into(), gpui::rgb(0xB45309).into()),
            ColorScheme::Coral => (gpui::rgb(0xC85F44).into(), gpui::rgb(0x9A5528).into()),
            ColorScheme::Rose => (gpui::rgb(0xE11D48).into(), gpui::rgb(0xBE123C).into()),
            ColorScheme::Neutral => (gpui::rgb(0x27272A).into(), gpui::rgb(0x18181B).into()),
        }
    }
}
