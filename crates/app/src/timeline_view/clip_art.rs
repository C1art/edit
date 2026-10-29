use egui::{Color32, Mesh, Painter, Rect};
use timeline::{Clip, Generator, MediaItem, Ticks, Timebase};

use super::scale::TimeScale;
use crate::clip_art::ClipArt;

/// Room left at the top of an audio clip for its name.
const LABEL_HEIGHT: f32 = 15.0;
/// Quietest level drawn. Bars are scaled in decibels, as other editors do by default, so
/// dialogue recorded well below full scale still reads clearly.
const FLOOR_DB: f32 = -48.0;

/// A clip on screen and the media it shows.
pub struct ClipArea<'a> {
    pub rect: Rect,
    pub clip: &'a Clip,
    pub item: &'a MediaItem,
    pub timebase: Timebase,
}

/// Fills a video clip with pictures of its media, one per tile across its width.
pub fn thumbnails(
    painter: &Painter,
    area: &ClipArea<'_>,
    scale: &TimeScale,
    art: &mut ClipArt,
    tint: Color32,
) {
    let ClipArea {
        rect,
        clip,
        item,
        timebase,
    } = *area;
    match &item.generator {
        Some(Generator::ColorMatte { color }) => {
            let color = Color32::from_rgb(color.r, color.g, color.b);
            painter.rect_filled(rect, 2.0, color * tint);
            return;
        }
        Some(Generator::BlackVideo) => {
            painter.rect_filled(rect, 2.0, Color32::BLACK * tint);
            return;
        }
        Some(
            Generator::Title(_)
            | Generator::AdjustmentLayer
            | Generator::Shape(_)
            | Generator::BarsAndTone
            | Generator::TransparentVideo
            | Generator::CountingLeader(_)
            | Generator::Graphic(_),
        ) => return,
        None if item.nest.is_some() => return,
        None => {}
    }
    let Some(format) = item.video else {
        return;
    };
    let tile_width = rect.height() * format.width as f32 / format.height.max(1) as f32;
    let frame = format.frame_rate.frame_start(1, timebase).0.max(1);
    // Times snap to a power-of-two count of frames, so the same pictures are reused while
    // scrolling and only zooming past a power of two asks for new ones.
    let tile_frames = (clip.rate().source_span(scale.span(tile_width)).0 + frame - 1) / frame;
    let step = i64::try_from(
        u64::try_from(tile_frames.max(1))
            .unwrap_or(1)
            .next_power_of_two(),
    )
    .unwrap_or(i64::MAX)
    .saturating_mul(frame);
    let path = item.proxy.as_deref().unwrap_or(&item.path);
    let last = (item.duration.0 - frame).max(0);
    let visible = painter.clip_rect().intersect(rect);
    if visible.width() <= 0.0 || tile_width < 1.0 {
        return;
    }
    let first_tile = ((visible.left() - rect.left()) / tile_width).floor() as i64;
    let last_tile = ((visible.right() - rect.left()) / tile_width).ceil() as i64;
    let painter = painter.with_clip_rect(visible);
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    for tile in first_tile..last_tile {
        let left = rect.left() + tile as f32 * tile_width;
        let at = clip
            .source_time(clip.start + scale.span(left - rect.left()))
            .0;
        let at = Ticks((at.max(0) / step * step).min(last));
        if let Some(texture) = art.picture(path, at, timebase) {
            let tile_rect = Rect::from_min_size(
                egui::pos2(left, rect.top()),
                egui::vec2(tile_width, rect.height()),
            );
            painter.image(texture, tile_rect, uv, tint);
        }
    }
}

/// Draws an audio clip's loudness as bars mirrored about its middle, one per point.
pub fn waveform(
    painter: &Painter,
    area: &ClipArea<'_>,
    scale: &TimeScale,
    art: &mut ClipArt,
    color: Color32,
) {
    let ClipArea {
        rect,
        clip,
        item,
        timebase,
    } = *area;
    if !item.is_file() {
        return;
    }
    let Some(peaks) = art.peaks(&item.path) else {
        return;
    };
    let bars = Rect::from_x_y_ranges(rect.x_range(), (rect.top() + LABEL_HEIGHT)..=rect.bottom());
    let visible = painter.clip_rect().intersect(bars);
    if visible.width() <= 0.0 {
        return;
    }
    let (middle, half) = (bars.center().y, bars.height() / 2.0);
    let gain = clip.gain.amplitude();
    let column = clip.rate().source_span(scale.span(1.0));
    let mut mesh = Mesh::default();
    let mut x = visible.left().floor();
    while x < visible.right() {
        let start = clip.source_time(clip.start + scale.span(x - rect.left()));
        let level = peaks.max_in(start, start + column, timebase) * gain;
        let height = (decibel_scale(level) * half).max(0.5);
        mesh.add_colored_rect(
            Rect::from_x_y_ranges(x..=x + 1.0, (middle - height)..=(middle + height)),
            color,
        );
        x += 1.0;
    }
    painter.with_clip_rect(visible).add(mesh);
}

/// Maps an amplitude to 0 at `FLOOR_DB` and below, rising to 1 at full scale.
fn decibel_scale(amplitude: f32) -> f32 {
    if amplitude <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * amplitude.log10();
    (1.0 - db / FLOOR_DB).clamp(0.0, 1.0)
}
