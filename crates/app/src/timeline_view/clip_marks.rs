use std::ops::Range;
use std::sync::LazyLock;

use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind};
use timeline::{is_through_edit, Clip, Look, Param, Sequence, Ticks, Track, TrackKind};

use super::layout::{clip_rect, Row};
use super::scale::TimeScale;
use super::transition_grip;

const OUT_OF_SYNC: Color32 = Color32::from_rgb(200, 30, 30);
const APPLIED_EFFECTS: Color32 = Color32::from_rgb(196, 150, 230);
const CHANGED_PROPERTIES: Color32 = Color32::from_rgb(230, 200, 80);

/// Kept rather than built for every clip painted.
static UNTOUCHED: LazyLock<Look> = LazyLock::new(Look::default);

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
        .frames_in(Ticks(offset.0.abs()), sequence.timebase);
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

/// Premiere's fx badge after the name's other badges, starting `left` points in: purple
/// once effects are applied, yellow when only the clip's own properties are changed.
/// Returns the width it takes.
pub(super) fn fx_badge(
    painter: &Painter,
    rect: Rect,
    left: f32,
    kind: TrackKind,
    clip: &Clip,
) -> f32 {
    let fill = match kind {
        TrackKind::Video if !clip.look.effects.is_empty() => APPLIED_EFFECTS,
        TrackKind::Video if clip.look != *UNTOUCHED => CHANGED_PROPERTIES,
        TrackKind::Audio if !clip.audio_effects.is_empty() => APPLIED_EFFECTS,
        TrackKind::Audio if clip.volume != Param::default() || clip.pan != Param::default() => {
            CHANGED_PROPERTIES
        }
        _ => return 0.0,
    };
    let painter = painter.with_clip_rect(rect.shrink(1.0));
    let text = painter.layout_no_wrap("fx".to_owned(), FontId::proportional(10.0), Color32::BLACK);
    let badge = Rect::from_min_size(
        rect.left_top() + egui::vec2(2.0 + left, 2.0),
        text.size() + egui::vec2(6.0, 2.0),
    );
    painter.rect_filled(badge, 2.0, fill);
    painter.galley(badge.min + egui::vec2(3.0, 1.0), text, Color32::BLACK);
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
    for shown in transition_grip::shown(track, clip) {
        let (start, end, kind) = (shown.span.start, shown.span.end, shown.transition.kind);
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

/// Colors told apart at a glance, one per source, as Premiere's duplicate frame markers.
const DUPLICATE_COLORS: [Color32; 8] = [
    Color32::from_rgb(235, 90, 90),
    Color32::from_rgb(240, 170, 60),
    Color32::from_rgb(230, 220, 80),
    Color32::from_rgb(120, 210, 90),
    Color32::from_rgb(70, 200, 200),
    Color32::from_rgb(90, 140, 240),
    Color32::from_rgb(170, 110, 230),
    Color32::from_rgb(230, 110, 190),
];
const DUPLICATE_HEIGHT: f32 = 3.0;

/// A strip along the foot of `body` over the `ranges` of `clip` that show footage
/// shown elsewhere in the sequence.
pub(super) fn duplicate_marks(
    painter: &Painter,
    body: Rect,
    scale: &TimeScale,
    clip: &Clip,
    ranges: &[Range<Ticks>],
) {
    let palette_index = (clip.source.0 % DUPLICATE_COLORS.len() as u64) as usize;
    let color = DUPLICATE_COLORS[palette_index];
    let foot = body.bottom() - 1.0;
    for range in ranges {
        let across = scale.x(range.start).max(body.left())..=scale.x(range.end).min(body.right());
        let strip = Rect::from_x_y_ranges(across, foot - DUPLICATE_HEIGHT..=foot);
        painter.rect_filled(strip, 0.0, color);
    }
}
