use serde::{Deserialize, Serialize};

use crate::{FrameRate, Sequence, Ticks, Timecode};

/// How a sequence shows positions and lengths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeDisplay {
    /// Drop-frame at 29.97 and 59.94 fps, so it keeps with the clock; plain elsewhere.
    #[default]
    Timecode,
    NonDropTimecode,
    Frames,
}

impl TimeDisplay {
    /// The choices that differ from each other at `rate`.
    pub fn choices(rate: FrameRate) -> impl Iterator<Item = Self> {
        let drops = rate.dropped_per_minute().is_some();
        [Self::Timecode, Self::NonDropTimecode, Self::Frames]
            .into_iter()
            .filter(move |display| drops || *display != Self::NonDropTimecode)
    }

    pub fn name(self, rate: FrameRate) -> &'static str {
        match self {
            Self::Timecode if rate.dropped_per_minute().is_some() => "Drop-Frame Timecode",
            Self::Timecode => "Timecode",
            Self::NonDropTimecode => "Non-Drop-Frame Timecode",
            Self::Frames => "Frames",
        }
    }

    /// Frame numbers skipped each minute at `rate`, when shown drop-frame.
    pub(crate) fn dropped(self, rate: FrameRate) -> Option<u64> {
        rate.dropped_per_minute().filter(|_| self == Self::Timecode)
    }
}

impl Sequence {
    /// A length as the sequence shows times; see `timecode_at` for a position.
    pub fn timecode(&self, length: Ticks) -> Timecode {
        let rate = self.format.frame_rate;
        Timecode::shown(length, self.timebase, rate, self.time_display)
    }

    /// A position as the sequence shows times, counted from its start time.
    pub fn timecode_at(&self, position: Ticks) -> Timecode {
        self.timecode(position + self.start_time)
    }
}

impl FrameRate {
    /// The frame numbers drop-frame timecode skips at the start of each minute but every
    /// tenth, for the rates that run a thousandth slow of a whole count.
    pub fn dropped_per_minute(self) -> Option<u64> {
        match (self.num(), self.den()) {
            (30000, 1001) => Some(2),
            (60000, 1001) => Some(4),
            _ => None,
        }
    }
}

/// The drop-frame label, counted in nominal frames, that frame `frame` is shown as.
pub(crate) fn drop_frame_label(frame: u64, dropped: u64, nominal: u64) -> u64 {
    let per_minute = nominal * 60 - dropped;
    let per_ten_minutes = per_minute * 10 + dropped;
    let (tens, within) = (frame / per_ten_minutes, frame % per_ten_minutes);
    let minutes_dropped = within.saturating_sub(dropped) / per_minute;
    frame + dropped * (9 * tens + minutes_dropped)
}

/// The frame a drop-frame label stands for; a label that's skipped means the frame
/// after it.
pub(crate) fn drop_frame_of_label(label: u64, dropped: u64, nominal: u64) -> u64 {
    let per_minute = nominal * 60;
    let minutes = label / per_minute;
    let into_minute = label % per_minute;
    let label = if !minutes.is_multiple_of(10) && into_minute < dropped {
        label - into_minute + dropped
    } else {
        label
    };
    label - dropped * (minutes - minutes / 10)
}
