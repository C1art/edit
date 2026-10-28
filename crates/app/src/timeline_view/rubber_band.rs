use egui::{Color32, Painter, Pos2, Rangef, Rect, Stroke};
use timeline::{Clip, Gain, Param, Ticks};

use super::scale::TimeScale;

/// The band spans this range of levels; quieter ones sit on its floor.
const FLOOR_DB: f32 = -60.0;
const CEILING_DB: f32 = 15.0;
/// Leaves the clip's name clear of the line.
const LABEL_HEIGHT: f32 = 15.0;
/// How close, in points, the pointer must be to grab the line or a keyframe.
const GRAB: f32 = 4.0;
const LINE: Color32 = Color32::from_rgb(250, 210, 60);

/// What the pointer is over on an audio clip's volume line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandHit {
    Key(usize),
    /// The line, at this source time.
    Line(Ticks),
}

/// Where the line moves within a clip's rectangle.
pub fn band(rect: Rect) -> Rangef {
    Rangef::new(rect.top() + LABEL_HEIGHT, rect.bottom() - 3.0)
}

fn level_y(band: Rangef, db: f32) -> f32 {
    let fraction = ((db - FLOOR_DB) / (CEILING_DB - FLOOR_DB)).clamp(0.0, 1.0);
    band.max - fraction * band.span()
}

/// The change in decibels for moving the pointer `dy` points down a band.
pub fn db_for_drag(band: Rangef, dy: f32) -> f32 {
    -dy * (CEILING_DB - FLOOR_DB) / band.span().max(1.0)
}

/// The line through the level at both ends of the clip and every keyframe between, left
/// to right.
fn points(clip: &Clip, scale: &TimeScale, band: Rangef) -> Vec<(Pos2, Option<usize>)> {
    let at = |position: Ticks, key| {
        let db = clip.volume.value_at(clip.source_time(position));
        (egui::pos2(scale.x(position), level_y(band, db)), key)
    };
    let last = clip.end() - Ticks(1);
    let mut points: Vec<_> = clip
        .volume
        .keys
        .iter()
        .enumerate()
        .map(|(index, key)| (clip.position_of(key.time), index))
        .filter(|&(position, _)| clip.covers(position))
        .map(|(position, index)| at(position, Some(index)))
        .collect();
    points.push(at(clip.start, None));
    points.push(at(last, None));
    points.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
    if let Some(end) = points.last_mut() {
        end.0.x = scale.x(clip.end());
    }
    points
}

pub fn paint(painter: &Painter, rect: Rect, scale: &TimeScale, clip: &Clip) {
    let points = points(clip, scale, band(rect));
    let painter = painter.with_clip_rect(painter.clip_rect().intersect(rect));
    let line: Vec<Pos2> = points.iter().map(|(pos, _)| *pos).collect();
    painter.add(egui::Shape::line(line, Stroke::new(1.5, LINE)));
    for (pos, _) in points.iter().filter(|(_, key)| key.is_some()) {
        painter.circle(
            *pos,
            3.0,
            LINE,
            Stroke::new(1.0, Color32::from_black_alpha(180)),
        );
    }
}

pub fn hit(rect: Rect, scale: &TimeScale, clip: &Clip, pointer: Pos2) -> Option<BandHit> {
    let points = points(clip, scale, band(rect));
    let key = points
        .iter()
        .filter_map(|&(pos, key)| Some((pos, key?)))
        .find(|(pos, _)| pos.distance(pointer) <= GRAB + 1.0);
    if let Some((_, index)) = key {
        return Some(BandHit::Key(index));
    }
    let on_line = points.windows(2).any(|pair| {
        let [(from, _), (to, _)] = [pair[0], pair[1]];
        if !(from.x..=to.x).contains(&pointer.x) {
            return false;
        }
        let progress = (pointer.x - from.x) / (to.x - from.x).max(f32::EPSILON);
        (from.y + (to.y - from.y) * progress - pointer.y).abs() <= GRAB
    });
    let position = scale
        .time(pointer.x)
        .clamp(clip.start, clip.end() - Ticks(1));
    on_line.then(|| BandHit::Line(clip.source_time(position)))
}

/// `volume` dragged from what the pointer grabbed: a keyframe moves `delta` along the
/// sequence and the level shifts `db`; on the line the level shifts, or the keyframes
/// either side of the grab when animated.
pub fn dragged(clip: &Clip, grabbed: BandHit, delta: Ticks, db: f32) -> Param {
    let mut volume = clip.volume.clone();
    let shift = |hundredths: i32| {
        (hundredths + (db * 100.0).round() as i32).clamp(Gain::SILENT.0, Gain::MAX.0)
    };
    match grabbed {
        BandHit::Key(index) => {
            let source_delta = clip.speed.source_span(delta);
            let source_delta = if clip.reverse {
                Ticks::ZERO - source_delta
            } else {
                source_delta
            };
            let low = index
                .checked_sub(1)
                .map_or(clip.source_in, |before| volume.keys[before].time + Ticks(1));
            let high = volume
                .keys
                .get(index + 1)
                .map_or(clip.source_out() - Ticks(1), |after| after.time - Ticks(1));
            if let Some(key) = volume.keys.get_mut(index) {
                if low <= high {
                    key.time = (key.time + source_delta).clamp(low, high);
                }
                key.hundredths = shift(key.hundredths);
            }
        }
        BandHit::Line(at) if volume.is_animated() => {
            let after = volume.keys.partition_point(|key| key.time <= at);
            let around = after.saturating_sub(1)..(after + 1).min(volume.keys.len());
            for key in &mut volume.keys[around] {
                key.hundredths = shift(key.hundredths);
            }
        }
        BandHit::Line(_) => volume.hundredths = shift(volume.hundredths),
    }
    volume
}
