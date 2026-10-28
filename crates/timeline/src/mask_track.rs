use serde::{Deserialize, Serialize};

use crate::{Mask, MaskShape, Param, Similarity, Ticks};

/// Corners of the polygon an ellipse is tracked as.
const ELLIPSE_POINTS: usize = 32;
const OUTLINE_POINTS: usize = 64;

/// What of the picture's motion a tracked mask follows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaskTracking {
    Position,
    PositionRotation,
    #[default]
    PositionScaleRotation,
}

impl MaskTracking {
    pub const ALL: [Self; 3] = [
        Self::Position,
        Self::PositionRotation,
        Self::PositionScaleRotation,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Position => "Position",
            Self::PositionRotation => "Position & Rotation",
            Self::PositionScaleRotation => "Position, Scale & Rotation",
        }
    }

    pub fn turns(self) -> bool {
        self != Self::Position
    }

    pub fn scales(self) -> bool {
        self == Self::PositionScaleRotation
    }
}

/// Converts fractions of a picture's width and height to the square units of
/// [`Similarity`] and back.
#[derive(Debug, Clone, Copy)]
pub struct PictureUnits {
    /// The picture's width and height over its longer side.
    shape: [f64; 2],
}

impl PictureUnits {
    pub fn new(size: [u32; 2]) -> Self {
        let size = size.map(|side| f64::from(side.max(1)));
        let unit = size[0].max(size[1]);
        Self {
            shape: size.map(|side| side / unit),
        }
    }

    pub fn to_square(&self, point: [f64; 2]) -> [f64; 2] {
        self.vector_to_square(point.map(|fraction| fraction - 0.5))
    }

    pub fn from_square(&self, point: [f64; 2]) -> [f64; 2] {
        self.vector_from_square(point)
            .map(|fraction| fraction + 0.5)
    }

    pub fn vector_to_square(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        [x * self.shape[0], y * self.shape[1]]
    }

    pub fn vector_from_square(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        [x / self.shape[0], y / self.shape[1]]
    }
}

impl Mask {
    /// The outline of what the mask covers at `time`, before feather and expansion, as
    /// fractions of a picture of `size`.
    pub fn region_at(&self, time: Ticks, size: [u32; 2]) -> Vec<[f64; 2]> {
        if self.shape == MaskShape::Path {
            return self
                .outline_at(time, OUTLINE_POINTS)
                .into_iter()
                .map(|point| point.map(f64::from))
                .collect();
        }
        let units = PictureUnits::new(size);
        let percent = |param: &Param| f64::from(param.value_at(time)) / 100.0;
        let [half_x, half_y] = units
            .vector_to_square(self.size.each_ref().map(percent))
            .map(|side| side / 2.0);
        let place = Similarity {
            shift: units.to_square(self.center.each_ref().map(percent)),
            angle: f64::from(self.rotation.value_at(time)).to_radians(),
            scale: 1.0,
        };
        let corners: Vec<[f64; 2]> = match self.shape {
            MaskShape::Ellipse => (0..ELLIPSE_POINTS)
                .map(|index| {
                    let angle = std::f64::consts::TAU * index as f64 / ELLIPSE_POINTS as f64;
                    [half_x * angle.cos(), half_y * angle.sin()]
                })
                .collect(),
            _ => vec![
                [-half_x, -half_y],
                [half_x, -half_y],
                [half_x, half_y],
                [-half_x, half_y],
            ],
        };
        corners
            .into_iter()
            .map(|corner| units.from_square(place.apply(corner)))
            .collect()
    }

    /// Keys the mask where it is at `from` and at each pose's time, carried there by the
    /// pose: a similarity in the square units of a picture of `size`. Only what its
    /// tracking follows is keyed, except that a path's keys hold every vertex.
    pub fn follow(&mut self, from: Ticks, poses: &[(Ticks, Similarity)], size: [u32; 2]) {
        let units = PictureUnits::new(size);
        let percent = |param: &Param| f64::from(param.value_at(from)) / 100.0;
        if self.shape == MaskShape::Path {
            for vertex in &mut self.path {
                let position = units.to_square(vertex.position.each_ref().map(percent));
                let handle = units.vector_to_square(vertex.handle.each_ref().map(percent));
                for (axis, param) in vertex.position.iter_mut().enumerate() {
                    key(param, from, poses, |pose| {
                        100.0 * units.from_square(pose.apply(position))[axis]
                    });
                }
                for (axis, param) in vertex.handle.iter_mut().enumerate() {
                    key(param, from, poses, |pose| {
                        let turn = Similarity {
                            shift: [0.0, 0.0],
                            ..*pose
                        };
                        100.0 * units.vector_from_square(turn.apply(handle))[axis]
                    });
                }
            }
            return;
        }
        let center = units.to_square(self.center.each_ref().map(percent));
        for (axis, param) in self.center.iter_mut().enumerate() {
            key(param, from, poses, |pose| {
                100.0 * units.from_square(pose.apply(center))[axis]
            });
        }
        if self.tracking.turns() {
            let rotation = f64::from(self.rotation.value_at(from));
            key(&mut self.rotation, from, poses, |pose| {
                rotation + pose.angle.to_degrees()
            });
        }
        if self.tracking.scales() {
            for param in &mut self.size {
                let size = f64::from(param.value_at(from));
                key(param, from, poses, |pose| size * pose.scale);
            }
        }
    }
}

/// Keys `param` with its value at `from` and, at each pose's time, the value `value`
/// gives for the pose.
fn key(
    param: &mut Param,
    from: Ticks,
    poses: &[(Ticks, Similarity)],
    value: impl Fn(&Similarity) -> f64,
) {
    if param.is_animated() {
        param.set_at(from, param.value_at(from));
    } else {
        param.toggle_animation(from);
    }
    for (time, pose) in poses {
        param.set_at(*time, value(pose) as f32);
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;
    use crate::PathVertex;

    fn assert_near(param: &Param, time: Ticks, expected: f32) {
        let value = param.value_at(time);
        assert!((value - expected).abs() < 0.02, "{value} is not {expected}");
    }

    #[test]
    fn a_tracked_mask_moves_turns_and_grows_with_the_picture_in_its_own_units() {
        let size = [1920, 1080];
        let pose = Similarity {
            shift: [0.1, 0.0],
            angle: FRAC_PI_2,
            scale: 2.0,
        };
        let (from, at) = (Ticks(0), Ticks(10));
        let mut rectangle = Mask::new(MaskShape::Rectangle);
        rectangle.center = [Param::from_value(25.0), Param::from_value(50.0)];
        rectangle.size = [Param::from_value(20.0), Param::from_value(10.0)];
        rectangle.follow(from, &[(at, pose)], size);
        // The center sits a quarter width left of the picture's; turned a quarter
        // above it, doubled, then shifted a tenth of the width right.
        assert_near(&rectangle.center[0], at, 60.0);
        assert_near(&rectangle.center[1], at, 50.0 - 50.0 * 16.0 / 9.0);
        assert_near(&rectangle.rotation, at, 90.0);
        assert_near(&rectangle.size[0], at, 40.0);
        assert_near(&rectangle.center[0], from, 25.0);

        let mut path = Mask::new(MaskShape::Path);
        path.path = vec![PathVertex::new([0.5, 0.5], [0.1, 0.0])];
        path.follow(from, &[(at, pose)], size);
        let handle = &path.path[0].handle;
        // A handle a tenth of the width long, turned upright and doubled, spans more
        // of the shorter height.
        assert_near(&handle[0], at, 0.0);
        assert_near(&handle[1], at, 2.0 * 10.0 * 16.0 / 9.0);
        assert_near(&path.path[0].position[0], at, 60.0);
    }
}
