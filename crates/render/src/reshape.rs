/// Warps that move a layer's whole picture, applied after its other effects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reshape {
    pub mirror: Option<Mirror>,
    /// Where the picture's corners go, as fractions of its width and height: upper
    /// left, upper right, lower left, lower right.
    pub corner_pin: Option<[[f32; 2]; 4]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mirror {
    /// Fractions of the picture's width and height.
    pub center: [f32; 2],
    /// Radians clockwise; at 0 the left half shows on the right.
    pub angle: f32,
}

/// Corners nearer than this to lying on one line leave the picture as it is.
const DEGENERATE: f64 = 1e-9;

impl Reshape {
    pub const NONE: Self = Self {
        mirror: None,
        corner_pin: None,
    };

    /// The shader's view, as five vec4s: the mirror's center, angle and 1 when on; the
    /// rows of the map from the pinned picture back to the source, the first ending in
    /// 1 when on; then the pinned picture's bounds.
    pub(crate) fn uniform(&self) -> [f32; 20] {
        let mirror = self.mirror.map_or([0.0; 4], |mirror| {
            [mirror.center[0], mirror.center[1], mirror.angle, 1.0]
        });
        let pin = self
            .corner_pin
            .and_then(|corners| Some((unpin(corners)?, bounds(corners))));
        let ([first, second, third], bounds) = pin.unwrap_or(([[0.0; 3]; 3], [0.0; 4]));
        let on = if pin.is_some() { 1.0 } else { 0.0 };
        [
            mirror[0], mirror[1], mirror[2], mirror[3], first[0], first[1], first[2], on,
            second[0], second[1], second[2], 0.0, third[0], third[1], third[2], 0.0, bounds[0],
            bounds[1], bounds[2], bounds[3],
        ]
    }
}

fn bounds(corners: [[f32; 2]; 4]) -> [f32; 4] {
    corners.iter().fold(
        [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ],
        |[left, top, right, bottom], &[x, y]| {
            [left.min(x), top.min(y), right.max(x), bottom.max(y)]
        },
    )
}

/// The projective map from where the corners went back to the unit square, as the rows
/// of a matrix on homogeneous points; none when the corners don't span an area.
fn unpin(corners: [[f32; 2]; 4]) -> Option<[[f32; 3]; 3]> {
    let [a, b, c, d, e, f, g, h] = square_to_quad(corners.map(|corner| corner.map(f64::from)))?;
    let forward = [[a, b, c], [d, e, f], [g, h, 1.0]];
    let inverse = invert(forward)?;
    Some(inverse.map(|row| row.map(|value| value as f32)))
}

/// The map taking the unit square's corners to `corners`, after Heckbert's
/// "Fundamentals of Texture Mapping": x = (au + bv + c) / (gu + hv + 1), and y likewise
/// with d, e and f.
fn square_to_quad(corners: [[f64; 2]; 4]) -> Option<[f64; 8]> {
    // Around the square: (0,0), (1,0), (1,1), (0,1).
    let [[x0, y0], [x1, y1], [x3, y3], [x2, y2]] = corners;
    let (dx1, dx2, dx3) = (x1 - x2, x3 - x2, x0 - x1 + x2 - x3);
    let (dy1, dy2, dy3) = (y1 - y2, y3 - y2, y0 - y1 + y2 - y3);
    let det = dx1 * dy2 - dx2 * dy1;
    if det.abs() < DEGENERATE {
        return None;
    }
    let g = (dx3 * dy2 - dx2 * dy3) / det;
    let h = (dx1 * dy3 - dx3 * dy1) / det;
    Some([
        x1 - x0 + g * x1,
        x3 - x0 + h * x3,
        x0,
        y1 - y0 + g * y1,
        y3 - y0 + h * y3,
        y0,
        g,
        h,
    ])
}

fn invert(m: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let cofactor = |row: usize, column: usize| {
        let (r0, r1) = ((row + 1) % 3, (row + 2) % 3);
        let (c0, c1) = ((column + 1) % 3, (column + 2) % 3);
        m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
    };
    let det: f64 = (0..3)
        .map(|column| m[0][column] * cofactor(0, column))
        .sum();
    if det.abs() < DEGENERATE {
        return None;
    }
    // The inverse is the transposed cofactors over the determinant.
    Some([0, 1, 2].map(|row| [0, 1, 2].map(|column| cofactor(column, row) / det)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(rows: [[f32; 3]; 3], [x, y]: [f32; 2]) -> [f32; 2] {
        let [u, v, w] = rows.map(|[a, b, c]| a * x + b * y + c);
        [u / w, v / w]
    }

    #[test]
    fn each_pinned_corner_maps_back_to_its_corner_of_the_picture() {
        let corners = [[0.1, -0.2], [1.3, 0.05], [-0.1, 0.9], [0.8, 1.2]];
        let rows = unpin(corners).unwrap();
        let square = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
        for (corner, expected) in corners.into_iter().zip(square) {
            let [x, y] = apply(rows, corner);
            assert!((x - expected[0]).abs() < 1e-4 && (y - expected[1]).abs() < 1e-4);
        }
    }

    #[test]
    fn corners_on_one_line_leave_the_picture_unpinned() {
        assert_eq!(
            unpin([[0.0, 0.0], [0.5, 0.5], [1.0, 1.0], [2.0, 2.0]]),
            None
        );
    }
}
