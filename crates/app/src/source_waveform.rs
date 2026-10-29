use egui::{Color32, Mesh, Painter, Rect, Response, Stroke};
use timeline::{MediaItem, Ticks};

use crate::clip_art::ClipArt;
use crate::source::SourceMonitor;
use crate::timeline_view::decibel_scale;

const WAVEFORM: Color32 = Color32::from_rgb(120, 200, 150);
const MARKED: Color32 = Color32::from_rgba_premultiplied(40, 70, 120, 90);

/// Draws the sound of a source that has no pictures across the monitor, as Premiere
/// does, with its marked range and playhead.
pub(crate) fn paint(
    painter: &Painter,
    rect: Rect,
    source: &SourceMonitor,
    item: &MediaItem,
    art: &mut ClipArt,
) {
    let duration = source.duration();
    if !item.is_file() || duration <= Ticks::ZERO || rect.width() < 1.0 {
        return;
    }
    let ticks_per_point = duration.0 as f64 / f64::from(rect.width());
    let time_at = |x: f32| Ticks((f64::from(x - rect.left()) * ticks_per_point).round() as i64);
    let x_at = |time: Ticks| rect.left() + (time.0 as f64 / ticks_per_point) as f32;

    if source.marks.is_set() {
        let marked = source.marks.range(duration);
        let band = Rect::from_x_y_ranges(x_at(marked.start)..=x_at(marked.end), rect.y_range());
        painter.rect_filled(band, 0.0, MARKED);
    }
    if let Some(peaks) = art.peaks(&item.path) {
        let timebase = source.viewer.player.sequence().timebase;
        let bars = rect.shrink2(egui::vec2(0.0, rect.height() * 0.1));
        let (middle, half) = (bars.center().y, bars.height() / 2.0);
        let mut mesh = Mesh::default();
        let mut x = rect.left();
        while x < rect.right() {
            let (start, length) = source.media_range(time_at(x)..time_at(x + 1.0));
            let level = peaks.max_in(start, start + length, timebase);
            let height = (decibel_scale(level) * half).max(0.5);
            mesh.add_colored_rect(
                Rect::from_x_y_ranges(x..=x + 1.0, (middle - height)..=(middle + height)),
                WAVEFORM,
            );
            x += 1.0;
        }
        painter.with_clip_rect(rect).add(mesh);
    }
    let playhead = x_at(source.viewer.player.position());
    painter.line_segment(
        [
            egui::pos2(playhead, rect.top()),
            egui::pos2(playhead, rect.bottom()),
        ],
        Stroke::new(1.0, crate::theme::ACCENT),
    );
}

/// Moves the playhead to where the waveform was clicked.
pub(crate) fn seek_on_click(picture: &Response, source: &mut SourceMonitor) {
    let Some(pointer) = picture.interact_pointer_pos().filter(|_| picture.clicked()) else {
        return;
    };
    let rect = picture.rect;
    let along = f64::from((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    let time = Ticks((source.duration().0 as f64 * along).round() as i64);
    source.viewer.player.seek(time);
}
