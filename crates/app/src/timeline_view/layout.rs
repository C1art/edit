use egui::{Pos2, Rect};
use timeline::{ClipId, Edge, Sequence, Ticks, TrackRef};

use super::scale::TimeScale;

pub const ROW_HEIGHT: f32 = 46.0;
/// Space between the video and audio tracks.
pub const DIVIDER: f32 = 6.0;
/// How close to an edge, in points, the pointer must be to grab it.
const EDGE_GRAB: f32 = 6.0;

#[derive(Debug, Clone, Copy)]
pub struct Row {
    pub track: TrackRef,
    pub top: f32,
}

impl Row {
    pub fn y_range(&self) -> egui::Rangef {
        egui::Rangef::new(self.top, self.top + ROW_HEIGHT)
    }
}

/// Video tracks stacked with V1 at the bottom, then audio tracks with A1 at the top, as
/// in other editors.
pub fn rows(sequence: &Sequence, top: f32) -> Vec<Row> {
    let video = (0..sequence.video.len()).rev().map(TrackRef::video);
    let audio = (0..sequence.audio.len()).map(TrackRef::audio);
    let mut y = top;
    video
        .chain(audio)
        .map(|track| {
            if track == TrackRef::audio(0) {
                y += DIVIDER;
            }
            let row = Row { track, top: y };
            y += ROW_HEIGHT;
            row
        })
        .collect()
}

pub fn total_height(sequence: &Sequence) -> f32 {
    (sequence.video.len() + sequence.audio.len()) as f32 * ROW_HEIGHT + DIVIDER
}

pub fn row_at(rows: &[Row], y: f32) -> Option<Row> {
    rows.iter().copied().find(|row| row.y_range().contains(y))
}

pub fn clip_rect(row: &Row, scale: &TimeScale, start: Ticks, end: Ticks) -> Rect {
    Rect::from_x_y_ranges(
        scale.x(start)..=scale.x(end),
        (row.top + 2.0)..=(row.top + ROW_HEIGHT - 2.0),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Edge {
        clip: ClipId,
        track: TrackRef,
        edge: Edge,
        at: Ticks,
    },
    Body {
        clip: ClipId,
        track: TrackRef,
    },
    Empty {
        track: Option<TrackRef>,
    },
}

pub fn hit(sequence: &Sequence, rows: &[Row], scale: &TimeScale, pos: Pos2) -> Hit {
    let Some(row) = row_at(rows, pos.y) else {
        return Hit::Empty { track: None };
    };
    let Some(track) = sequence.track(row.track) else {
        return Hit::Empty { track: None };
    };
    let near = |time: Ticks| (scale.x(time) - pos.x).abs() <= EDGE_GRAB;
    let inside = track.clips.iter().find(|clip| {
        let x = scale.x(clip.start)..=scale.x(clip.end());
        x.contains(&pos.x)
    });
    let edge_of = |clip: &timeline::Clip, edge| Hit::Edge {
        clip: clip.id,
        track: row.track,
        edge,
        at: match edge {
            Edge::Head => clip.start,
            Edge::Tail => clip.end(),
        },
    };
    if let Some(clip) = inside {
        // On a short clip both edges are near; take the closer one.
        let to_head = pos.x - scale.x(clip.start);
        let to_tail = scale.x(clip.end()) - pos.x;
        return match (near(clip.start), near(clip.end())) {
            (true, true) if to_head <= to_tail => edge_of(clip, Edge::Head),
            (true, true) | (false, true) => edge_of(clip, Edge::Tail),
            (true, false) => edge_of(clip, Edge::Head),
            (false, false) => Hit::Body {
                clip: clip.id,
                track: row.track,
            },
        };
    }
    track
        .clips
        .iter()
        .find_map(|clip| {
            if near(clip.end()) {
                Some(edge_of(clip, Edge::Tail))
            } else if near(clip.start) {
                Some(edge_of(clip, Edge::Head))
            } else {
                None
            }
        })
        .unwrap_or(Hit::Empty {
            track: Some(row.track),
        })
}
