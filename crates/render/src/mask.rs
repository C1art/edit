use timeline::MaskShape;

/// Masks a layer can carry; the shader's uniform has room for this many.
pub const MAX_MASKS: usize = 4;
/// Points a path mask's outline can have; the shader keeps this many for each mask.
pub const MAX_OUTLINE_POINTS: usize = 64;

/// A path mask's closed outline, as fractions of the picture from its top left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outline {
    points: [[f32; 2]; MAX_OUTLINE_POINTS],
    len: usize,
}

impl Outline {
    pub const NONE: Self = Self {
        points: [[0.0; 2]; MAX_OUTLINE_POINTS],
        len: 0,
    };

    /// The first [`MAX_OUTLINE_POINTS`] of `points`.
    pub fn new(points: &[[f32; 2]]) -> Self {
        let mut outline = Self::NONE;
        outline.len = points.len().min(MAX_OUTLINE_POINTS);
        outline.points[..outline.len].copy_from_slice(&points[..outline.len]);
        outline
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

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
    /// A path mask's shape, in place of the center, size and rotation.
    pub outline: Outline,
}

/// The fitted picture's size in canvas pixels and the mask count, then three `vec4`s
/// for each of [`MAX_MASKS`] masks, then their outlines two points to a `vec4`, unused
/// ones zero.
pub(crate) fn uniform(masks: &[Option<LayerMask>; MAX_MASKS], picture: [f32; 2]) -> Vec<f32> {
    let used: Vec<&LayerMask> = masks.iter().flatten().collect();
    let flag = |set: bool| if set { 1.0 } else { 0.0 };
    // The shader numbers shapes in declaration order.
    let code = |shape: MaskShape| shape as u32 as f32;
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
            code(mask.shape),
            flag(mask.inverted),
            mask.outline.len as f32,
            0.0,
        ]
    });
    let outlines = (0..MAX_MASKS).flat_map(|slot| {
        let outline = used.get(slot).map_or(&Outline::NONE, |mask| &mask.outline);
        outline.points.into_iter().flatten()
    });
    [picture[0], picture[1], used.len() as f32, 0.0]
        .into_iter()
        .chain(packed)
        .chain(std::iter::repeat(0.0))
        .take(4 + MAX_MASKS * 12)
        .chain(outlines)
        .collect()
}
