/// How a layer is drawn: everything except the picture itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerLook {
    /// From the canvas center to the picture's center, in canvas pixels, +y down.
    pub offset: [f32; 2],
    /// Relative to the picture fitted to the canvas.
    pub scale: f32,
    /// Clockwise.
    pub rotation_degrees: f32,
    /// Fractions of the picture removed from the left, top, right and bottom.
    pub crop: [f32; 4],
    pub opacity: f32,
    /// Gaussian blur radius in canvas pixels.
    pub blur: f32,
}

impl LayerLook {
    pub const IDENTITY: Self = Self {
        offset: [0.0, 0.0],
        scale: 1.0,
        rotation_degrees: 0.0,
        crop: [0.0; 4],
        opacity: 1.0,
        blur: 0.0,
    };
}

/// Where a layer lands in normalized device coordinates (the canvas spans -1..1, +y
/// up): the center of the uncropped picture and the vectors along its full width and
/// height, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placement {
    pub(crate) center: [f32; 2],
    pub(crate) axis_x: [f32; 2],
    pub(crate) axis_y: [f32; 2],
    pub(crate) crop: [f32; 4],
    pub(crate) opacity: f32,
}

impl Placement {
    pub(crate) const FULL: Self = Self {
        center: [0.0, 0.0],
        axis_x: [2.0, 0.0],
        axis_y: [0.0, -2.0],
        crop: [0.0; 4],
        opacity: 1.0,
    };

    /// Fits the layer to the canvas keeping its aspect ratio, then applies the look.
    pub(crate) fn new(layer: (u32, u32), canvas: (u32, u32), look: &LayerLook) -> Self {
        let (cw, ch) = (canvas.0 as f32, canvas.1 as f32);
        let scale = fit_scale(layer, canvas) * look.scale;
        let (width, height) = (layer.0 as f32 * scale, layer.1 as f32 * scale);
        let (sin, cos) = look.rotation_degrees.to_radians().sin_cos();
        // Rotating in pixel space, where +y is down, turns positive angles clockwise.
        let rotate = |x: f32, y: f32| [x * cos - y * sin, x * sin + y * cos];
        let to_ndc = |[x, y]: [f32; 2]| [x * 2.0 / cw, -y * 2.0 / ch];
        Self {
            center: to_ndc(look.offset),
            axis_x: to_ndc(rotate(width, 0.0)),
            axis_y: to_ndc(rotate(0.0, height)),
            crop: look.crop.map(|side| side.clamp(0.0, 1.0)),
            opacity: look.opacity.clamp(0.0, 1.0),
        }
    }

    pub(crate) fn to_bytes(self) -> Vec<u8> {
        let [cx, cy] = self.center;
        let [xx, xy] = self.axis_x;
        let [yx, yy] = self.axis_y;
        [cx, cy, xx, xy, yx, yy, self.opacity, 0.0]
            .into_iter()
            .chain(self.crop)
            .flat_map(f32::to_le_bytes)
            .collect()
    }
}

/// Canvas pixels per layer pixel when the layer is fitted to the canvas.
pub(crate) fn fit_scale(layer: (u32, u32), canvas: (u32, u32)) -> f32 {
    let (lw, lh) = (layer.0 as f32, layer.1 as f32);
    (canvas.0 as f32 / lw).min(canvas.1 as f32 / lh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rounded(values: [f32; 2]) -> [f32; 2] {
        values.map(|value| (value * 1000.0).round() / 1000.0)
    }

    #[test]
    fn fit_pillarboxes_a_narrower_layer() {
        let placement = Placement::new((960, 1080), (1920, 1080), &LayerLook::IDENTITY);
        assert_eq!(placement.axis_x, [1.0, 0.0]);
        assert_eq!(placement.axis_y, [0.0, -2.0]);
    }

    #[test]
    fn motion_moves_scales_and_turns_clockwise() {
        let look = LayerLook {
            offset: [250.0, 250.0],
            scale: 0.5,
            rotation_degrees: 90.0,
            ..LayerLook::IDENTITY
        };
        let placement = Placement::new((1000, 1000), (1000, 1000), &look);
        // A quarter of the way right and down from the center.
        assert_eq!(placement.center, [0.5, -0.5]);
        // The top edge now runs down the screen, at half the width.
        assert_eq!(rounded(placement.axis_x), [0.0, -1.0]);
        assert_eq!(rounded(placement.axis_y), [-1.0, 0.0]);
    }
}
