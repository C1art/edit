use timeline::MaskShape;

/// Masks a layer can carry; the shader's uniform has room for this many.
pub const MAX_MASKS: usize = 4;

/// An opacity mask in the picture's own space, so it moves, scales and turns with it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerMask {
    pub shape: MaskShape,
    /// Fractions of the picture's width and height from its top left.
    pub center: [f32; 2],
    /// Fractions of the picture's width and height.
    pub size: [f32; 2],
    /// Clockwise.
    pub rotation_degrees: f32,
    /// The soft edge's width and how far the outline grows, both in canvas pixels of
    /// the picture as fitted to the canvas, before motion scales it.
    pub feather: f32,
    pub expansion: f32,
    pub opacity: f32,
    pub inverted: bool,
}

/// The fitted picture's size in canvas pixels and the mask count, then three `vec4`s
/// for each of [`MAX_MASKS`] masks, unused ones zero.
pub(crate) fn uniform(masks: &[Option<LayerMask>; MAX_MASKS], picture: [f32; 2]) -> Vec<f32> {
    let used: Vec<&LayerMask> = masks.iter().flatten().collect();
    let flag = |set: bool| if set { 1.0 } else { 0.0 };
    let packed = used.iter().flat_map(|mask| {
        let [x, y] = mask.center;
        let [width, height] = mask.size;
        [
            x,
            y,
            width,
            height,
            mask.rotation_degrees.to_radians(),
            mask.feather.max(0.0),
            mask.expansion,
            mask.opacity.clamp(0.0, 1.0),
            flag(mask.shape == MaskShape::Rectangle),
            flag(mask.inverted),
            0.0,
            0.0,
        ]
    });
    [picture[0], picture[1], used.len() as f32, 0.0]
        .into_iter()
        .chain(packed)
        .chain(std::iter::repeat(0.0))
        .take(4 + MAX_MASKS * 12)
        .collect()
}
