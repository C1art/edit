use egui::{Color32, Painter, Pos2, Rangef, Rect, Stroke};
use timeline::{Clip, ClipId, Gain, Param, Sequence, Ticks, Track, TrackRef};

use super::scale::TimeScale;

/// The band spans this range of levels; quieter ones sit on its floor.
const FLOOR_DB: f32 = -60.0;
const CEILING_DB: f32 = 15.0;
/// Leaves the clip's name clear of the line.
const LABEL_HEIGHT: f32 = 15.0;
/// How close, in points, the pointer must be to grab the line or a keyframe.
const GRAB: f32 = 4.0;
/// Points between samples of an animated line.
const SAMPLE_SPACING: f32 = 3.0;
const LINE: Color32 = Color32::from_rgb(250, 210, 60);
/// Track automation, told apart from the clip lines it replaces.
const TRACK_LINE: Color32 = Color32::from_rgb(255, 135, 60);

/// Whose volume line a band shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandOwner {
    Clip(ClipId),
    /// The track's automation, shown across its whole row.
    Track(TrackRef),
}

/// A volume line placed on the timeline: a clip's, keyed in its source time, or a
/// track's, keyed in sequence time.
pub struct Envelope<'a> {
    volume: &'a Param,
    /// The sequence span the line is drawn over.
    span: (Ticks, Ticks),
    clip: Option<&'a Clip>,
}

impl<'a> Envelope<'a> {
    pub fn clip(clip: &'a Clip) -> Self {
        Self {
            volume: &clip.volume,
            span: (clip.start, clip.end()),
            clip: Some(clip),
        }
    }

    /// The track's automation, drawn up to `end`.
    pub fn track(track: &'a Track, end: Ticks) -> Self {
        Self {
            volume: &track.automation,
            span: (Ticks::ZERO, end),
            clip: None,
        }
    }

    fn time(&self, position: Ticks) -> Ticks {
        self.clip
            .map_or(position, |clip| clip.source_time(position))
    }

    fn position(&self, time: Ticks) -> Ticks {
        self.clip.map_or(time, |clip| clip.position_of(time))
    }

    pub fn key_position(&self, index: usize) -> Option<Ticks> {
        Some(self.position(self.volume.keys.get(index)?.time))
    }

    /// How far a key's time moves for a drag of `delta` along the sequence.
    fn key_delta(&self, delta: Ticks) -> Ticks {
        let Some(clip) = self.clip else {
            return delta;
        };
        let source_delta = clip.rate().source_span(delta);
        if clip.reverse {
            Ticks::ZERO - source_delta
        } else {
            source_delta
        }
    }

    /// The earliest and latest times a key may take.
    fn key_bounds(&self) -> (Ticks, Ticks) {
        self.clip.map_or((Ticks::ZERO, Ticks(i64::MAX)), |clip| {
            (clip.source_in, clip.source_out() - Ticks(1))
        })
    }

    /// The level at the time a click at `position` lands on.
    pub fn time_at(&self, position: Ticks) -> Ticks {
        self.time(position.clamp(self.span.0, self.span.1 - Ticks(1)))
    }

    pub fn volume(&self) -> &Param {
        self.volume
    }

    /// The line `owner` shows in `sequence`, a track's drawn up to `end`.
    pub fn of(sequence: &'a Sequence, owner: BandOwner, end: Ticks) -> Option<Self> {
        match owner {
            BandOwner::Clip(id) => sequence.clip(id).map(|(_, clip)| Self::clip(clip)),
            BandOwner::Track(track) => sequence.track(track).map(|track| Self::track(track, end)),
        }
    }
}

/// What the pointer is over on a volume line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandHit {
    Key(usize),
    /// The line, at this time of the line's owner.
    Line(Ticks),
}

/// Where the line moves within a clip's rectangle or a track's row.
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

/// The line through the level at both ends of the span and every keyframe between, left
/// to right, sampled within `visible` so eased and held segments show their shape.
fn points(
    envelope: &Envelope,
    scale: &TimeScale,
    band: Rangef,
    visible: Rangef,
) -> Vec<(Pos2, Option<usize>)> {
    let at = |position: Ticks, key| {
        let db = envelope.volume.value_at(envelope.time(position));
        (egui::pos2(scale.x(position), level_y(band, db)), key)
    };
    let (start, end) = envelope.span;
    let last = end - Ticks(1);
    let mut points: Vec<_> = envelope
        .volume
        .keys
        .iter()
        .enumerate()
        .map(|(index, key)| (envelope.position(key.time), index))
        .filter(|&(position, _)| (start..end).contains(&position))
        .map(|(position, index)| at(position, Some(index)))
        .collect();
    points.push(at(start, None));
    points.push(at(last, None));
    if envelope.volume.is_animated() {
        let from = visible.min.max(scale.x(start));
        let to = visible.max.min(scale.x(last));
        let samples = ((to - from) / SAMPLE_SPACING).ceil().max(0.0) as usize;
        points.extend((1..samples).map(|step| {
            let x = from + step as f32 * SAMPLE_SPACING;
            at(scale.time(x).clamp(start, last), None)
        }));
    }
    points.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
    if let Some(last) = points.last_mut() {
        last.0.x = scale.x(end);
    }
    points
}

pub fn paint(painter: &Painter, rect: Rect, scale: &TimeScale, envelope: &Envelope) {
    let painter = painter.with_clip_rect(painter.clip_rect().intersect(rect));
    let points = points(envelope, scale, band(rect), painter.clip_rect().x_range());
    let color = if envelope.clip.is_some() {
        LINE
    } else {
        TRACK_LINE
    };
    let line: Vec<Pos2> = points.iter().map(|(pos, _)| *pos).collect();
    painter.add(egui::Shape::line(line, Stroke::new(1.5, color)));
    for (pos, _) in points.iter().filter(|(_, key)| key.is_some()) {
        painter.circle(
            *pos,
            3.0,
            color,
            Stroke::new(1.0, Color32::from_black_alpha(180)),
        );
    }
}

pub fn hit(rect: Rect, scale: &TimeScale, envelope: &Envelope, pointer: Pos2) -> Option<BandHit> {
    let near = Rangef::new(pointer.x - 2.0 * GRAB, pointer.x + 2.0 * GRAB);
    let points = points(envelope, scale, band(rect), near);
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
    on_line.then(|| BandHit::Line(envelope.time_at(scale.time(pointer.x))))
}

/// `volume` dragged from what the pointer grabbed: a keyframe moves `delta` along the
/// sequence and the level shifts `db`; on the line the level shifts, or the keyframes
/// either side of the grab when animated.
pub fn dragged(envelope: &Envelope, grabbed: BandHit, delta: Ticks, db: f32) -> Param {
    let mut volume = envelope.volume.clone();
    let shift = |hundredths: i32| {
        (hundredths + (db * 100.0).round() as i32).clamp(Gain::SILENT.0, Gain::MAX.0)
    };
    match grabbed {
        BandHit::Key(index) => {
            let key_delta = envelope.key_delta(delta);
            let (first, last) = envelope.key_bounds();
            let low = index
                .checked_sub(1)
                .map_or(first, |before| volume.keys[before].time + Ticks(1));
            let high = volume
                .keys
                .get(index + 1)
                .map_or(last, |after| after.time - Ticks(1));
            if let Some(key) = volume.keys.get_mut(index) {
                if low <= high {
                    key.time = (key.time + key_delta).clamp(low, high);
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
