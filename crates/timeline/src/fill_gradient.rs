use serde::{Deserialize, Serialize};

use crate::Rgb;

/// A fill that blends from the text's or shape's own color to `to` across it, as
/// Essential Graphics' gradient fills do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FillGradient {
    pub kind: GradientKind,
    pub to: Rgb,
    /// Degrees clockwise from left-to-right; a radial gradient ignores it.
    pub angle: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum GradientKind {
    #[default]
    Linear,
    /// From the center out to the edges.
    Radial,
}

impl GradientKind {
    pub const ALL: [Self; 2] = [Self::Linear, Self::Radial];

    pub fn name(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Radial => "Radial",
        }
    }
}

impl Default for FillGradient {
    fn default() -> Self {
        Self {
            kind: GradientKind::Linear,
            to: Rgb::BLACK,
            angle: 90,
        }
    }
}

impl FillGradient {
    /// How far along the gradient, from 0 to 1, the point `(x, y)` lies in a box
    /// `half` its size each way about its center, with `(x, y)` from that center.
    pub fn amount(&self, (x, y): (f32, f32), half: (f32, f32)) -> f32 {
        let half = (half.0.max(f32::EPSILON), half.1.max(f32::EPSILON));
        let amount = match self.kind {
            GradientKind::Linear => {
                let (sin, cos) = (self.angle as f32).to_radians().sin_cos();
                // Reaches the box's corners, so the end colors show in full there.
                let reach = (half.0 * cos.abs() + half.1 * sin.abs()).max(f32::EPSILON);
                (x * cos + y * sin) / reach * 0.5 + 0.5
            }
            GradientKind::Radial => ((x / half.0).powi(2) + (y / half.1).powi(2)).sqrt(),
        };
        amount.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_gradients_run_corner_to_corner_along_their_angle() {
        let across = FillGradient {
            angle: 0,
            ..FillGradient::default()
        };
        let half = (100.0, 50.0);
        assert_eq!(across.amount((-100.0, 30.0), half), 0.0);
        assert_eq!(across.amount((100.0, -30.0), half), 1.0);
        assert_eq!(across.amount((0.0, 50.0), half), 0.5);
        let down = FillGradient::default();
        assert!((down.amount((0.0, -50.0), half)).abs() < 1e-5);
        let diagonal = FillGradient {
            angle: 45,
            ..FillGradient::default()
        };
        assert!(diagonal.amount((-100.0, -50.0), half) < 1e-5);
        assert!((diagonal.amount((100.0, 50.0), half) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn radial_gradients_reach_their_end_color_at_the_edges() {
        let radial = FillGradient {
            kind: GradientKind::Radial,
            ..FillGradient::default()
        };
        let half = (100.0, 50.0);
        assert_eq!(radial.amount((0.0, 0.0), half), 0.0);
        assert!((radial.amount((0.0, 50.0), half) - 1.0).abs() < 1e-5);
        assert!((radial.amount((50.0, 0.0), half) - 0.5).abs() < 1e-5);
    }
}
