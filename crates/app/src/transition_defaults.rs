//! The transition the default-transition commands put down, and how long it lasts.

use std::ops::RangeInclusive;

use timeline::{Sequence, TrackKind, Transition, TransitionKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TransitionDefaults {
    /// Audio always crossfades, as no other audio transition suits every cut.
    pub video: TransitionKind,
    /// Hundredths of a second.
    pub length: u32,
}

impl TransitionDefaults {
    pub const STANDARD: Self = Self {
        video: TransitionKind::CrossDissolve,
        length: 100,
    };
    pub const LENGTHS: RangeInclusive<u32> = 10..=1000;

    /// The transition for a cut on a `track` of `sequence`, a whole number of its frames
    /// long.
    pub fn transition(self, sequence: &Sequence, track: TrackKind) -> Transition {
        let kind = match track {
            TrackKind::Video => self.video,
            TrackKind::Audio => TransitionKind::CrossDissolve,
        };
        let rate = sequence.format.frame_rate;
        let duration = rate.frame_start(self.frames(rate.num(), rate.den()), sequence.timebase);
        Transition::new(kind, duration)
    }

    fn frames(self, num: u32, den: u32) -> i64 {
        whole_frames(self.length, num, den)
    }

    /// The video kind's name, for the preferences file.
    pub fn key(self) -> &'static str {
        self.video.name()
    }

    pub fn video_from_key(key: &str) -> Option<TransitionKind> {
        TransitionKind::VIDEO
            .into_iter()
            .find(|kind| kind.name() == key)
    }
}

/// The nearest whole number of frames at `num`/`den` per second to `hundredths` of a
/// second, at least one.
pub(crate) fn whole_frames(hundredths: u32, num: u32, den: u32) -> i64 {
    let per_hundred_seconds = i64::from(den) * 100;
    let frames =
        (i64::from(hundredths) * i64::from(num) + per_hundred_seconds / 2) / per_hundred_seconds;
    frames.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_round_to_the_nearest_frame_and_never_vanish() {
        let length = |length| TransitionDefaults {
            length,
            ..TransitionDefaults::STANDARD
        };
        assert_eq!(length(100).frames(30_000, 1001), 30);
        assert_eq!(length(50).frames(24, 1), 12);
        assert_eq!(length(33).frames(25, 1), 8);
        assert_eq!(length(1).frames(24, 1), 1);
    }
}
