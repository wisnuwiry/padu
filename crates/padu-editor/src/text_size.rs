//! Font size management and chord-based scale adjustments.

use gpui::{App, Global};

/// How far scaling chords move document text size in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextSize {
    pub step: f32,
    pub min: f32,
    pub max: f32,
}

impl TextSize {
    pub fn of(cx: &App) -> Self {
        cx.try_global::<Installed>()
            .map_or_else(Self::default, |installed| installed.0)
    }

    pub fn clamp(self, points: f32) -> f32 {
        (points.clamp(self.min, self.max) * 10.0).round() / 10.0
    }
}

impl Default for TextSize {
    fn default() -> Self {
        Self {
            step: 1.0,
            min: 10.0,
            max: 32.0,
        }
    }
}

struct Installed(TextSize);
impl Global for Installed {}

struct Adjustment(f32);
impl Global for Adjustment {}

/// Sets the text size step and bounds configuration.
pub fn set_text_size(cx: &mut App, text_size: TextSize) {
    cx.set_global(Installed(text_size));
}

/// Returns the current runtime text size adjustment in points.
pub fn text_size_adjustment(cx: &App) -> f32 {
    cx.try_global::<Adjustment>().map_or(0.0, |held| held.0)
}

/// Nudges the text size adjustment by `points`.
pub fn adjust_text_size(cx: &mut App, points: f32) {
    set_adjustment(text_size_adjustment(cx) + points, cx);
}

/// Resets the text size adjustment to 0.
pub fn reset_text_size(cx: &mut App) {
    set_adjustment(0.0, cx);
}

pub fn set_adjustment(points: f32, cx: &mut App) {
    cx.set_global(Adjustment(points));
    cx.refresh_windows();
}

/// Resolves the effective font size in points given a base size.
pub fn resolve(base: Option<f32>, cx: &App) -> f32 {
    let base = base.unwrap_or(14.0);
    TextSize::of(cx).clamp(base + text_size_adjustment(cx))
}
