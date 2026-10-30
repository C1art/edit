//! Ready-made speed ramps, laid down as time remapping keyframes that keep a clip's
//! length and the part of its source it plays, so nothing around it moves.

use std::ops::Range;

use crate::{Anchor, FrameRate, Ticks, TimeRemap, Timebase};

/// How many times faster the quick part of a ramp plays than the slow part.
const CONTRAST: f64 = 4.0;
/// Segments a ramp is made of, enough that its speed seems to glide rather than step.
const STEPS: i64 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpeedRamp {
    RampUp,
    RampDown,
    FastMiddle,
    SlowMiddle,
}

impl SpeedRamp {
    pub const ALL: [Self; 4] = [
        Self::RampUp,
        Self::RampDown,
        Self::FastMiddle,
        Self::SlowMiddle,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::RampUp => "Ramp Up",
            Self::RampDown => "Ramp Down",
            Self::FastMiddle => "Fast Middle",
            Self::SlowMiddle => "Slow Middle",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::RampUp => "Starts slow and speeds up through the middle of the clip",
            Self::RampDown => "Starts fast and slows down through the middle of the clip",
            Self::FastMiddle => "Rushes through the middle, slow either side",
            Self::SlowMiddle => "Slow motion in the middle, fast either side",
        }
    }

    /// How fast the clip plays `along` (0 to 1) its length, relative to its slowest.
    fn pace(self, along: f64) -> f64 {
        let rising = |from: f64, to: f64| smoothstep((along - from) / (to - from));
        let middle = rising(0.2, 0.4).min(1.0 - rising(0.6, 0.8));
        let quick = match self {
            Self::RampUp => rising(0.25, 0.75),
            Self::RampDown => 1.0 - rising(0.25, 0.75),
            Self::FastMiddle => middle,
            Self::SlowMiddle => 1.0 - middle,
        };
        1.0 + (CONTRAST - 1.0) * quick
    }

    /// The remapping playing `source` over `duration`, keyed on whole frames; `None` for
    /// a clip too short to ramp or a ramp that would leave the speeds allowed.
    pub(crate) fn remap(
        self,
        source: Range<Ticks>,
        duration: Ticks,
        (rate, timebase): (FrameRate, Timebase),
    ) -> Option<TimeRemap> {
        let frames = rate.frame_at(duration, timebase);
        let steps = STEPS.min(frames);
        if steps < 2 || source.end <= source.start {
            return None;
        }
        let mut offsets: Vec<Ticks> = (0..steps)
            .map(|step| rate.frame_start(step * frames / steps, timebase))
            .collect();
        offsets.push(duration);
        let length = duration.0 as f64;
        let mut covered = vec![0.0];
        for pair in offsets.windows(2) {
            let middle = (pair[0].0 + pair[1].0) as f64 / 2.0 / length;
            let ticks = (pair[1] - pair[0]).0 as f64;
            let total = covered.last().copied().unwrap_or_default();
            covered.push(total + self.pace(middle) * ticks);
        }
        let total = covered.last().copied().unwrap_or(1.0);
        let span = (source.end - source.start).0 as f64;
        let anchors = offsets
            .into_iter()
            .zip(covered)
            .map(|(offset, covered)| Anchor {
                offset,
                source: source.start + Ticks((span * covered / total).round() as i64),
            })
            .collect();
        TimeRemap::from_anchors(anchors)
    }
}

fn smoothstep(along: f64) -> f64 {
    let along = along.clamp(0.0, 1.0);
    along * along * (3.0 - 2.0 * along)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ramp_keeps_the_clip_and_its_source_and_speeds_through_the_middle() {
        let frames = Timebase::new(1, 25).expect("valid timebase");
        let rate = FrameRate::FPS_25;
        let remap = SpeedRamp::FastMiddle
            .remap(Ticks(0)..Ticks(200), Ticks(100), (rate, frames))
            .expect("ramps");
        let anchors = remap.anchors();
        let ends = (anchors[0], anchors[anchors.len() - 1]);
        assert_eq!(ends.0.offset, Ticks(0));
        assert_eq!(ends.0.source, Ticks(0));
        assert_eq!(ends.1.offset, Ticks(100));
        assert_eq!(ends.1.source, Ticks(200));
        let speed = |index| remap.speed(index).expect("segment").percent();
        let middle = speed(anchors.len() / 2);
        assert!(middle > speed(0) && middle > speed(anchors.len() - 2));
    }
}
