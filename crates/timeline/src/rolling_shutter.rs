use serde::{Deserialize, Serialize};

use crate::{CameraPath, Similarity, Ticks};

/// The order a camera's sensor reads its lines in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScanDirection {
    #[default]
    TopToBottom,
    BottomToTop,
    LeftToRight,
    RightToLeft,
}

impl ScanDirection {
    pub const ALL: [Self; 4] = [
        Self::TopToBottom,
        Self::BottomToTop,
        Self::LeftToRight,
        Self::RightToLeft,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::TopToBottom => "Top → Bottom",
            Self::BottomToTop => "Bottom → Top",
            Self::LeftToRight => "Left → Right",
            Self::RightToLeft => "Right → Left",
        }
    }

    /// The way the read moves across the picture, right and down.
    pub fn axis(self) -> [f64; 2] {
        match self {
            Self::TopToBottom => [0.0, 1.0],
            Self::BottomToTop => [0.0, -1.0],
            Self::LeftToRight => [1.0, 0.0],
            Self::RightToLeft => [-1.0, 0.0],
        }
    }
}

/// Undoes the skew and wobble of a sensor that reads its lines one after another while
/// the camera moves, from an analysis of the clip's motion.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RollingShutterRepair {
    /// Percent of a frame's time the sensor takes to read all its lines.
    pub rate: u32,
    pub scan: ScanDirection,
    #[serde(default)]
    pub path: Option<CameraPath>,
}

impl Default for RollingShutterRepair {
    fn default() -> Self {
        Self {
            rate: 50,
            scan: ScanDirection::default(),
            path: None,
        }
    }
}

impl RollingShutterRepair {
    /// How far the picture moved, right and down in shares of its longer side, between
    /// the sensor reading its first line and its last at source time `time`, if the clip
    /// has been analyzed.
    pub fn travel_at(&self, time: Ticks) -> Option<[f64; 2]> {
        let path = self.path.as_ref()?;
        let shift = |index: usize| Similarity::from(path.poses[index]).shift;
        let last = path.poses.len().checked_sub(1).filter(|&last| last > 0)?;
        let index = path
            .frames
            .partition_point(|&frame| frame <= time)
            .saturating_sub(1)
            .min(last);
        let (before, after) = (index.saturating_sub(1), (index + 1).min(last));
        let ([x0, y0], [x1, y1]) = (shift(before), shift(after));
        let frames = (after - before) as f64;
        let read = f64::from(self.rate) / 100.0;
        Some([(x1 - x0) / frames * read, (y1 - y0) / frames * read])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pose;

    #[test]
    fn travel_is_the_cameras_speed_over_the_read_time() {
        let poses = [0.0, 0.01, 0.03, 0.06].map(|x| {
            Pose::from(Similarity {
                shift: [x, 0.0],
                ..Similarity::IDENTITY
            })
        });
        let repair = RollingShutterRepair {
            path: Some(CameraPath {
                frames: (0..4).map(|frame| Ticks(frame * 10)).collect(),
                poses: poses.to_vec(),
                size: [1920, 1080],
            }),
            ..RollingShutterRepair::default()
        };
        let travel = |time| repair.travel_at(Ticks(time)).map(|[x, _]| x);
        let close = |a: Option<f64>, b: f64| a.is_some_and(|a| (a - b).abs() < 1e-6);
        // Half a frame's read of 0.015 a frame, averaged over the frames either side.
        assert!(close(travel(15), 0.0075));
        // The ends look one frame forward or back.
        assert!(close(travel(0), 0.005));
        assert!(close(travel(99), 0.015));
        assert_eq!(RollingShutterRepair::default().travel_at(Ticks(0)), None);
    }
}
