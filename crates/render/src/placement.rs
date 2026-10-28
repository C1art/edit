/// Where a layer lands in the canvas: left, top, right, bottom in normalized device
/// coordinates, where the canvas spans -1..1 with +y up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NdcRect(pub(crate) [f32; 4]);

impl NdcRect {
    pub(crate) const FULL: Self = Self([-1.0, 1.0, 1.0, -1.0]);

    /// The largest centered rectangle with the layer's aspect ratio that fits the canvas.
    pub(crate) fn fit(layer: (u32, u32), canvas: (u32, u32)) -> Self {
        let (lw, lh) = (layer.0 as f32, layer.1 as f32);
        let (cw, ch) = (canvas.0 as f32, canvas.1 as f32);
        let scale = (cw / lw).min(ch / lh);
        let (x, y) = (lw * scale / cw, lh * scale / ch);
        Self([-x, y, x, -y])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_pillarboxes_a_narrower_layer_and_letterboxes_a_wider_one() {
        assert_eq!(
            NdcRect::fit((960, 1080), (1920, 1080)),
            NdcRect([-0.5, 1.0, 0.5, -1.0])
        );
        assert_eq!(
            NdcRect::fit((1920, 540), (1920, 1080)),
            NdcRect([-1.0, 0.5, 1.0, -0.5])
        );
    }
}
