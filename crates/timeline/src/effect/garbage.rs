//! Garbage mattes: polygons that keep only the part of the picture inside them.

use serde::{Deserialize, Serialize};

use crate::{Param, Ticks};

/// Keeps the picture inside the polygon through `points`, each in percent of the
/// picture's width and height, clockwise from its upper left corner.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GarbageMatte {
    pub points: Vec<[Param; 2]>,
}

impl GarbageMatte {
    /// `count` points spread evenly around the picture's edge, so that the matte at
    /// first keeps everything.
    pub fn new(count: usize) -> Self {
        let points = (0..count)
            .map(|index| {
                let along = (index * 4) as f64 / count as f64;
                let (side, part) = (along.floor() as usize, (along.fract() * 100.0) as f32);
                let [x, y] = match side {
                    0 => [part, 0.0],
                    1 => [100.0, part],
                    2 => [100.0 - part, 100.0],
                    _ => [0.0, 100.0 - part],
                };
                [Param::from_value(x), Param::from_value(y)]
            })
            .collect();
        Self { points }
    }

    pub fn name(&self) -> &'static str {
        match self.points.len() {
            4 => "Four-Point Garbage Matte",
            8 => "Eight-Point Garbage Matte",
            _ => "Sixteen-Point Garbage Matte",
        }
    }

    /// The polygon at `time` as fractions of the picture from its top left.
    pub fn outline_at(&self, time: Ticks) -> Vec<[f32; 2]> {
        self.points
            .iter()
            .map(|point| point.each_ref().map(|axis| axis.value_at(time) / 100.0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_start_around_the_picture_edge_clockwise() {
        let outline = |count| GarbageMatte::new(count).outline_at(Ticks::ZERO);
        assert_eq!(outline(4), [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        let eight = outline(8);
        assert_eq!(eight[1], [0.5, 0.0]);
        assert_eq!(eight[7], [0.0, 0.5]);
        assert_eq!(outline(16)[5], [1.0, 0.25]);
    }
}
