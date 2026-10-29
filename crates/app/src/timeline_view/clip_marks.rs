use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind};
use timeline::{cut_window, is_through_edit, Clip, Sequence, Ticks, Track};

use super::layout::{clip_rect, Row};
use super::scale::TimeScale;

const OUT_OF_SYNC: Color32 = Color32::from_rgb(200, 30, 30);

/// Premiere's bow-tie at a cut the media plays straight through, which can be joined.
pub(super) fn through_edit_marks(painter: &Painter, row: &Row, scale: &TimeScale, track: &Track) {
    let fill = Color32::from_white_alpha(200);
    for pair in track.clips.windows(2) {
        if !is_through_edit(&pair[0], &pair[1]) {
            continue;
        }
        let rect = clip_rect(row, scale, pair[0].start, pair[1].end());
        let x = scale.x(pair[1].start);
        for (edge, inward) in [(rect.top() + 1.0, 5.0), (rect.bottom() - 1.0, -5.0)] {
            for side in [-4.0, 4.0] {
                let points = vec![
                    egui::pos2(x, edge + inward),
                    egui::pos2(x + side, edge),
                    egui::pos2(x, edge),
                ];
                painter.add(egui::Shape::convex_polygon(points, fill, Stroke::NONE));
            }
        }
    }
}

/// Premiere's red frame count on a clip slipped out of sync with its link; returns the
/// width it takes so the name can follow it.
pub(super) fn sync_badge(
    painter: &Painter,
    rect: Rect,
    sequence: &Sequence,
    offset: Option<&Ticks>,
) -> f32 {
    let Some(&offset) = offset else {
        return 0.0;
    };
    let frames = sequence
        .format
        .frame_rate
        .frame_at(Ticks(offset.0.abs()), sequence.timebase);
    let sign = if offset > Ticks::ZERO { '+' } else { '-' };
    let painter = painter.with_clip_rect(rect.shrink(1.0));
    let text = painter.layout_no_wrap(
        format!("{sign}{frames}"),
        FontId::proportional(11.0),
        Color32::WHITE,
    );
    let badge = Rect::from_min_size(
        rect.left_top() + egui::vec2(2.0, 2.0),
        text.size() + egui::vec2(6.0, 2.0),
    );
    painter.rect_filled(badge, 2.0, OUT_OF_SYNC);
    painter.galley(badge.min + egui::vec2(3.0, 1.0), text, Color32::WHITE);
    badge.width() + 2.0
}

/// A diamond at the bottom of the clip for each keyframe it shows.
pub(super) fn keyframe_marks(painter: &Painter, rect: Rect, scale: &TimeScale, clip: &Clip) {
    let y = rect.bottom() - 6.0;
    for time in clip.look.key_times() {
        let position = clip.position_of(time);
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

/// A notch along the top of the clip at each speed keyframe.
pub(super) fn speed_marks(painter: &Painter, rect: Rect, scale: &TimeScale, clip: &Clip) {
    let Some(remap) = &clip.remap else {
        return;
    };
    let stroke = Stroke::new(2.0, Color32::WHITE);
    for anchor in remap.anchors() {
        let position = clip.start + anchor.offset;
        if position <= clip.start || !clip.covers(position) {
            continue;
        }
        let x = scale.x(position);
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.top() + 8.0)],
            stroke,
        );
    }
}

/// Shades where a clip's transitions play: across a shared cut or inside the clip.
pub(super) fn transition_marks(
    painter: &Painter,
    row: &Row,
    scale: &TimeScale,
    track: &Track,
    clip: &Clip,
) {
    let mut spans = Vec::new();
    if let Some(transition) = clip.transition_in {
        let previous = track.clips.iter().find(|other| other.end() == clip.start);
        let span = match previous {
            Some(previous) => {
                let (before, after) = cut_window(previous, clip, transition);
                (clip.start - before, clip.start + after)
            }
            None => (
                clip.start,
                clip.start + transition.duration.min(clip.duration),
            ),
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
