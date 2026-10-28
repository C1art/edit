use egui::{Key, Modifiers};

/// Which way along the timeline a track select tool picks clips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Selection,
    TrackSelect(Direction),
    Ripple,
    Roll,
    RateStretch,
    Razor,
    Slip,
    Slide,
}

impl Tool {
    pub const ALL: [Self; 9] = [
        Self::Selection,
        Self::TrackSelect(Direction::Forward),
        Self::TrackSelect(Direction::Backward),
        Self::Ripple,
        Self::Roll,
        Self::RateStretch,
        Self::Razor,
        Self::Slip,
        Self::Slide,
    ];

    pub fn shortcut(self) -> (Modifiers, Key) {
        let key = match self {
            Self::Selection => Key::V,
            Self::TrackSelect(_) => Key::A,
            Self::Ripple => Key::B,
            Self::Roll => Key::N,
            Self::RateStretch => Key::R,
            Self::Razor => Key::C,
            Self::Slip => Key::Y,
            Self::Slide => Key::U,
        };
        match self {
            Self::TrackSelect(Direction::Backward) => (Modifiers::SHIFT, key),
            _ => (Modifiers::NONE, key),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Selection => "Select",
            Self::TrackSelect(Direction::Forward) => "Track Fwd",
            Self::TrackSelect(Direction::Backward) => "Track Back",
            Self::Ripple => "Ripple",
            Self::Roll => "Roll",
            Self::RateStretch => "Stretch",
            Self::Razor => "Razor",
            Self::Slip => "Slip",
            Self::Slide => "Slide",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Selection => "Move clips; drag an edge to trim (V)",
            Self::TrackSelect(Direction::Forward) => {
                "Select every clip from here on, or with Shift on one track (A)"
            }
            Self::TrackSelect(Direction::Backward) => {
                "Select every clip up to here, or with Shift on one track (Shift+A)"
            }
            Self::Ripple => "Drag an edge to trim and shift what follows (B)",
            Self::Roll => "Drag a cut to move it between two clips (N)",
            Self::RateStretch => "Drag an edge to change a clip's speed to fit (R)",
            Self::Razor => "Click a clip to cut it (C)",
            Self::Slip => "Drag a clip to change its source range (Y)",
            Self::Slide => "Drag a clip between its neighbours (U)",
        }
    }
}
