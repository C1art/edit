use std::fmt;

use serde::{Deserialize, Serialize};

use crate::time_display::{drop_frame_label, drop_frame_of_label};
use crate::{Ticks, TimeDisplay, Timebase};

/// Frames per second as the exact fraction `num / den` (e.g. `30000/1001` for 29.97 fps).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FrameRate {
    num: u32,
    den: u32,
}

impl FrameRate {
    pub const FPS_25: Self = Self { num: 25, den: 1 };

    pub const fn new(num: u32, den: u32) -> Option<Self> {
        if num == 0 || den == 0 {
            None
        } else {
            Some(Self { num, den })
        }
    }

    pub const fn num(self) -> u32 {
        self.num
    }

    pub const fn den(self) -> u32 {
        self.den
    }

    /// For display only; time math stays rational.
    pub fn approx_fps(self) -> f64 {
        f64::from(self.num) / f64::from(self.den)
    }

    /// The duration of one frame, which is exactly one tick of this timebase.
    pub const fn frame_timebase(self) -> Timebase {
        match Timebase::new(self.den, self.num) {
            Some(timebase) => timebase,
            None => unreachable!(),
        }
    }

    /// Frames counted per timecode second: the rate rounded up, so 29.97 counts to 30.
    pub(crate) fn nominal(self) -> i64 {
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

/// A position or length as a sequence shows it: `HH:MM:SS:FF` timecode, `HH;MM;SS;FF`
/// drop-frame timecode, or a count of frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timecode {
    negative: bool,
    shown: Shown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    Clock {
        hours: u64,
        minutes: u64,
        seconds: u64,
        frames: u64,
        drop_frame: bool,
    },
    Frames(u64),
}

impl Timecode {
    /// Non-drop-frame timecode, as interchange formats count.
    pub fn new(position: Ticks, timebase: Timebase, rate: FrameRate) -> Self {
        Self::shown(position, timebase, rate, TimeDisplay::NonDropTimecode)
    }

    pub fn shown(
        position: Ticks,
        timebase: Timebase,
        rate: FrameRate,
        display: TimeDisplay,
    ) -> Self {
        let frame = rate.frame_at(position, timebase);
        let negative = frame < 0;
        let total = frame.unsigned_abs();
        if display == TimeDisplay::Frames {
            return Self {
                negative,
                shown: Shown::Frames(total),
            };
        }
        let per_second = rate.nominal().unsigned_abs();
        let dropped = display.dropped(rate);
        let label = dropped.map_or(total, |dropped| {
            drop_frame_label(total, dropped, per_second)
        });
        let seconds_total = label / per_second;
        Self {
            negative,
            shown: Shown::Clock {
                hours: seconds_total / 3600,
                minutes: seconds_total / 60 % 60,
                seconds: seconds_total % 60,
                frames: label % per_second,
                drop_frame: dropped.is_some(),
            },
        }
    }
}

/// A timecode as typed: a position, or with a leading `+` or `-`, a move from the current
/// one. Both count frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedTimecode {
    At(i64),
    By(i64),
}

impl TypedTimecode {
    /// The frame a written timecode such as a file's tag names; drop-frame when a `;`
    /// splits its fields, as SMPTE writes it.
    pub fn frame_of_label(label: &str, rate: FrameRate) -> Option<i64> {
        let display = if label.contains(';') {
            TimeDisplay::Timecode
        } else {
            TimeDisplay::NonDropTimecode
        };
        match Self::parse(label, rate, display)? {
            Self::At(frame) => Some(frame),
            Self::By(_) => None,
        }
    }

    /// Reads timecode as Premiere's fields take it: `HH:MM:SS:FF` with the leading
    /// fields optional, or bare digits read in pairs from the right, so `500` at any
    /// rate is five seconds. Shown as frames, bare digits count frames.
    pub fn parse(text: &str, rate: FrameRate, display: TimeDisplay) -> Option<Self> {
        let text = text.trim();
        let (sign, body) = match text.strip_prefix('+') {
            Some(body) => (Some(1), body),
            None => match text.strip_prefix('-') {
                Some(body) => (Some(-1), body),
                None => (None, text),
            },
        };
        let body = body.trim();
        let all_digits = |field: &str| field.bytes().all(|byte| byte.is_ascii_digit());
        let separated = body.contains([':', ';', '.']);
        if display == TimeDisplay::Frames && !separated {
            let frames: i64 = body.parse().ok().filter(|_| all_digits(body))?;
            return Some(match sign {
                Some(sign) => Self::By(sign * frames),
                None => Self::At(frames),
            });
        }
        let fields: Vec<&str> = if separated {
            body.split([':', ';', '.']).collect()
        } else if !body.is_empty() {
            let pairs = body.as_bytes().rchunks(2).rev();
            pairs
                .map(std::str::from_utf8)
                .collect::<Result<_, _>>()
                .ok()?
        } else {
            return None;
        };
        if fields.len() > 4 || !fields.iter().all(|field| all_digits(field)) {
            return None;
        }
        let nominal = rate.nominal();
        let units = [1, nominal, nominal * 60, nominal * 3600];
        let frames = fields
            .iter()
            .rev()
            .zip(units)
            .try_fold(0_i64, |sum, (field, unit)| {
                let value: i64 = if field.is_empty() {
                    0
                } else {
                    field.parse().ok()?
                };
                sum.checked_add(value.checked_mul(unit)?)
            })?;
        Some(match (sign, display.dropped(rate)) {
            (Some(sign), _) => Self::By(sign * frames),
            (None, None) => Self::At(frames),
            (None, Some(dropped)) => {
                let label = frames.unsigned_abs();
                let frame = drop_frame_of_label(label, dropped, nominal.unsigned_abs());
                Self::At(i64::try_from(frame).ok()?)
            }
        })
    }
}

impl fmt::Display for Timecode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.negative { "-" } else { "" };
        match self.shown {
            Shown::Clock {
                hours,
                minutes,
                seconds,
                frames,
                drop_frame,
            } => {
                let colon = if drop_frame { ';' } else { ':' };
                write!(
                    f,
                    "{sign}{hours:02}{colon}{minutes:02}{colon}{seconds:02}{colon}{frames:02}"
                )
            }
            Shown::Frames(frames) => write!(f, "{sign}{frames}"),
        }
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
    fn file_timecode_labels_read_drop_frame_only_with_semicolons() {
        let pal = FrameRate::new(25, 1).unwrap();
        assert_eq!(
            TypedTimecode::frame_of_label("01:00:00:00", pal),
            Some(90_000)
        );
        assert_eq!(
            TypedTimecode::frame_of_label("01:00:00;00", NTSC),
            Some(107_892)
        );
        assert_eq!(
            TypedTimecode::frame_of_label("01:00:00:00", NTSC),
            Some(108_000)
        );
        assert_eq!(TypedTimecode::frame_of_label("-00:00:01:00", pal), None);
    }

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
    fn typed_timecode_fills_fields_from_the_right() {
        let pal = FrameRate::FPS_25;
        assert_eq!(
            TypedTimecode::parse("00:00:05:00", pal, TimeDisplay::NonDropTimecode),
            Some(TypedTimecode::At(125))
        );
        assert_eq!(
            TypedTimecode::parse("1:00", pal, TimeDisplay::NonDropTimecode),
            Some(TypedTimecode::At(25))
        );
        assert_eq!(
            TypedTimecode::parse("500", pal, TimeDisplay::NonDropTimecode),
            Some(TypedTimecode::At(125))
        );
        assert_eq!(
            TypedTimecode::parse("1000000", NTSC, TimeDisplay::NonDropTimecode),
            Some(TypedTimecode::At(30 * 3600))
        );
    }

    #[test]
    fn typed_timecode_with_a_sign_is_a_move() {
        let pal = FrameRate::FPS_25;
        assert_eq!(
            TypedTimecode::parse("+10", pal, TimeDisplay::NonDropTimecode),
            Some(TypedTimecode::By(10))
        );
        assert_eq!(
            TypedTimecode::parse("-1.00", pal, TimeDisplay::NonDropTimecode),
            Some(TypedTimecode::By(-25))
        );
    }

    #[test]
    fn typed_timecode_rejects_what_is_not_timecode() {
        let pal = FrameRate::FPS_25;
        for text in ["", "abc", "1:2:3:4:5", "1:-2", "+", "99999999999999999999"] {
            assert_eq!(
                TypedTimecode::parse(text, pal, TimeDisplay::NonDropTimecode),
                None,
                "{text:?}"
            );
        }
    }

    #[test]
    fn drop_frame_timecode_skips_two_labels_a_minute_but_every_tenth() {
        let tb = Timebase::HZ_90K;
        let shown = |frame| {
            let position = NTSC.frame_start(frame, tb);
            Timecode::shown(position, tb, NTSC, TimeDisplay::Timecode).to_string()
        };
        assert_eq!(shown(1799), "00;00;59;29");
        assert_eq!(shown(1800), "00;01;00;02");
        assert_eq!(shown(17_981), "00;09;59;29");
        assert_eq!(shown(17_982), "00;10;00;00");
        // An hour of 29.97 fps is 107,892 frames, which drop-frame labels an hour.
        assert_eq!(shown(107_892), "01;00;00;00");
    }

    #[test]
    fn typed_drop_frame_timecode_finds_the_frame_it_labels() {
        let parse = |text: &str| TypedTimecode::parse(text, NTSC, TimeDisplay::Timecode);
        assert_eq!(parse("00;01;00;02"), Some(TypedTimecode::At(1800)));
        assert_eq!(parse("01:00:00:00"), Some(TypedTimecode::At(107_892)));
        // Labels drop-frame skips mean the first frame shown after them.
        assert_eq!(parse("00;01;00;00"), Some(TypedTimecode::At(1800)));
        for frame in (0..40_000).step_by(7) {
            let tb = Timebase::HZ_90K;
            let position = NTSC.frame_start(frame, tb);
            let shown = Timecode::shown(position, tb, NTSC, TimeDisplay::Timecode).to_string();
            assert_eq!(parse(&shown), Some(TypedTimecode::At(frame)), "{shown}");
        }
    }

    #[test]
    fn shown_as_frames_counts_and_reads_frames() {
        let tb = Timebase::HZ_90K;
        let position = NTSC.frame_start(1234, tb);
        let shown = Timecode::shown(position, tb, NTSC, TimeDisplay::Frames);
        assert_eq!(shown.to_string(), "1234");
        let parse = |text| TypedTimecode::parse(text, NTSC, TimeDisplay::Frames);
        assert_eq!(parse("1234"), Some(TypedTimecode::At(1234)));
        assert_eq!(parse("+100"), Some(TypedTimecode::By(100)));
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
