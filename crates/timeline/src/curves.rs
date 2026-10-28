use serde::{Deserialize, Serialize};

/// Input and output levels run from black at 0 to white at this.
pub const CURVE_RANGE: u16 = 1_000;

/// A point a tone curve passes through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CurvePoint {
    pub input: u16,
    pub output: u16,
}

/// A tone curve: a smooth line through its points, sorted by input, that never
/// overshoots between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Curve {
    pub points: Vec<CurvePoint>,
}

/// RGB curves as a grading panel has them: the master curve, then one per channel.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Curves {
    pub master: Curve,
    pub red: Curve,
    pub green: Curve,
    pub blue: Curve,
}

impl Default for Curve {
    fn default() -> Self {
        Self {
            points: vec![
                CurvePoint {
                    input: 0,
                    output: 0,
                },
                CurvePoint {
                    input: CURVE_RANGE,
                    output: CURVE_RANGE,
                },
            ],
        }
    }
}

impl Curve {
    pub fn is_identity(&self) -> bool {
        self.points.iter().all(|point| point.input == point.output)
    }

    /// The output for `input`, both 0 to 1. Beyond the end points the curve runs flat.
    pub fn sample(&self, input: f32) -> f32 {
        let points: Vec<[f32; 2]> = self
            .points
            .iter()
            .map(|point| [point.input, point.output].map(|value| value as f32 / CURVE_RANGE as f32))
            .collect();
        smooth_through(&points, input).map_or(input, |value| value.clamp(0.0, 1.0))
    }

    /// Adds a point, or moves the one already at `input`, keeping the points sorted.
    pub fn set(&mut self, point: CurvePoint) {
        match self
            .points
            .binary_search_by_key(&point.input, |existing| existing.input)
        {
            Ok(index) => self.points[index] = point,
            Err(index) => self.points.insert(index, point),
        }
    }
}

impl Curves {
    pub fn is_identity(&self) -> bool {
        self.channels().iter().all(|curve| curve.is_identity())
    }

    pub fn channels(&self) -> [&Curve; 4] {
        [&self.master, &self.red, &self.green, &self.blue]
    }

    /// Red, green and blue outputs at `size` evenly spaced inputs from 0 to 1: the
    /// master curve, then each channel's.
    pub fn table(&self, size: usize) -> Vec<[f32; 3]> {
        let last = size.saturating_sub(1).max(1) as f32;
        (0..size)
            .map(|index| {
                let master = self.master.sample(index as f32 / last);
                [&self.red, &self.green, &self.blue].map(|curve| curve.sample(master))
            })
            .collect()
    }
}

/// The monotone cubic through `points`, sorted by their first coordinate, at `input`,
/// holding the end levels beyond them; `None` without points.
pub(crate) fn smooth_through(points: &[[f32; 2]], input: f32) -> Option<f32> {
    let (first, last) = (points.first()?, points.last()?);
    if input <= first[0] || points.len() == 1 {
        return Some(first[1]);
    }
    if input >= last[0] {
        return Some(last[1]);
    }
    let segment = points
        .windows(2)
        .position(|pair| input < pair[1][0])
        .unwrap_or(points.len() - 2);
    let tangents = monotone_tangents(points);
    let ([x0, y0], [x1, y1]) = (points[segment], points[segment + 1]);
    let width = x1 - x0;
    let t = (input - x0) / width;
    let (t2, t3) = (t * t, t * t * t);
    Some(
        (2.0 * t3 - 3.0 * t2 + 1.0) * y0
            + (t3 - 2.0 * t2 + t) * width * tangents[segment]
            + (-2.0 * t3 + 3.0 * t2) * y1
            + (t3 - t2) * width * tangents[segment + 1],
    )
}

/// Fritsch–Carlson tangents: the slopes a cubic through each point may take without
/// overshooting its neighbours.
fn monotone_tangents(points: &[[f32; 2]]) -> Vec<f32> {
    let secants: Vec<f32> = points
        .windows(2)
        .map(|pair| (pair[1][1] - pair[0][1]) / (pair[1][0] - pair[0][0]).max(f32::EPSILON))
        .collect();
    let mut tangents: Vec<f32> = (0..points.len())
        .map(
            |index| match (index.checked_sub(1).map(|i| secants[i]), secants.get(index)) {
                (Some(before), Some(&after)) if before * after > 0.0 => (before + after) / 2.0,
                (Some(_), Some(_)) => 0.0,
                (Some(only), None) | (None, Some(&only)) => only,
                (None, None) => 0.0,
            },
        )
        .collect();
    for (index, &secant) in secants.iter().enumerate() {
        if secant == 0.0 {
            tangents[index] = 0.0;
            tangents[index + 1] = 0.0;
            continue;
        }
        let (a, b) = (tangents[index] / secant, tangents[index + 1] / secant);
        let length = a.hypot(b);
        if length > 3.0 {
            let scale = 3.0 / length;
            tangents[index] = scale * a * secant;
            tangents[index + 1] = scale * b * secant;
        }
    }
    tangents
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve(points: &[(u16, u16)]) -> Curve {
        Curve {
            points: points
                .iter()
                .map(|&(input, output)| CurvePoint { input, output })
                .collect(),
        }
    }

    #[test]
    fn curves_pass_through_their_points_without_overshooting() {
        let identity = Curve::default();
        assert!((identity.sample(0.37) - 0.37).abs() < 1e-6);
        // A steep rise then a plateau would ring above 0.8 with an ordinary spline.
        let lifted = curve(&[(0, 0), (200, 800), (600, 800), (1_000, 1_000)]);
        assert!((lifted.sample(0.2) - 0.8).abs() < 1e-6);
        let samples: Vec<f32> = (0..=100).map(|i| lifted.sample(i as f32 / 100.0)).collect();
        assert!(samples.windows(2).all(|pair| pair[1] >= pair[0] - 1e-6));
        assert!(samples[20..=60]
            .iter()
            .all(|&value| (value - 0.8).abs() < 1e-6));
        // Outside its end points a curve holds their levels.
        let crushed = curve(&[(100, 50), (900, 950)]);
        assert_eq!(crushed.sample(0.0), 0.05);
        assert_eq!(crushed.sample(1.0), 0.95);
    }

    #[test]
    fn channel_curves_follow_the_master() {
        let curves = Curves {
            master: curve(&[(0, 1_000), (1_000, 0)]),
            red: curve(&[(0, 0), (1_000, 500)]),
            ..Curves::default()
        };
        let table = curves.table(3);
        assert_eq!(table[0], [0.5, 1.0, 1.0]);
        assert_eq!(table[2], [0.0, 0.0, 0.0]);
    }
}
