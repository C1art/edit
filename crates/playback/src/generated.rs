use std::cell::RefCell;
use std::sync::Arc;

use egui::epaint::text::{FontDefinitions, Fonts, LayoutJob, TextFormat, TextOptions};
use egui::epaint::{Color32, FontColorTransferFunction, FontFamily, FontId};
use egui::{Align, Rect};
use media::VideoFrame;
use timeline::{Generator, Rgb, TextAlign, Ticks, Title, TitleFont};

/// Titles never wrap closer than this to the canvas edges, as broadcast title safe.
const TITLE_SAFE: f32 = 0.9;
/// Font atlases only grow, so a long session of title edits starts a fresh one here.
const ATLAS_REFILL: f32 = 0.8;
const SHADOW_OPACITY: f32 = 0.7;

thread_local! {
    static FONTS: RefCell<Option<Fonts>> = const { RefCell::new(None) };
}

/// The picture `generator` draws on a `width` by `height` canvas, with straight alpha.
pub fn generated_frame(generator: &Generator, width: u32, height: u32) -> Arc<VideoFrame> {
    let mut canvas = Canvas::new(width, height);
    match generator {
        Generator::ColorMatte { color } => canvas.fill(*color),
        Generator::Title(title) => draw_title(&mut canvas, title),
        Generator::AdjustmentLayer => {}
    }
    Arc::new(VideoFrame {
        pts: Ticks::ZERO,
        width,
        height,
        pixels: canvas.into_pixels(),
    })
}

fn draw_title(canvas: &mut Canvas, title: &Title) {
    let Some(text) = Coverage::of_title(title, canvas.width, canvas.height) else {
        return;
    };
    let size = title.size as f32;
    if let Some(background) = title.background {
        let pad = (size * 0.3).round() as i64;
        let (left, top, right, bottom) = text.bounds;
        canvas.fill_box(
            (left - pad, top - pad / 2, right + pad, bottom + pad / 2),
            background,
        );
    }
    if title.shadow {
        let offset = (size / 24.0).round().max(1.0) as i64;
        let blur = (size / 32.0).round() as usize;
        let shadow = text.shifted(offset, offset).blurred(blur);
        canvas.paint(&shadow, Rgb::BLACK, SHADOW_OPACITY);
    }
    // Text over a transparent canvas would otherwise sample black fringes from the
    // pixels around it when scaled.
    if title.background.is_none() && !title.shadow {
        canvas.bleed(title.color);
    }
    canvas.paint(&text, title.color, 1.0);
}

/// Straight-alpha RGBA pixels with `f32` channels while drawing.
struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<[f32; 4]>,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![[0.0; 4]; width as usize * height as usize],
        }
    }

    fn fill(&mut self, color: Rgb) {
        self.pixels.fill(opaque(color));
    }

    fn bleed(&mut self, color: Rgb) {
        let [r, g, b, _] = opaque(color);
        for pixel in self.pixels.iter_mut().filter(|pixel| pixel[3] == 0.0) {
            *pixel = [r, g, b, 0.0];
        }
    }

    fn fill_box(&mut self, (left, top, right, bottom): (i64, i64, i64, i64), color: Rgb) {
        let clamp_x = |x: i64| x.clamp(0, i64::from(self.width)) as usize;
        let clamp_y = |y: i64| y.clamp(0, i64::from(self.height)) as usize;
        let width = self.width as usize;
        for y in clamp_y(top)..clamp_y(bottom) {
            self.pixels[y * width + clamp_x(left)..y * width + clamp_x(right)].fill(opaque(color));
        }
    }

    /// Composites `color` over the canvas where `coverage` covers it.
    fn paint(&mut self, coverage: &Coverage, color: Rgb, opacity: f32) {
        let [r, g, b, _] = opaque(color);
        for (pixel, &cover) in self.pixels.iter_mut().zip(&coverage.alpha) {
            let alpha = cover * opacity;
            if alpha <= 0.0 {
                continue;
            }
            let below = pixel[3] * (1.0 - alpha);
            let out = alpha + below;
            let mix = |top: f32, bottom: f32| (top * alpha + bottom * below) / out;
            *pixel = [mix(r, pixel[0]), mix(g, pixel[1]), mix(b, pixel[2]), out];
        }
    }

    fn into_pixels(self) -> Vec<u8> {
        self.pixels
            .into_iter()
            .flat_map(|pixel| pixel.map(|channel| (channel * 255.0).round() as u8))
            .collect()
    }
}

fn opaque(color: Rgb) -> [f32; 4] {
    [color.r, color.g, color.b, 255].map(|channel| f32::from(channel) / 255.0)
}

/// How much of each canvas pixel a shape covers, from 0 to 1.
struct Coverage {
    width: usize,
    height: usize,
    alpha: Vec<f32>,
    /// Left, top, right and bottom of the laid out text, in canvas pixels.
    bounds: (i64, i64, i64, i64),
}

impl Coverage {
    fn of_title(title: &Title, width: u32, height: u32) -> Option<Self> {
        if title.text.trim().is_empty() || title.size == 0 {
            return None;
        }
        let family = match title.font {
            TitleFont::Sans => FontFamily::Proportional,
            TitleFont::Mono => FontFamily::Monospace,
        };
        let mut job = LayoutJob::single_section(
            title.text.clone(),
            TextFormat::simple(FontId::new(title.size as f32, family), Color32::WHITE),
        );
        job.wrap.max_width = width as f32 * TITLE_SAFE;
        job.halign = match title.align {
            TextAlign::Left => Align::LEFT,
            TextAlign::Center => Align::Center,
            TextAlign::Right => Align::RIGHT,
        };
        FONTS.with_borrow_mut(|fonts| {
            let fonts = match fonts {
                Some(fonts) if fonts.font_atlas_fill_ratio() < ATLAS_REFILL => fonts,
                _ => fonts.insert(new_fonts()),
            };
            let galley = fonts.layout_uncached(1.0, Arc::new(job));
            let mut coverage = Self {
                width: width as usize,
                height: height as usize,
                alpha: vec![0.0; width as usize * height as usize],
                bounds: (0, 0, 0, 0),
            };
            // The galley's rows sit around x = 0 as aligned; center the whole block.
            let rect = galley.rect;
            let origin = egui::vec2(width as f32, height as f32) / 2.0 - rect.center().to_vec2();
            coverage.bounds = bounds(rect.translate(origin));
            let atlas = fonts.texture_atlas().image();
            for row in &galley.rows {
                for glyph in &row.row.glyphs {
                    let uv = glyph.uv_rect;
                    if uv.is_nothing() {
                        continue;
                    }
                    let top_left = row.pos + origin + glyph.pos.to_vec2() + uv.offset;
                    coverage.stamp(
                        top_left.x.round() as i64,
                        top_left.y.round() as i64,
                        |x, y| {
                            let [u, v] = [usize::from(uv.min[0]) + x, usize::from(uv.min[1]) + y];
                            (u < usize::from(uv.max[0]) && v < usize::from(uv.max[1]))
                                .then(|| f32::from(atlas.pixels[v * atlas.size[0] + u].a()) / 255.0)
                        },
                        [uv.max[0] - uv.min[0], uv.max[1] - uv.min[1]].map(usize::from),
                    );
                }
            }
            Some(coverage)
        })
    }

    /// Adds a glyph of `size` texels whose top left lands on `(left, top)`.
    fn stamp(
        &mut self,
        left: i64,
        top: i64,
        texel: impl Fn(usize, usize) -> Option<f32>,
        size: [usize; 2],
    ) {
        for y in 0..size[1] {
            let Ok(canvas_y) = usize::try_from(top + y as i64) else {
                continue;
            };
            for x in 0..size[0] {
                let Ok(canvas_x) = usize::try_from(left + x as i64) else {
                    continue;
                };
                if canvas_x >= self.width || canvas_y >= self.height {
                    continue;
                }
                if let Some(cover) = texel(x, y) {
                    let pixel = &mut self.alpha[canvas_y * self.width + canvas_x];
                    *pixel = (*pixel + cover).min(1.0);
                }
            }
        }
    }

    fn shifted(&self, dx: i64, dy: i64) -> Self {
        let mut alpha = vec![0.0; self.alpha.len()];
        for (y, row) in alpha.chunks_mut(self.width).enumerate() {
            let Ok(from_y) = usize::try_from(y as i64 - dy) else {
                continue;
            };
            if from_y >= self.height {
                continue;
            }
            for (x, pixel) in row.iter_mut().enumerate() {
                if let Ok(from_x) = usize::try_from(x as i64 - dx) {
                    if from_x < self.width {
                        *pixel = self.alpha[from_y * self.width + from_x];
                    }
                }
            }
        }
        Self { alpha, ..*self }
    }

    /// Box-blurs `radius` pixels each way, across then down.
    fn blurred(mut self, radius: usize) -> Self {
        if radius == 0 {
            return self;
        }
        let (width, height) = (self.width, self.height);
        box_blur(&mut self.alpha, width, height, 1, width, radius);
        box_blur(&mut self.alpha, height, width, width, 1, radius);
        self
    }
}

/// Blurs `lines` lines of `length` values; consecutive values are `step` apart and
/// lines start `stride` apart.
fn box_blur(
    values: &mut [f32],
    length: usize,
    lines: usize,
    step: usize,
    stride: usize,
    radius: usize,
) {
    let window = (2 * radius + 1) as f32;
    let mut line = vec![0.0; length];
    for start in (0..lines).map(|index| index * stride) {
        line.iter_mut()
            .enumerate()
            .for_each(|(i, value)| *value = values[start + i * step]);
        let mut sum: f32 = line[..radius.min(length)].iter().sum();
        for i in 0..length {
            if let Some(entering) = line.get(i + radius) {
                sum += entering;
            }
            if let Some(leaving) = (i > radius).then(|| line[i - radius - 1]) {
                sum -= leaving;
            }
            values[start + i * step] = sum / window;
        }
    }
}

fn bounds(rect: Rect) -> (i64, i64, i64, i64) {
    (
        rect.left().floor() as i64,
        rect.top().floor() as i64,
        rect.right().ceil() as i64,
        rect.bottom().ceil() as i64,
    )
}

fn new_fonts() -> Fonts {
    let options = TextOptions {
        max_texture_side: 8192,
        color_transfer_function: FontColorTransferFunction::Gamma(1.0),
        subpixel_binning: false,
        ..TextOptions::default()
    };
    Fonts::new(options, FontDefinitions::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_blur_spreads_a_point_evenly_and_keeps_its_weight() {
        let mut values = vec![0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0];
        box_blur(&mut values, 7, 1, 1, 7, 1);
        assert_eq!(values, [0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
    }
}
