use std::sync::Arc;

use super::*;
use image::ImageEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use padu_client::persistence::{ConversationBackgroundEffect, EFFECT_FILTER_CONFIG};

/// Conversation backgrounds are decorative, so they are capped well below the
/// 48 MB wire limit to keep daemon uploads and transcript frames cheap.
pub(super) const MAX_BACKGROUND_IMAGE_BYTES: usize = 2 * 1024 * 1024;

const BAYER_8X8: [[f32; 8]; 8] = [
    [
        0.0 / 64.0,
        32.0 / 64.0,
        8.0 / 64.0,
        40.0 / 64.0,
        2.0 / 64.0,
        34.0 / 64.0,
        10.0 / 64.0,
        42.0 / 64.0,
    ],
    [
        48.0 / 64.0,
        16.0 / 64.0,
        56.0 / 64.0,
        24.0 / 64.0,
        50.0 / 64.0,
        18.0 / 64.0,
        58.0 / 64.0,
        26.0 / 64.0,
    ],
    [
        12.0 / 64.0,
        44.0 / 64.0,
        4.0 / 64.0,
        36.0 / 64.0,
        14.0 / 64.0,
        46.0 / 64.0,
        6.0 / 64.0,
        38.0 / 64.0,
    ],
    [
        60.0 / 64.0,
        28.0 / 64.0,
        52.0 / 64.0,
        20.0 / 64.0,
        62.0 / 64.0,
        30.0 / 64.0,
        54.0 / 64.0,
        22.0 / 64.0,
    ],
    [
        3.0 / 64.0,
        35.0 / 64.0,
        11.0 / 64.0,
        43.0 / 64.0,
        1.0 / 64.0,
        33.0 / 64.0,
        9.0 / 64.0,
        41.0 / 64.0,
    ],
    [
        51.0 / 64.0,
        19.0 / 64.0,
        59.0 / 64.0,
        27.0 / 64.0,
        49.0 / 64.0,
        17.0 / 64.0,
        57.0 / 64.0,
        25.0 / 64.0,
    ],
    [
        15.0 / 64.0,
        47.0 / 64.0,
        7.0 / 64.0,
        39.0 / 64.0,
        13.0 / 64.0,
        45.0 / 64.0,
        5.0 / 64.0,
        37.0 / 64.0,
    ],
    [
        63.0 / 64.0,
        31.0 / 64.0,
        55.0 / 64.0,
        23.0 / 64.0,
        61.0 / 64.0,
        29.0 / 64.0,
        53.0 / 64.0,
        21.0 / 64.0,
    ],
];

// 8x8 bitmap glyphs for 8 progressive ASCII density levels:
// ' ', '.', ':', '-', '+', '*', '%', '#'
const ASCII_GLYPHS_8X8: [[u8; 8]; 8] = [
    // ' '
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    // '.'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00],
    // ':'
    [0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x00],
    // '-'
    [0x00, 0x00, 0x00, 0x7E, 0x7E, 0x00, 0x00, 0x00],
    // '+'
    [0x00, 0x18, 0x18, 0x7E, 0x7E, 0x18, 0x18, 0x00],
    // '*'
    [0x00, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0x00, 0x00],
    // '%'
    [0x00, 0x63, 0x66, 0x0C, 0x18, 0x33, 0x63, 0x00],
    // '#'
    [0x24, 0x7E, 0x24, 0x24, 0x7E, 0x24, 0x00, 0x00],
];

static DITHER_LUT: std::sync::LazyLock<[[u8; 256]; 64]> = std::sync::LazyLock::new(|| {
    let mut lut = [[0u8; 256]; 64];
    let step = EFFECT_FILTER_CONFIG.dither.color_step;
    let spread_factor = EFFECT_FILTER_CONFIG.dither.spread;
    for y in 0..8 {
        for x in 0..8 {
            let bayer_idx = (y << 3) | x;
            let bayer = BAYER_8X8[y][x] - 0.5;
            let spread = bayer * spread_factor;
            for c in 0..=255 {
                let quantized = ((c as f32 + spread).clamp(0.0, 255.0) / step).round() * step;
                lut[bayer_idx][c] = quantized.clamp(0.0, 255.0) as u8;
            }
        }
    }
    lut
});

static ASCII_DITHER_LUT: std::sync::LazyLock<[[u8; 256]; 64]> = std::sync::LazyLock::new(|| {
    let mut lut = [[0u8; 256]; 64];
    let step = EFFECT_FILTER_CONFIG.ascii.color_step;
    let spread_factor = EFFECT_FILTER_CONFIG.ascii.spread;
    for y in 0..8 {
        for x in 0..8 {
            let bayer_idx = (y << 3) | x;
            let bayer = BAYER_8X8[y][x] - 0.5;
            let spread = bayer * spread_factor;
            for c in 0..=255 {
                let quantized = ((c as f32 + spread).clamp(0.0, 255.0) / step).round() * step;
                lut[bayer_idx][c] = quantized.clamp(0.0, 255.0) as u8;
            }
        }
    }
    lut
});

fn encode_png_fast(img: &image::RgbaImage) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let encoder =
        PngEncoder::new_with_quality(&mut bytes, CompressionType::Fast, FilterType::NoFilter);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some(bytes)
}

fn process_background_bytes(
    raw: &[u8],
    original_format: gpui::ImageFormat,
    effect: ConversationBackgroundEffect,
) -> Option<Arc<gpui::Image>> {
    match effect {
        ConversationBackgroundEffect::None => Some(Arc::new(gpui::Image::from_bytes(
            original_format,
            raw.to_vec(),
        ))),
        ConversationBackgroundEffect::Dither => {
            let Some(dynamic_img) = image::load_from_memory(raw).ok() else {
                return Some(Arc::new(gpui::Image::from_bytes(
                    original_format,
                    raw.to_vec(),
                )));
            };
            let rgba = dynamic_img.to_rgba8();
            let (orig_w, orig_h) = rgba.dimensions();
            if orig_w == 0 || orig_h == 0 {
                return None;
            }

            // Single source of truth effect resolution from background-effects.json
            let target_w = EFFECT_FILTER_CONFIG.dither.max_width.min(orig_w);
            let target_h = ((orig_h as f32 / orig_w as f32) * target_w as f32)
                .round()
                .max(1.0) as u32;
            let (target_w, target_h, source) = if target_w == orig_w && target_h == orig_h {
                (orig_w, orig_h, std::borrow::Cow::Borrowed(&rgba))
            } else {
                (
                    target_w,
                    target_h,
                    std::borrow::Cow::Owned(image::imageops::resize(
                        &rgba,
                        target_w,
                        target_h,
                        image::imageops::FilterType::Triangle,
                    )),
                )
            };

            let mut out = image::RgbaImage::new(target_w, target_h);
            let src_bytes = source.as_raw();
            let out_bytes = out.as_mut();
            let lut = &*DITHER_LUT;

            for y in 0..target_h {
                let y_shift = ((y & 7) << 3) as usize;
                let row_start = (y * target_w * 4) as usize;
                let row_end = row_start + (target_w * 4) as usize;
                let src_row = &src_bytes[row_start..row_end];
                let out_row = &mut out_bytes[row_start..row_end];

                for x in 0..target_w as usize {
                    let px_idx = x * 4;
                    let a = src_row[px_idx + 3];
                    if a < 10 {
                        out_row[px_idx..px_idx + 4].copy_from_slice(&[0, 0, 0, 0]);
                        continue;
                    }

                    let lut_row = &lut[y_shift | (x & 7)];
                    out_row[px_idx] = lut_row[src_row[px_idx] as usize];
                    out_row[px_idx + 1] = lut_row[src_row[px_idx + 1] as usize];
                    out_row[px_idx + 2] = lut_row[src_row[px_idx + 2] as usize];
                    out_row[px_idx + 3] = a;
                }
            }

            if let Some(png_bytes) = encode_png_fast(&out) {
                Some(Arc::new(gpui::Image::from_bytes(
                    gpui::ImageFormat::Png,
                    png_bytes,
                )))
            } else {
                Some(Arc::new(gpui::Image::from_bytes(
                    original_format,
                    raw.to_vec(),
                )))
            }
        }
        ConversationBackgroundEffect::Ascii => {
            let Some(dynamic_img) = image::load_from_memory(raw).ok() else {
                return Some(Arc::new(gpui::Image::from_bytes(
                    original_format,
                    raw.to_vec(),
                )));
            };
            let rgba = dynamic_img.to_rgba8();
            let (orig_w, orig_h) = rgba.dimensions();
            if orig_w == 0 || orig_h == 0 {
                return None;
            }

            // ASCII art structure configured via background-effects.json
            let cell_size = EFFECT_FILTER_CONFIG.ascii.cell_size;
            let min_cols = EFFECT_FILTER_CONFIG.ascii.min_columns;
            let max_cols = EFFECT_FILTER_CONFIG.ascii.max_columns;
            let cols = (orig_w / cell_size).clamp(min_cols, max_cols);
            let rows = ((orig_h as f32 / orig_w as f32) * cols as f32)
                .round()
                .max(1.0) as u32;

            let sampled =
                image::imageops::resize(&rgba, cols, rows, image::imageops::FilterType::Triangle);

            let out_w = cols * cell_size;
            let out_h = rows * cell_size;
            let mut out = image::RgbaImage::new(out_w, out_h);
            let out_bytes = out.as_mut();
            let sampled_bytes = sampled.as_raw();
            let lut = &*ASCII_DITHER_LUT;
            let bg_dim = EFFECT_FILTER_CONFIG.ascii.bg_dim;
            let bg_scale = (bg_dim * 256.0).round() as u32;

            for cy in 0..rows {
                let sampled_row_offset = (cy * cols * 4) as usize;
                for cx in 0..cols {
                    let s_idx = sampled_row_offset + (cx * 4) as usize;
                    let a = sampled_bytes[s_idx + 3];
                    if a < 10 {
                        continue;
                    }

                    let r = sampled_bytes[s_idx] as usize;
                    let g = sampled_bytes[s_idx + 1] as usize;
                    let b = sampled_bytes[s_idx + 2] as usize;
                    let luma = (r * 77 + g * 150 + b * 29) >> 8;

                    let glyph_idx = ((luma * 7 + 128) >> 8).min(7);
                    let glyph = ASCII_GLYPHS_8X8[glyph_idx];

                    // Dither-quantized colors for ASCII characters and softly dimmed background
                    let bg_r = ((r as u32 * bg_scale) >> 8).min(255) as usize;
                    let bg_g = ((g as u32 * bg_scale) >> 8).min(255) as usize;
                    let bg_b = ((b as u32 * bg_scale) >> 8).min(255) as usize;

                    let base_y = cy * cell_size;
                    let base_x = cx * cell_size;

                    for gy in 0..8 {
                        let row_bits = glyph[gy];
                        let y_shift = gy << 3;
                        let row_start = (((base_y + gy as u32) * out_w + base_x) * 4) as usize;
                        let row_slice = &mut out_bytes[row_start..row_start + 32];
                        for gx in 0..8 {
                            let is_fg = ((row_bits >> (7 - gx)) & 1) != 0;
                            let bayer_idx = y_shift | gx;
                            let lut_row = &lut[bayer_idx];
                            let px = gx * 4;
                            if is_fg {
                                row_slice[px] = lut_row[r];
                                row_slice[px + 1] = lut_row[g];
                                row_slice[px + 2] = lut_row[b];
                                row_slice[px + 3] = a;
                            } else {
                                row_slice[px] = lut_row[bg_r];
                                row_slice[px + 1] = lut_row[bg_g];
                                row_slice[px + 2] = lut_row[bg_b];
                                row_slice[px + 3] = a;
                            }
                        }
                    }
                }
            }

            if let Some(png_bytes) = encode_png_fast(&out) {
                Some(Arc::new(gpui::Image::from_bytes(
                    gpui::ImageFormat::Png,
                    png_bytes,
                )))
            } else {
                Some(Arc::new(gpui::Image::from_bytes(
                    original_format,
                    raw.to_vec(),
                )))
            }
        }
    }
}

/// Loads and paints the conversation background without doing filesystem work
/// from the transcript frame path.
impl Padu {
    pub(super) fn render_conversation_background(&self, cx: &App) -> Option<AnyElement> {
        let theme = Theme::current(cx);
        render_conversation_background(
            self.conversation_background_image(),
            &self.state.conversation_background,
            &theme,
            Some(self.conversation_card_corner_radius()),
        )
    }

    pub(super) fn conversation_card_corner_radius(&self) -> Pixels {
        if self.right_panel_fullscreen_active() && self.right_panel_fullscreen_conversation {
            px(14.0)
        } else {
            px(10.0)
        }
    }

    pub(super) fn set_conversation_background_opacity(
        &mut self,
        value: f32,
        cx: &mut Context<Self>,
    ) {
        self.state.conversation_background.opacity = value.clamp(0.0, 1.0);
        self.save();
        cx.notify();
    }

    pub(super) fn set_conversation_background_height(
        &mut self,
        value: f32,
        cx: &mut Context<Self>,
    ) {
        self.state.conversation_background.height_percent = value.clamp(20.0, 100.0);
        self.save();
        cx.notify();
    }

    pub(super) fn set_conversation_background_effect(
        &mut self,
        effect: ConversationBackgroundEffect,
        cx: &mut Context<Self>,
    ) {
        if self.state.conversation_background.effect == effect {
            return;
        }
        self.state.conversation_background.effect = effect;
        self.save();
        self.refresh_conversation_background_effect(cx);
        cx.notify();
    }

    pub(super) fn refresh_conversation_background_effect(&mut self, cx: &mut Context<Self>) {
        let effect = self.state.conversation_background.effect;
        if let Some(cached) = self
            .conversation_background_cache
            .borrow()
            .get(&effect)
            .cloned()
        {
            *self.conversation_background_image.borrow_mut() = Some(cached);
            cx.notify();
            return;
        }

        let raw = self.conversation_background_raw.borrow().clone();
        if let Some((format, bytes)) = raw {
            let generation = self
                .conversation_background_generation
                .get()
                .wrapping_add(1);
            self.conversation_background_generation.set(generation);
            cx.spawn(async move |this, cx| {
                let processed = cx
                    .background_executor()
                    .spawn(async move { process_background_bytes(&bytes, format, effect) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if this.conversation_background_generation.get() != generation {
                        return;
                    }
                    if let Some(img) = &processed {
                        this.conversation_background_cache
                            .borrow_mut()
                            .insert(effect, img.clone());
                    }
                    *this.conversation_background_image.borrow_mut() = processed;
                    cx.notify();
                });
            })
            .detach();
        } else {
            self.load_persisted_conversation_background(cx);
        }
    }

    pub(super) fn conversation_background_image(&self) -> Option<Arc<gpui::Image>> {
        self.conversation_background_image.borrow().clone()
    }

    pub(super) fn load_persisted_conversation_background(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.state.conversation_background.image_path.clone() else {
            return;
        };
        let Some(format) =
            crate::app::image_preview::image_format_for_name(path.to_string_lossy().as_ref())
        else {
            return;
        };
        let effect = self.state.conversation_background.effect;
        let generation = self
            .conversation_background_generation
            .get()
            .wrapping_add(1);
        self.conversation_background_generation.set(generation);
        cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_executor()
                .spawn(async move {
                    std::fs::read(&path).map(|bytes| {
                        let processed = process_background_bytes(&bytes, format, effect);
                        (format, Arc::new(bytes), processed)
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.conversation_background_generation.get() != generation {
                    return;
                }
                if let Ok((format, raw_arc, processed)) = loaded {
                    this.conversation_background_cache.borrow_mut().clear();
                    if let Some(img) = &processed {
                        this.conversation_background_cache
                            .borrow_mut()
                            .insert(effect, img.clone());
                    }
                    *this.conversation_background_raw.borrow_mut() = Some((format, raw_arc));
                    *this.conversation_background_image.borrow_mut() = processed;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn choose_conversation_background(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("settings.background_choose").into()),
        });
        let effect = self.state.conversation_background.effect;
        let generation = self
            .conversation_background_generation
            .get()
            .wrapping_add(1);
        self.conversation_background_generation.set(generation);
        cx.spawn(async move |this, cx| {
            let result = receiver
                .await
                .ok()
                .and_then(|result| result.ok())
                .and_then(|paths| paths.and_then(|paths| paths.into_iter().next()));
            let Some(path) = result else { return };
            let Some(format) =
                crate::app::image_preview::image_format_for_name(path.to_string_lossy().as_ref())
            else {
                let _ = this.update(cx, |this, cx| {
                    this.show_toast(tr!("settings.background_unsupported"));
                    cx.notify();
                });
                return;
            };
            let loaded = cx
                .background_executor()
                .spawn(async move {
                    std::fs::read(&path).map(|bytes| {
                        let processed = process_background_bytes(&bytes, format, effect);
                        (path, format, Arc::new(bytes), processed)
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.conversation_background_generation.get() != generation {
                    return;
                }
                match loaded {
                    Ok((path, format, raw_arc, processed))
                        if !raw_arc.is_empty() && raw_arc.len() <= MAX_BACKGROUND_IMAGE_BYTES =>
                    {
                        this.conversation_background_cache.borrow_mut().clear();
                        if let Some(img) = &processed {
                            this.conversation_background_cache
                                .borrow_mut()
                                .insert(effect, img.clone());
                        }
                        this.state.conversation_background.image_path = Some(path);
                        *this.conversation_background_raw.borrow_mut() = Some((format, raw_arc));
                        *this.conversation_background_image.borrow_mut() = processed;
                        this.save();
                    }
                    Ok((_, _, raw_arc, _)) if raw_arc.len() > MAX_BACKGROUND_IMAGE_BYTES => {
                        this.show_toast(tr!("settings.background_too_large"));
                    }
                    _ => this.show_toast(tr!("settings.background_unavailable")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn clear_conversation_background(&mut self, cx: &mut Context<Self>) {
        self.conversation_background_generation.set(
            self.conversation_background_generation
                .get()
                .wrapping_add(1),
        );
        self.state.conversation_background.image_path = None;
        self.conversation_background_raw.borrow_mut().take();
        self.conversation_background_image.borrow_mut().take();
        self.conversation_background_cache.borrow_mut().clear();
        self.save();
        cx.notify();
    }
}

pub(super) fn render_conversation_background(
    image: Option<Arc<gpui::Image>>,
    settings: &padu_client::persistence::ConversationBackgroundSettings,
    theme: &Theme,
    corner_radius: Option<Pixels>,
) -> Option<AnyElement> {
    let image = image?;
    let radius = corner_radius.unwrap_or(px(0.0));
    let is_full_height = settings.height_percent >= 99.0;
    Some(
        div()
            .absolute()
            .left_0()
            .right_0()
            .top_0()
            .h(if is_full_height {
                gpui::relative(1.0)
            } else {
                gpui::relative(settings.height_percent / 100.0)
            })
            .rounded_t(radius)
            .when(is_full_height, |d| d.rounded_b(radius))
            .overflow_hidden()
            .child(
                img(image)
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .opacity(settings.opacity)
                    .rounded_t(radius)
                    .when(is_full_height, |i| i.rounded_b(radius))
                    .flex_none(),
            )
            .when(!is_full_height, |d| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(gpui::relative(0.55))
                        .bg(linear_gradient(
                            180.0,
                            linear_color_stop(theme.surface.opacity(0.0), 0.0),
                            linear_color_stop(theme.surface, 1.0),
                        )),
                )
            })
            .into_any_element(),
    )
}
