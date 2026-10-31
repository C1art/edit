//! Auto Reframe: a picture enlarged to fill a frame of another shape, following its
//! subject.

use serde::{Deserialize, Serialize};

use crate::stabilize::{frames_per_second, smooth};
use crate::{Param, Ticks, Timebase};

/// How closely the frame follows the subject, as Premiere's motion presets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReframeMotion {
    /// For footage with little movement: the frame barely moves.
    Slower,
    #[default]
    Default,
    /// For fast action: the frame keeps up with quick moves.
    Faster,
}

impl ReframeMotion {
    pub const ALL: [Self; 3] = [Self::Slower, Self::Default, Self::Faster];

    pub fn name(self) -> &'static str {
        match self {
            Self::Slower => "Slower Motion",
            Self::Default => "Default",
            Self::Faster => "Faster Motion",
        }
    }

    /// How many seconds each way the subject's path is averaged over.
    fn smoothing_seconds(self) -> f64 {
        match self {
            Self::Slower => 1.5,
            Self::Default => 0.75,
            Self::Faster => 0.25,
        }
    }
}

/// Where analysis found a clip's subject: each analyzed frame's source time, in order,
/// and the subject's center then, in millionths of the picture from its top left.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubjectPath {
    pub frames: Vec<Ticks>,
    pub centers: Vec<[i32; 2]>,
}

/// The Auto Reframe effect's settings and the framing they make of the analyzed path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AutoReframe {
    /// Sequence pixels the reframed picture moves by.
    pub offset: [Param; 2],
    /// Percent of the size that just fills the frame.
    pub scale: Param,
    pub motion: ReframeMotion,
    #[serde(default)]
    pub subject: Option<SubjectPath>,
    /// The subject's path smoothed for `motion`, in millionths of the picture. Kept up
    /// to date with the settings by `retune`.
    #[serde(default)]
    pub framing: Vec<[i32; 2]>,
}

impl Default for AutoReframe {
    fn default() -> Self {
        Self {
            offset: [Param::default(), Param::default()],
            scale: Param::from_value(100.0),
            motion: ReframeMotion::default(),
            subject: None,
            framing: Vec::new(),
        }
    }
}

impl AutoReframe {
    /// Takes the analysis of a clip: each frame's source time and its subject's center
    /// as fractions of the picture.
    pub fn analyzed(&mut self, centers: &[(Ticks, [f32; 2])], timebase: Timebase) {
        let millionths = |fraction: f32| (f64::from(fraction.clamp(0.0, 1.0)) * 1e6) as i32;
        self.subject = Some(SubjectPath {
            frames: centers.iter().map(|&(time, _)| time).collect(),
            centers: centers
                .iter()
                .map(|&(_, center)| center.map(millionths))
                .collect(),
        });
        self.retune(timebase);
    }

    /// Recomputes the framing after the settings change.
    pub fn retune(&mut self, timebase: Timebase) {
        let Some(path) = &self.subject else {
            self.framing.clear();
            return;
        };
        let centers: Vec<[f64; 2]> = path
            .centers
            .iter()
            .map(|center| center.map(f64::from))
            .collect();
        let sigma = frames_per_second(&path.frames, timebase) * self.motion.smoothing_seconds();
        self.framing = smooth(&centers, sigma)
            .into_iter()
            .map(|center| center.map(|value| value.round() as i32))
            .collect();
    }

    /// Where the frame centers on at source time `time`, as fractions of the picture:
    /// the picture's middle until the clip is analyzed.
    pub fn subject_at(&self, time: Ticks) -> [f32; 2] {
        let Some(path) = &self.subject else {
            return [0.5; 2];
        };
        let fraction = |center: [i32; 2]| center.map(|value| value as f32 / 1e6);
        let point = |index: usize| Some((*path.frames.get(index)?, *self.framing.get(index)?));
        let after = path.frames.partition_point(|&frame| frame <= time);
        match (after.checked_sub(1).and_then(point), point(after)) {
            (Some((start, from)), Some((end, to))) => {
                let along = (time - start).0 as f32 / (end - start).0.max(1) as f32;
                let [from, to] = [from, to].map(fraction);
                [0, 1].map(|axis| from[axis] + (to[axis] - from[axis]) * along)
            }
            (Some((_, center)), None) | (None, Some((_, center))) => fraction(center),
            (None, None) => [0.5; 2],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ticks of the 90 kHz timebase in a frame at 30 fps.
    const FRAME: i64 = 3_000;

    /// A subject that jumps from the left third to the right third halfway through two
    /// seconds of footage.
    fn jumping(motion: ReframeMotion) -> AutoReframe {
        let centers: Vec<(Ticks, [f32; 2])> = (0..60)
            .map(|frame| {
                let x = if frame < 30 { 0.3 } else { 0.7 };
                (Ticks(frame * FRAME), [x, 0.5])
            })
            .collect();
        let mut reframe = AutoReframe {
            motion,
            ..AutoReframe::default()
        };
        reframe.analyzed(&centers, Timebase::HZ_90K);
        reframe
    }

    #[test]
    fn slower_motion_eases_into_a_jump_that_faster_motion_follows() {
        let x = |motion, frame| jumping(motion).subject_at(Ticks(frame * FRAME))[0];
        // Fifteen frames after the jump.
        let (slower, faster) = (x(ReframeMotion::Slower, 45), x(ReframeMotion::Faster, 45));
        assert!(faster > 0.69, "{faster}");
        assert!(slower > 0.5 && slower < 0.65, "{slower}");
        assert!((x(ReframeMotion::Faster, 0) - 0.3).abs() < 0.01);
    }

    #[test]
    fn the_subject_moves_smoothly_between_analyzed_frames() {
        let mut reframe = AutoReframe {
            motion: ReframeMotion::Faster,
            ..AutoReframe::default()
        };
        // A second apart, too far for the smoothing to reach.
        let centers = [(Ticks(0), [0.2, 0.4]), (Ticks(90_000), [0.4, 0.6])];
        reframe.analyzed(&centers, Timebase::HZ_90K);
        let middle = reframe.subject_at(Ticks(45_000));
        assert!((middle[0] - 0.3).abs() < 1e-3 && (middle[1] - 0.5).abs() < 1e-3);
        assert_eq!(reframe.subject_at(Ticks(200_000)), [0.4, 0.6]);
        assert_eq!(AutoReframe::default().subject_at(Ticks(5)), [0.5, 0.5]);
    }
}
