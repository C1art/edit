use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Ticks, Timebase};

/// Frames per second as the exact fraction `num / den` (e.g. `30000/1001` for 29.97 fps).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FrameRate {
    num: u32,
    den: u32,
}

impl FrameRate {
    pub const fn new(num: u32, den: u32) -> Option<Self> {
        if num == 0 || den == 0 {
            None
        } else {
            Some(Self { num, den })
        }
    }

    /// The duration of one frame, which is exactly one tick of this timebase.
    pub const fn frame_timebase(self) -> Timebase {
        match Timebase::new(self.den, self.num) {
            Some(timebase) => timebase,
            None => unreachable!(),
        }
    }

    /// Frames counted per timecode second: the rate rounded up, so 29.97 counts to 30.
    fn nominal(self) -> i64 {
        i64::from(self.num.div_ceil(self.den))
    }

    /// The frame that is showing at `position`, i.e. the last frame starting at or before it.
    pub fn frame_at(self, position: Ticks, timebase: Timebase) -> i64 {
        let numerator = i128::from(position.0) * i128::from(timebase.num()) * i128::from(self.num);
        let denominator = i128::from(timebase.den()) * i128::from(self.den);
        let frame = numerator.div_euclid(denominator);
        i64::try_from(frame).unwrap_or(if frame < 0 { i64::MIN } else { i64::MAX })
    }

    pub fn frame_start(self, frame: i64, timebase: Timebase) -> Ticks {
        let exact = Ticks(frame).rescale(self.frame_timebase(), timebase);
        // Rounding to nearest can land one tick past the frame's true start, which
        // `frame_at` would then attribute to the previous frame.
        if self.frame_at(exact, timebase) < frame {
            exact + Ticks(1)
        } else {
            exact
        }
    }
}

/// Non-drop-frame `HH:MM:SS:FF` timecode, as shown in the transport and timeline ruler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timecode {
    negative: bool,
    hours: i64,
    minutes: i64,
    seconds: i64,
    frames: i64,
}

impl Timecode {
    pub fn new(position: Ticks, timebase: Timebase, rate: FrameRate) -> Self {
        let frame = rate.frame_at(position, timebase);
        let nominal = rate.nominal();
        let total = frame.unsigned_abs();
        let per_second = nominal.unsigned_abs();
        let seconds_total = total / per_second;
        let to_i64 = |value: u64| i64::try_from(value).unwrap_or(i64::MAX);
        Self {
            negative: frame < 0,
            hours: to_i64(seconds_total / 3600),
            minutes: to_i64(seconds_total / 60 % 60),
            seconds: to_i64(seconds_total % 60),
            frames: to_i64(total % per_second),
        }
    }
}

impl fmt::Display for Timecode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.negative { "-" } else { "" };
        write!(
            f,
            "{sign}{:02}:{:02}:{:02}:{:02}",
            self.hours, self.minutes, self.seconds, self.frames
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NTSC: FrameRate = match FrameRate::new(30000, 1001) {
        Some(rate) => rate,
        None => unreachable!(),
    };

    #[test]
    fn frame_at_floors_within_a_frame() {
        let tb = Timebase::HZ_90K;
        assert_eq!(NTSC.frame_at(Ticks(3002), tb), 0);
        assert_eq!(NTSC.frame_at(Ticks(3003), tb), 1);
        assert_eq!(NTSC.frame_at(Ticks(-1), tb), -1);
    }

    #[test]
    fn frame_start_round_trips_through_frame_at_for_inexact_timebase() {
        let millis = Timebase::new(1, 1000).unwrap();
        for frame in 0..1000 {
            let start = NTSC.frame_start(frame, millis);
            assert_eq!(NTSC.frame_at(start, millis), frame, "frame {frame}");
        }
    }

    #[test]
    fn ntsc_timecode_counts_thirty_nominal_frames_per_second() {
        let tb = Timebase::HZ_90K;
        let one_hour_of_frames = 30 * 3600;
        let position = NTSC.frame_start(one_hour_of_frames + 29, tb);
        assert_eq!(Timecode::new(position, tb, NTSC).to_string(), "01:00:00:29");
    }

    #[test]
    fn negative_positions_format_with_sign() {
        let pal = FrameRate::new(25, 1).unwrap();
        let tb = Timebase::HZ_90K;
        assert_eq!(
            Timecode::new(Ticks(-90_000), tb, pal).to_string(),
            "-00:00:01:00"
        );
    }
}
