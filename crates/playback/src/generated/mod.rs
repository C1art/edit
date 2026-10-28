mod coverage;

use std::cell::RefCell;
use std::sync::Arc;

use media::VideoFrame;
use timeline::{CaptionStyle, Generator, Rgb, TextAlign, Ticks, Title, TitleFont};

use coverage::Coverage;

const SHADOW_OPACITY: f32 = 0.7;
/// Faux bold thickens the strokes of the built-in light face by this fraction of the size
/// each way.
const BOLD_GROWTH: f32 = 1.0 / 40.0;
/// Premiere's red, so a missing file can't pass for a dark shot.
/// The width of a monospace character in the built-in face, in sizes, with room to
/// spare so the line never wraps.
const TIMECODE_ADVANCE: f32 = 0.7;
/// Premiere's red, so a missing file can't pass for a dark shot.
const OFFLINE_RED: Rgb = Rgb::new(200, 30, 30);

/// The caption last drawn, and what it was drawn from, since one shows for many frames.
type CaptionCache = Option<((String, CaptionStyle, u32, u32), Arc<VideoFrame>)>;

thread_local! {
    static CAPTION: RefCell<CaptionCache> = const { RefCell::new(None) };
    static OFFLINE: RefCell<Option<Arc<VideoFrame>>> = const { RefCell::new(None) };
}

/// Where laid out text goes on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Anchor {
    Center,
    Bottom,
}

/// The picture `generator` draws on a `width` by `height` canvas, with straight alpha.
pub fn generated_frame(generator: &Generator, width: u32, height: u32) -> Arc<VideoFrame> {
    let mut canvas = Canvas::new(width, height);
    match generator {
        Generator::ColorMatte { color } => canvas.fill(*color),
        Generator::Title(title) => draw_title(&mut canvas, title, Anchor::Center),
        Generator::AdjustmentLayer => {}
    }
    Arc::new(VideoFrame {
        pts: Ticks::ZERO,
        width,
        height,
        pixels: canvas.into_pixels(),
    })
}

/// What a clip whose file can't be opened shows in place of its pictures.
pub(crate) fn offline_frame(width: u32, height: u32) -> Arc<VideoFrame> {
    OFFLINE.with_borrow_mut(|cache| {
        if let Some(frame) = cache
            .as_ref()
            .filter(|frame| (frame.width, frame.height) == (width, height))
        {
            return Arc::clone(frame);
        }
        let title = Title {
            text: "Media Offline".to_owned(),
            size: (height / 10).max(8),
            color: Rgb::WHITE,
            align: TextAlign::Center,
            font: TitleFont::Sans,
            shadow: false,
            bold: true,
            ..Title::default()
        };
        let mut canvas = Canvas::new(width, height);
        canvas.fill(OFFLINE_RED);
        draw_title(&mut canvas, &title, Anchor::Center);
        let frame = Arc::new(VideoFrame {
            pts: Ticks::ZERO,
            width,
            height,
            pixels: canvas.into_pixels(),
        });
        *cache = Some(Arc::clone(&frame));
        frame
    })
}

/// `text` in white monospace `size` pixels tall on a black box of `opacity`, as the
/// Timecode effect draws it.
pub(crate) fn timecode_box(text: &str, size: u32, opacity: f32) -> VideoFrame {
    let characters = text.chars().count() as f32;
    let width = (size as f32 * (TIMECODE_ADVANCE * characters + 1.0)).ceil() as u32;
    let height = (size as f32 * 1.5).ceil() as u32;
    let title = Title {
        text: text.to_owned(),
        size,
        color: Rgb::WHITE,
        align: TextAlign::Center,
        font: TitleFont::Mono,
        shadow: false,
        ..Title::default()
    };
    let mut canvas = Canvas::new(width, height);
    canvas.pixels.fill([0.0, 0.0, 0.0, opacity]);
    draw_title(&mut canvas, &title, Anchor::Center);
    VideoFrame {
        pts: Ticks::ZERO,
        width,
        height,
        pixels: canvas.into_pixels(),
    }
}

/// `text` drawn in `style` near the bottom of a `width` by `height` canvas.
pub(crate) fn caption_frame(
    text: &str,
    style: CaptionStyle,
    width: u32,
    height: u32,
) -> Arc<VideoFrame> {
    CAPTION.with_borrow_mut(|cache| {
        if let Some((key, frame)) = cache.as_ref() {
            if key.0 == text && (key.1, key.2, key.3) == (style, width, height) {
                return Arc::clone(frame);
            }
        }
        let title = Title {
            text: text.to_owned(),
            size: style.size,
            color: style.color,
            align: TextAlign::Center,
            font: TitleFont::Sans,
            shadow: style.background.is_none(),
            background: style.background,
            ..Title::default()
        };
        let mut canvas = Canvas::new(width, height);
        draw_title(&mut canvas, &title, Anchor::Bottom);
        let frame = Arc::new(VideoFrame {
            pts: Ticks::ZERO,
            width,
            height,
            pixels: canvas.into_pixels(),
        });
        *cache = Some(((text.to_owned(), style, width, height), Arc::clone(&frame)));
        frame
    })
}

fn draw_title(canvas: &mut Canvas, title: &Title, anchor: Anchor) {
    let Some(mut text) = Coverage::of_title(title, canvas.width, canvas.height, anchor) else {
        return;
    };
    let size = title.size as f32;
    if title.bold {
        text = text.dilated(size * BOLD_GROWTH);
    }
    let outline = title
        .stroke
        .filter(|stroke| stroke.width > 0)
        .map(|stroke| (text.dilated(stroke.width as f32), stroke.color));
    if let Some(background) = title.background {
        let pad = (size * 0.3).round() as i64;
        let grown = title.stroke.map_or(0, |stroke| i64::from(stroke.width));
        let (left, top, right, bottom) = text.bounds;
        canvas.fill_box(
            (
                left - pad - grown,
                top - pad / 2 - grown,
                right + pad + grown,
                bottom + pad / 2 + grown,
            ),
            background,
        );
    }
    let silhouette = outline.as_ref().map_or(&text, |(outline, _)| outline);
    if title.shadow {
        let offset = (size / 24.0).round().max(1.0) as i64;
        let blur = (size / 32.0).round() as usize;
        let shadow = silhouette.shifted(offset, offset).blurred(blur);
        canvas.paint(&shadow, Rgb::BLACK, SHADOW_OPACITY);
    }
    // Text over a transparent canvas would otherwise sample black fringes from the
    // pixels around it when scaled.
    if title.background.is_none() && !title.shadow {
        canvas.bleed(outline.as_ref().map_or(title.color, |&(_, color)| color));
    }
    if let Some((outline, color)) = &outline {
        canvas.paint(outline, *color, 1.0);
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
