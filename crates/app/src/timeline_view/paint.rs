use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind};
use timeline::{Clip, ClipId, Gain, Sequence, Ticks, Timecode, Track, TrackKind};

use super::layout::{clip_rect, Row};
use super::scale::{label_interval, TimeScale};

pub const PLAYHEAD: Color32 = Color32::from_rgb(230, 70, 60);
const VIDEO_CLIP: Color32 = Color32::from_rgb(64, 104, 160);
const AUDIO_CLIP: Color32 = Color32::from_rgb(58, 128, 92);
const MIN_LABEL_SPACING: f32 = 90.0;

pub fn ruler(painter: &Painter, rect: Rect, scale: &TimeScale, sequence: &Sequence) {
    let visuals = painter.ctx().global_style().visuals.clone();
    painter.rect_filled(rect, 0.0, visuals.faint_bg_color);
    let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
    let interval = label_interval(scale, rate, timebase, MIN_LABEL_SPACING);
    let minor = if interval % 5 == 0 {
        interval / 5
    } else {
        interval
    };
    let first = rate.frame_at(scale.time(rect.left()), timebase).max(0);
    let last = rate.frame_at(scale.time(rect.right()), timebase) + 1;
    let mut frame = first - first % minor;
    let color = visuals.text_color();
    while frame <= last {
        let time = rate.frame_start(frame, timebase);
        let x = scale.x(time);
        let major = frame % interval == 0;
        let height = if major { 10.0 } else { 4.0 };
        painter.line_segment(
            [
                egui::pos2(x, rect.bottom() - height),
                egui::pos2(x, rect.bottom()),
            ],
            Stroke::new(1.0, color.gamma_multiply(0.6)),
        );
        if major {
            painter.text(
                egui::pos2(x + 3.0, rect.top() + 2.0),
                Align2::LEFT_TOP,
                Timecode::new(time, timebase, rate).to_string(),
                FontId::monospace(10.0),
                color,
            );
        }
        frame += minor;
    }
}

pub fn row_background(
    painter: &Painter,
    rect: Rect,
    row: &Row,
    sequence: &Sequence,
    targeted: bool,
) {
    let visuals = painter.ctx().global_style().visuals.clone();
    let mut fill = visuals.extreme_bg_color;
    if targeted {
        fill = fill.lerp_to_gamma(visuals.selection.bg_fill, 0.12);
    }
    painter.rect_filled(
        Rect::from_x_y_ranges(rect.x_range(), row.y_range()),
        0.0,
        fill,
    );
    if sequence.track(row.track).is_some_and(|track| track.locked) {
        painter.rect_filled(
            Rect::from_x_y_ranges(rect.x_range(), row.y_range()),
            0.0,
            Color32::from_black_alpha(90),
        );
    }
    painter.hline(
        rect.x_range(),
        row.top + super::layout::ROW_HEIGHT,
        Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
    );
}

pub fn clips(
    painter: &Painter,
    row: &Row,
    scale: &TimeScale,
    sequence: &Sequence,
    selection: &[ClipId],
) {
    let Some(track) = sequence.track(row.track) else {
        return;
    };
    let base = match row.track.kind {
        TrackKind::Video => VIDEO_CLIP,
        TrackKind::Audio => AUDIO_CLIP,
    };
    let visible = painter.clip_rect().x_range();
    for clip in &track.clips {
        let rect = clip_rect(row, scale, clip.start, clip.end());
        if rect.right() < visible.min || rect.left() > visible.max {
            continue;
        }
        let selected = selection.contains(&clip.id);
        let mut fill = if selected {
            base.lerp_to_gamma(Color32::WHITE, 0.25)
        } else {
            base
        };
        if track.muted {
            fill = fill.gamma_multiply(0.4);
        }
        painter.rect_filled(rect, 3.0, fill);
        let stroke = if selected {
            Stroke::new(2.0, Color32::WHITE)
        } else {
            Stroke::new(1.0, Color32::from_black_alpha(120))
        };
        painter.rect_stroke(rect, 3.0, stroke, StrokeKind::Inside);
        transition_marks(painter, row, scale, track, clip);
        keyframe_marks(painter, rect, scale, clip);
        let name = sequence
            .media_item(clip.source)
            .map_or("Missing media", |item| item.name.as_str());
        let link = if clip.link.is_some() {
            ""
        } else {
            " [unlinked]"
        };
        let gain = if row.track.kind == TrackKind::Audio && clip.gain != Gain::UNITY {
            format!(" {:+.1} dB", clip.gain.db())
        } else {
            String::new()
        };
        painter.with_clip_rect(rect.shrink(2.0)).text(
            rect.left_top() + egui::vec2(5.0, 4.0),
            Align2::LEFT_TOP,
            format!("{name}{link}{gain}"),
            FontId::proportional(11.0),
            Color32::WHITE,
        );
    }
}

/// A diamond at the bottom of the clip for each keyframe it shows.
fn keyframe_marks(painter: &Painter, rect: Rect, scale: &TimeScale, clip: &Clip) {
    let y = rect.bottom() - 6.0;
    for time in clip.look.key_times() {
        let position = clip.start + (time - clip.source_in);
        if !clip.covers(position) {
            continue;
        }
        let x = scale.x(position);
        let points = [(0.0, -4.0), (4.0, 0.0), (0.0, 4.0), (-4.0, 0.0)]
            .map(|(dx, dy)| egui::pos2(x + dx, y + dy))
            .to_vec();
        painter.add(egui::Shape::convex_polygon(
            points,
            Color32::from_rgb(250, 210, 60),
            Stroke::new(1.0, Color32::from_black_alpha(160)),
        ));
    }
}

/// Shades where a clip's transitions play: across a shared cut or inside the clip.
fn transition_marks(painter: &Painter, row: &Row, scale: &TimeScale, track: &Track, clip: &Clip) {
    let follows = |start: Ticks| track.clips.iter().any(|other| other.end() == start);
    let mut spans = Vec::new();
    if let Some(transition) = clip.transition_in {
        let span = if follows(clip.start) {
            let half = Ticks(transition.duration.0 / 2);
            (clip.start - half, clip.start - half + transition.duration)
        } else {
            (
                clip.start,
                clip.start + transition.duration.min(clip.duration),
            )
        };
        spans.push((span, transition.kind));
    }
    if let Some(transition) = clip.transition_out {
        if !track.clips.iter().any(|next| next.start == clip.end()) {
            let duration = transition.duration.min(clip.duration);
            spans.push(((clip.end() - duration, clip.end()), transition.kind));
        }
    }
    for ((start, end), kind) in spans {
        let rect = clip_rect(row, scale, start, end).shrink2(egui::vec2(0.0, 2.0));
        painter.rect_filled(rect, 2.0, Color32::from_white_alpha(50));
        painter.rect_stroke(
            rect,
            2.0,
            Stroke::new(1.0, Color32::from_white_alpha(140)),
            StrokeKind::Inside,
        );
        painter.with_clip_rect(rect).text(
            rect.left_bottom() + egui::vec2(3.0, -2.0),
            Align2::LEFT_BOTTOM,
            kind.name(),
            FontId::proportional(9.0),
            Color32::WHITE,
        );
    }
}

pub fn playhead(painter: &Painter, ruler: Rect, bottom: f32, scale: &TimeScale, position: Ticks) {
    let x = scale.x(position);
    if !ruler.x_range().contains(x) {
        return;
    }
    painter.line_segment(
        [egui::pos2(x, ruler.top()), egui::pos2(x, bottom)],
        Stroke::new(1.5, PLAYHEAD),
    );
    let top = ruler.bottom() - 10.0;
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(x - 6.0, top),
            egui::pos2(x + 6.0, top),
            egui::pos2(x, ruler.bottom()),
        ],
        PLAYHEAD,
        Stroke::NONE,
    ));
}

/// A vertical guide where a drop or snap will land.
pub fn guide(painter: &Painter, x: f32, top: f32, bottom: f32) {
    painter.line_segment(
        [egui::pos2(x, top), egui::pos2(x, bottom)],
        Stroke::new(1.5, Color32::from_rgb(250, 210, 60)),
    );
}
