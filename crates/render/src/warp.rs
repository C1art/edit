/// Warps that a layer's effects can stack, each moving the picture's plane.
pub const MAX_WARPS: usize = 4;

/// How far, in picture heights, the eye is from a picture turned in depth.
const FOCAL_LENGTH: f64 = 2.0;
/// Nearer than this share of the focal length, the picture passes the eye and shows
/// nothing.
const NEAREST: f64 = 0.01;
/// Skewing further than this many degrees would stretch the picture without bound.
const STEEPEST_SKEW: f32 = 85.0;

/// A map of the picture's plane, measured in fractions of the picture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Warp {
    /// Where the picture's corners go: upper left, upper right, lower left, lower right.
    CornerPin([[f32; 2]; 4]),
    Transform(PlaneTransform),
    /// Turns the picture in depth about its middle, seen in perspective.
    Turn {
        /// Radians about the vertical axis, the right side coming nearer.
        swivel: f32,
        /// Radians about the horizontal axis, the bottom coming nearer.
        tilt: f32,
        /// Picture heights it moves away by.
        distance: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneTransform {
    /// The point that turns and scales in place.
    pub anchor: [f32; 2],
    /// Where the anchor goes.
    pub position: [f32; 2],
    /// Factors of the width and the height.
    pub scale: [f32; 2],
    /// Degrees the picture slants by along the skew axis.
    pub skew_degrees: f32,
    /// Degrees clockwise from vertical.
    pub skew_axis_degrees: f32,
    /// Degrees clockwise.
    pub rotation_degrees: f32,
}

impl Warp {
    /// Where `point` of the picture goes, on a picture `aspect` times as wide as it is
    /// high; none when it passes behind the eye.
    pub(crate) fn map(&self, point: [f64; 2], aspect: f64) -> Option<[f64; 2]> {
        match self {
            Self::CornerPin(corners) => {
                let corners = corners.map(|corner| corner.map(f64::from));
                let [a, b, c, d, e, f, g, h] = super::reshape::square_to_quad(corners)?;
                let [u, v] = point;
                let w = g * u + h * v + 1.0;
                (w.abs() > f64::EPSILON).then(|| [(a * u + b * v + c) / w, (d * u + e * v + f) / w])
            }
            Self::Transform(transform) => Some(transform.map(point, aspect)),
            &Self::Turn {
                swivel,
                tilt,
                distance,
            } => {
                let (swivel, tilt) = (f64::from(swivel), f64::from(tilt));
                let x = (point[0] - 0.5) * aspect;
                let y = point[1] - 0.5;
                let (x, z) = (x * swivel.cos(), -x * swivel.sin());
                let (y, z) = (
                    y * tilt.cos() + z * tilt.sin(),
                    -y * tilt.sin() + z * tilt.cos(),
                );
                let depth = FOCAL_LENGTH + z + f64::from(distance);
                if depth < FOCAL_LENGTH * NEAREST {
                    return None;
                }
                let seen = FOCAL_LENGTH / depth;
                Some([x * seen / aspect + 0.5, y * seen + 0.5])
            }
        }
    }
}

impl PlaneTransform {
    fn map(&self, [x, y]: [f64; 2], aspect: f64) -> [f64; 2] {
        // In picture heights, so turning keeps right angles on any picture.
        let square = |[x, y]: [f32; 2]| [f64::from(x) * aspect, f64::from(y)];
        let [anchor_x, anchor_y] = square(self.anchor);
        let [position_x, position_y] = square(self.position);
        let [width, height] = self.scale.map(f64::from);
        let p = [(x * aspect - anchor_x) * width, (y - anchor_y) * height];
        let axis = f64::from(self.skew_axis_degrees).to_radians();
        let slant = f64::from(self.skew_degrees.clamp(-STEEPEST_SKEW, STEEPEST_SKEW))
            .to_radians()
            .tan();
        let [u, v] = turn(p, -axis);
        let p = turn([u + slant * v, v], axis);
        let [u, v] = turn(p, f64::from(self.rotation_degrees).to_radians());
        [(u + position_x) / aspect, v + position_y]
    }
}

/// Clockwise on the screen, where y runs down.
fn turn([x, y]: [f64; 2], radians: f64) -> [f64; 2] {
    let (sin, cos) = radians.sin_cos();
    [x * cos - y * sin, x * sin + y * cos]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6
    }

    #[test]
    fn a_quarter_turn_of_a_wide_picture_keeps_its_shape() {
        let transform = Warp::Transform(PlaneTransform {
            anchor: [0.5, 0.5],
            position: [0.5, 0.5],
            scale: [1.0, 1.0],
            skew_degrees: 0.0,
            skew_axis_degrees: 0.0,
            rotation_degrees: 90.0,
        });
        // On a picture twice as wide, the turned corner lands a quarter width in and
        // half a height above.
        let corner = transform.map([0.0, 0.0], 2.0);
        assert!(close(corner.unwrap(), [0.75, -0.5]), "{corner:?}");
    }

    #[test]
    fn scaling_about_the_anchor_keeps_the_anchor_in_place() {
        let transform = Warp::Transform(PlaneTransform {
            anchor: [0.0, 0.0],
            position: [0.0, 0.0],
            scale: [0.5, 0.25],
            skew_degrees: 0.0,
            skew_axis_degrees: 0.0,
            rotation_degrees: 0.0,
        });
        assert!(close(transform.map([1.0, 1.0], 1.5).unwrap(), [0.5, 0.25]));
    }

    #[test]
    fn a_picture_swivelled_toward_the_eye_grows_on_its_near_side() {
        let turn = Warp::Turn {
            swivel: 0.5,
            tilt: 0.0,
            distance: 0.0,
        };
        let near = turn.map([1.0, 0.0], 1.0).unwrap();
        let far = turn.map([0.0, 0.0], 1.0).unwrap();
        assert!(near[1] < far[1], "{near:?} {far:?}");
        assert!(close(turn.map([0.5, 0.5], 1.0).unwrap(), [0.5, 0.5]));
    }
}
