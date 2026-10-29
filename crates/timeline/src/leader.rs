//! The Universal Counting Leader: a second each for 8 down to 3 with a clock sweep, the
//! 2 for a single frame, then black until the program starts two seconds later.

use serde::{Deserialize, Serialize};

use crate::{FrameRate, Rgb};

pub const LEADER_SECONDS: i64 = 8;
const FIRST_NUMERAL: i64 = 8;
/// The second that the 2 opens, after the sweeps of 8 down to 3.
const TWO_SECOND: i64 = FIRST_NUMERAL - 2;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CountingLeader {
    pub wipe_color: Rgb,
    pub background_color: Rgb,
    pub line_color: Rgb,
    pub target_color: Rgb,
    pub numeral_color: Rgb,
    /// The 2-pop that picture and sound are synced by.
    pub blip_on_two: bool,
    pub blip_every_second: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LeaderPicture {
    /// `numeral` with its sweep `swept` of the way around, 0 to 1.
    Count {
        numeral: u8,
        swept: f32,
    },
    Black,
}

impl Default for CountingLeader {
    fn default() -> Self {
        Self {
            wipe_color: Rgb::new(150, 150, 150),
            background_color: Rgb::new(72, 72, 72),
            line_color: Rgb::BLACK,
            target_color: Rgb::WHITE,
            numeral_color: Rgb::WHITE,
            blip_on_two: true,
            blip_every_second: false,
        }
    }
}

impl CountingLeader {
    pub fn picture_on_frame(frame: i64, rate: FrameRate) -> LeaderPicture {
        let (second, into) = split_second(frame, rate);
        match second {
            0..TWO_SECOND => LeaderPicture::Count {
                numeral: (FIRST_NUMERAL - second) as u8,
                swept: into as f32 / rate.nominal() as f32,
            },
            TWO_SECOND if into == 0 => LeaderPicture::Count {
                numeral: 2,
                swept: 0.0,
            },
            _ => LeaderPicture::Black,
        }
    }

    /// Whether the cue blip sounds through `frame`.
    pub fn blips_on_frame(&self, frame: i64, rate: FrameRate) -> bool {
        match split_second(frame, rate) {
            (TWO_SECOND, 0) => self.blip_on_two,
            (0..TWO_SECOND, 0) => self.blip_every_second,
            _ => false,
        }
    }
}

/// Which timecode second `frame` falls in and how many frames into it.
fn split_second(frame: i64, rate: FrameRate) -> (i64, i64) {
    let per_second = rate.nominal();
    (frame.div_euclid(per_second), frame.rem_euclid(per_second))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_down_by_the_second_and_shows_the_two_for_one_frame() {
        let rate = FrameRate::FPS_25;
        let at = |frame| CountingLeader::picture_on_frame(frame, rate);
        let count = |numeral, swept| LeaderPicture::Count { numeral, swept };
        assert_eq!(at(0), count(8, 0.0));
        assert_eq!(at(5 * 25 + 10), count(3, 0.4));
        assert_eq!(at(6 * 25), count(2, 0.0));
        assert_eq!(at(6 * 25 + 1), LeaderPicture::Black);
    }

    #[test]
    fn blips_on_the_first_frame_of_the_chosen_seconds() {
        let rate = FrameRate::FPS_25;
        let leader = CountingLeader {
            blip_every_second: true,
            ..CountingLeader::default()
        };
        let blipped: Vec<i64> = (0..LEADER_SECONDS * 25)
            .filter(|&frame| leader.blips_on_frame(frame, rate))
            .collect();
        assert_eq!(blipped, [0, 25, 50, 75, 100, 125, 150]);
        let two_only = CountingLeader::default();
        assert!(!two_only.blips_on_frame(0, rate));
        assert!(two_only.blips_on_frame(150, rate));
    }
}
