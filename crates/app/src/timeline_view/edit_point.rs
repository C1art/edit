use egui::{Color32, Painter, Stroke};
use timeline::{Sequence, Ticks, TrackRef};

use super::layout::Row;
use super::scale::TimeScale;

/// Which side of a selected edit point keyboard trims move, as Premiere's trim types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrimType {
    /// Both sides together.
    #[default]
    Roll,
    /// The outgoing clip's out point, rippling what follows.
    RippleOut,
    /// The incoming clip's in point, rippling what follows.
    RippleIn,
}

impl TrimType {
    pub fn next(self) -> Self {
        match self {
            Self::Roll => Self::RippleOut,
            Self::RippleOut => Self::RippleIn,
            Self::RippleIn => Self::Roll,
        }
    }
}

/// A cut selected for trimming from the keyboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditPoint {
    pub at: Ticks,
    pub tracks: Vec<TrackRef>,
    pub trim: TrimType,
}

impl EditPoint {
    /// Whether a clip still starts or ends at the point on one of its tracks, which an
    /// undo or another edit may have changed.
    pub fn is_on(&self, sequence: &Sequence) -> bool {
        self.tracks.iter().any(|&track| {
            sequence.track(track).is_some_and(|track| {
                track
                    .clips
                    .iter()
                    .any(|clip| clip.start == self.at || clip.end() == self.at)
            })
        })
    }
}

/// Premiere's trim handles: red for a roll across the cut, yellow on the side a ripple
/// trims.
pub fn paint(painter: &Painter, row: &Row, scale: &TimeScale, point: &EditPoint) {
    if !point.tracks.contains(&row.track) {
        return;
    }
    let x = scale.x(point.at);
    let (top, bottom) = (row.top + 2.0, row.bottom() - 2.0);
    let (color, sides): (Color32, &[f32]) = match point.trim {
        TrimType::Roll => (Color32::from_rgb(230, 60, 60), &[-1.0, 1.0]),
        TrimType::RippleOut => (Color32::from_rgb(240, 200, 40), &[-1.0]),
        TrimType::RippleIn => (Color32::from_rgb(240, 200, 40), &[1.0]),
    };
    let stroke = Stroke::new(2.5, color);
    painter.line_segment([egui::pos2(x, top), egui::pos2(x, bottom)], stroke);
    for &side in sides {
        let arm = x + side * 6.0;
        painter.line_segment([egui::pos2(x, top), egui::pos2(arm, top)], stroke);
        painter.line_segment([egui::pos2(x, bottom), egui::pos2(arm, bottom)], stroke);
    }
}
