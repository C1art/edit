use std::cell::RefCell;
use std::sync::Arc;

use egui::epaint::text::{FontDefinitions, Fonts, Galley, LayoutJob, TextFormat, TextOptions};
use egui::epaint::{Color32, FontColorTransferFunction, FontFamily, FontId};
use egui::{Align, Rect};
use timeline::{TextAlign, Title, TitleFont};

use super::Anchor;

/// Titles never wrap closer than this to the canvas edges, as broadcast title safe.
const TITLE_SAFE: f32 = 0.9;
/// Font atlases only grow, so a long session of title edits starts a fresh one here.
const ATLAS_REFILL: f32 = 0.8;
/// Captions sit this fraction of the canvas height above its bottom, inside title safe.
const CAPTION_MARGIN: f32 = 0.08;
/// Faux italic leans each row this far right per pixel up, about eleven degrees.
const ITALIC_SLANT: f32 = 0.2;
/// Stands in for infinity in the distance transform, where infinity minus itself is NaN.
const FAR: f64 = 1e12;

thread_local! {
    static FONTS: RefCell<Option<Fonts>> = const { RefCell::new(None) };
}

/// `title` laid out to wrap within title safe of a canvas `width` wide, or on one line
/// for a crawl, with the fonts made or refreshed first.
fn title_galley(title: &Title, width: u32, fonts: &mut Option<Fonts>) -> Galley {
    let fonts = match fonts {
        Some(fonts) if fonts.font_atlas_fill_ratio() < ATLAS_REFILL => fonts,
        _ => fonts.insert(new_fonts()),
    };
    let size = title.size as f32;
    let family = match title.font {
        TitleFont::Sans => FontFamily::Proportional,
        TitleFont::Mono => FontFamily::Monospace,
    };
    let font = FontId::new(size, family);
    let row_height = fonts.with_pixels_per_point(1.0).row_height(&font);
    let mut format = TextFormat::simple(font, Color32::WHITE);
    format.extra_letter_spacing = size * title.tracking as f32 / 1000.0;
    format.line_height = Some((row_height + title.leading as f32).max(1.0));
    let mut job = LayoutJob::single_section(title.text.clone(), format);
    job.wrap.max_width = if title.motion.is_crawl() {
        f32::INFINITY
    } else {
        width as f32 * TITLE_SAFE
    };
    job.halign = match title.align {
        TextAlign::Left => Align::LEFT,
        TextAlign::Center => Align::Center,
        TextAlign::Right => Align::RIGHT,
    };
    fonts.layout_uncached(1.0, Arc::new(job))
}

/// How much of each canvas pixel a shape covers, from 0 to 1.
pub(super) struct Coverage {
    width: usize,
    height: usize,
    pub(super) alpha: Vec<f32>,
    /// Left, top, right and bottom of the laid out text, in canvas pixels.
    pub(super) bounds: (i64, i64, i64, i64),
    /// The same for the pixels actually covered, which slanted or grown letters push
    /// past the layout.
    ink: (i64, i64, i64, i64),
}

impl Coverage {
    /// The width and height `title` lays out to, wrapped as on a canvas `width` wide.
    pub(super) fn title_extent(title: &Title, width: u32) -> Option<egui::Vec2> {
        if title.text.trim().is_empty() || title.size == 0 {
            return None;
        }
        FONTS.with_borrow_mut(|fonts| Some(title_galley(title, width, fonts).rect.size()))
    }

    pub(super) fn of_title(title: &Title, width: u32, height: u32, anchor: Anchor) -> Option<Self> {
        if title.text.trim().is_empty() || title.size == 0 {
            return None;
        }
        FONTS.with_borrow_mut(|fonts| {
            let galley = title_galley(title, width, fonts);
            let fonts = fonts.as_mut()?;
            let mut coverage = Self::empty(width as usize, height as usize);
            // The galley's rows sit around x = 0 as aligned; center the whole block.
            let rect = galley.rect;
            let canvas = egui::vec2(width as f32, height as f32);
            let mut origin = canvas / 2.0 - rect.center().to_vec2();
            if anchor == Anchor::Bottom {
                origin.y = canvas.y * (1.0 - CAPTION_MARGIN) - rect.bottom();
            }
            coverage.bounds = bounds(rect.translate(origin));
            let slant = if title.italic { ITALIC_SLANT } else { 0.0 };
            let atlas = fonts.texture_atlas().image();
            for row in &galley.rows {
                // Letters lean about the middle of their row, so the block stays centered.
                let middle = origin.y + row.pos.y + row.row.size.y / 2.0;
                let lean = |y: i64| slant * (middle - y as f32);
                for glyph in &row.row.glyphs {
                    let uv = glyph.uv_rect;
                    if uv.is_nothing() {
                        continue;
                    }
                    let top_left = row.pos + origin + glyph.pos.to_vec2() + uv.offset;
                    coverage.stamp(
                        top_left.x.round(),
                        top_left.y.round() as i64,
                        lean,
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

    /// `cover` of each pixel center inside `region`, which bounds the shape.
    pub(super) fn from_fn(
        width: usize,
        height: usize,
        region: (i64, i64, i64, i64),
        cover: impl Fn(f32, f32) -> f32,
    ) -> Self {
        let mut coverage = Self::empty(width, height);
        coverage.bounds = region;
        let (left, top, right, bottom) = region;
        for y in top.max(0)..bottom.min(height as i64) {
            for x in left.max(0)..right.min(width as i64) {
                coverage.add(x, y, cover(x as f32 + 0.5, y as f32 + 0.5));
            }
        }
        coverage
    }

    fn empty(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            alpha: vec![0.0; width * height],
            bounds: (0, 0, 0, 0),
            ink: (i64::MAX, i64::MAX, i64::MIN, i64::MIN),
        }
    }

    /// Adds a glyph of `size` texels whose top left lands on `(left, top)`, each row moved
    /// right by `lean` of its canvas row and split between the two pixels it straddles.
    fn stamp(
        &mut self,
        left: f32,
        top: i64,
        lean: impl Fn(i64) -> f32,
        texel: impl Fn(usize, usize) -> Option<f32>,
        size: [usize; 2],
    ) {
        for y in 0..size[1] {
            let canvas_y = top + y as i64;
            let start = left + lean(canvas_y);
            let (whole, fraction) = (start.floor() as i64, start - start.floor());
            for x in 0..size[0] {
                let Some(cover) = texel(x, y) else {
                    continue;
                };
                let canvas_x = whole + x as i64;
                self.add(canvas_x, canvas_y, cover * (1.0 - fraction));
                self.add(canvas_x + 1, canvas_y, cover * fraction);
            }
        }
    }

    fn add(&mut self, x: i64, y: i64, cover: f32) {
        if cover <= 0.0 || x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return;
        }
        let pixel = &mut self.alpha[y as usize * self.width + x as usize];
        *pixel = (*pixel + cover).min(1.0);
        let ink = &mut self.ink;
        *ink = (
            ink.0.min(x),
            ink.1.min(y),
            ink.2.max(x + 1),
            ink.3.max(y + 1),
        );
    }

    pub(super) fn shifted(&self, dx: i64, dy: i64) -> Self {
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
        let (left, top, right, bottom) = self.ink;
        Self {
            alpha,
            ink: (left + dx, top + dy, right + dx, bottom + dy),
            ..*self
        }
    }

    /// Box-blurs `radius` pixels each way, across then down.
    pub(super) fn blurred(mut self, radius: usize) -> Self {
        if radius == 0 {
            return self;
        }
        let (width, height) = (self.width, self.height);
        box_blur(&mut self.alpha, width, height, 1, width, radius);
        box_blur(&mut self.alpha, height, width, width, 1, radius);
        self
    }

    /// The shape grown `radius` pixels in every direction, with round corners and a
    /// one-pixel soft edge.
    pub(super) fn dilated(&self, radius: f32) -> Self {
        let mut grown = Self {
            alpha: self.alpha.clone(),
            ..*self
        };
        let (left, top, right, bottom) = self.ink;
        if radius <= 0.0 || left >= right {
            return grown;
        }
        let reach = radius.ceil() as i64 + 1;
        let clamp = |value: i64, limit: usize| value.clamp(0, limit as i64) as usize;
        let (x0, x1) = (
            clamp(left - reach, self.width),
            clamp(right + reach, self.width),
        );
        let (y0, y1) = (
            clamp(top - reach, self.height),
            clamp(bottom + reach, self.height),
        );
        let window_width = x1 - x0;
        let mut distances: Vec<f64> = (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .map(|(x, y)| {
                if self.alpha[y * self.width + x] >= 0.5 {
                    0.0
                } else {
                    FAR
                }
            })
            .collect();
        squared_distances(&mut distances, window_width, y1 - y0);
        for (index, squared) in distances.into_iter().enumerate() {
            let (x, y) = (x0 + index % window_width, y0 + index / window_width);
            let edge = (radius + 0.5 - squared.sqrt() as f32).clamp(0.0, 1.0);
            let pixel = &mut grown.alpha[y * self.width + x];
            *pixel = pixel.max(edge);
        }
        grown.ink = (x0 as i64, y0 as i64, x1 as i64, y1 as i64);
        grown
    }
}

/// Replaces each value of a `width` by `height` grid, zero inside a shape and `FAR`
/// outside, with its squared distance to the nearest point inside: Felzenszwalb and
/// Huttenlocher's transform, down each column and then along each row.
fn squared_distances(grid: &mut [f64], width: usize, height: usize) {
    let longest = width.max(height);
    let mut line = vec![0.0; longest];
    let mut out = vec![0.0; longest];
    let mut hull = vec![0; longest];
    let mut edges = vec![0.0; longest + 1];
    let mut pass = |length: usize, index: &dyn Fn(usize) -> usize| {
        (0..length).for_each(|i| line[i] = grid[index(i)]);
        lower_envelope(&line[..length], &mut out, &mut hull, &mut edges);
        (0..length).for_each(|i| grid[index(i)] = out[i]);
    };
    for x in 0..width {
        pass(height, &|y| y * width + x);
    }
    for y in 0..height {
        pass(width, &|x| y * width + x);
    }
}

/// One dimension of the transform: each point's minimum over every sample `q` of
/// `(point - q)² + values[q]`, from the lower envelope of those parabolas.
fn lower_envelope(values: &[f64], out: &mut [f64], hull: &mut [usize], edges: &mut [f64]) {
    let crossing = |q: usize, p: usize| {
        let (q_f, p_f) = (q as f64, p as f64);
        ((values[q] + q_f * q_f) - (values[p] + p_f * p_f)) / (2.0 * (q_f - p_f))
    };
    let mut top = 0;
    hull[0] = 0;
    edges[0] = f64::NEG_INFINITY;
    edges[1] = f64::INFINITY;
    for q in 1..values.len() {
        let mut meet = crossing(q, hull[top]);
        while meet <= edges[top] {
            top -= 1;
            meet = crossing(q, hull[top]);
        }
        top += 1;
        hull[top] = q;
        edges[top] = meet;
        edges[top + 1] = f64::INFINITY;
    }
    top = 0;
    for (point, slot) in out.iter_mut().enumerate().take(values.len()) {
        while edges[top + 1] < point as f64 {
            top += 1;
        }
        let gap = point as f64 - hull[top] as f64;
        *slot = gap * gap + values[hull[top]];
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

    #[test]
    fn distances_are_euclidean_to_the_nearest_inside_point() {
        let mut grid = vec![FAR; 5 * 4];
        grid[0] = 0.0;
        grid[2 * 5 + 4] = 0.0;
        squared_distances(&mut grid, 5, 4);
        // Inside points at (0, 0) and (4, 2).
        assert_eq!(grid[0], 0.0);
        assert_eq!(grid[3 * 5], 9.0);
        assert_eq!(grid[5 + 2], 5.0);
        assert_eq!(grid[3 * 5 + 3], 2.0);
    }

    #[test]
    fn dilating_a_point_grows_a_soft_edged_disc() {
        let mut point = Coverage::empty(9, 9);
        point.add(4, 4, 1.0);
        let disc = point.dilated(2.0);
        let at = |x: usize, y: usize| disc.alpha[y * 9 + x];
        assert_eq!((at(5, 5), at(4, 2), at(6, 4)), (1.0, 0.5, 0.5));
        assert_eq!((at(4, 1), at(6, 6)), (0.0, 0.0));
        assert_eq!(disc.ink, (1, 1, 8, 8));
    }
}
