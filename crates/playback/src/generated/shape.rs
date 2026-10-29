use timeline::{Shape, ShapeKind};

use super::coverage::Coverage;

/// How much of each pixel `shape` covers on a `width` by `height` canvas, centered,
/// grown `grow` pixels outward, less the shape itself when `hollow`.
pub(super) fn shape_coverage(
    shape: &Shape,
    width: u32,
    height: u32,
    grow: f32,
    hollow: bool,
) -> Option<Coverage> {
    if shape.width == 0 || shape.height == 0 {
        return None;
    }
    let center = (width as f32 / 2.0, height as f32 / 2.0);
    let half = (shape.width as f32 / 2.0, shape.height as f32 / 2.0);
    let reach = (half.0 + grow + 1.0, half.1 + grow + 1.0);
    let region = (
        (center.0 - reach.0).floor() as i64,
        (center.1 - reach.1).floor() as i64,
        (center.0 + reach.0).ceil() as i64,
        (center.1 + reach.1).ceil() as i64,
    );
    let edge = |distance: f32, grow: f32| (grow + 0.5 - distance).clamp(0.0, 1.0);
    Some(Coverage::from_fn(
        width as usize,
        height as usize,
        region,
        |x, y| {
            let distance = signed_distance(shape, x - center.0, y - center.1);
            let outer = edge(distance, grow);
            if hollow {
                outer - edge(distance, 0.0)
            } else {
                outer
            }
        },
    ))
}

/// Distance from `(x, y)`, relative to the shape's center, to its edge: negative inside.
/// Exact for rectangles; for ellipses a first-order estimate, exact on the edge, which
/// is where antialiasing and strokes need it.
fn signed_distance(shape: &Shape, x: f32, y: f32) -> f32 {
    let half = (shape.width as f32 / 2.0, shape.height as f32 / 2.0);
    match shape.kind {
        ShapeKind::Rectangle => {
            let radius = (shape.corner_radius as f32).min(half.0).min(half.1);
            let (qx, qy) = (x.abs() - half.0 + radius, y.abs() - half.1 + radius);
            qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius
        }
        ShapeKind::Ellipse => {
            let level = (x / half.0).powi(2) + (y / half.1).powi(2) - 1.0;
            let slope = 2.0 * (x / half.0.powi(2)).hypot(y / half.1.powi(2));
            if slope == 0.0 {
                return -half.0.min(half.1);
            }
            level / slope
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_covers(shape: &Shape, grow: f32, hollow: bool, area: f32) {
        let coverage = shape_coverage(shape, 64, 64, grow, hollow).unwrap();
        let total: f32 = coverage.alpha.iter().sum();
        assert!((total - area).abs() < 0.5, "{total} for {area}");
    }

    #[test]
    fn shapes_cover_their_area_and_strokes_only_the_ring_around_them() {
        let corner = 1.0 - std::f32::consts::FRAC_PI_4;
        let mut shape = Shape::new(ShapeKind::Rectangle);
        (shape.width, shape.height) = (20, 10);
        assert_covers(&shape, 0.0, false, 200.0);
        // A stroke rounds the corners it goes around.
        assert_covers(&shape, 2.0, true, 24.0 * 14.0 - 4.0 * 4.0 * corner - 200.0);
        shape.corner_radius = 5;
        assert_covers(&shape, 0.0, false, 200.0 - 4.0 * 25.0 * corner);
        shape.kind = ShapeKind::Ellipse;
        assert_covers(&shape, 0.0, false, std::f32::consts::PI * 10.0 * 5.0);
    }
}
