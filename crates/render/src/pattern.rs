/// Ellipse and Cell Pattern, both off by default. Points are fractions of the picture's
/// width and height, sizes are in its pixels, and colors are gamma-encoded, 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pattern {
    pub ellipse: Option<EllipseRing>,
    pub cells: Option<Cells>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EllipseRing {
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub thickness: f32,
    /// 0 to 1, of the thickness.
    pub softness: f32,
    pub inside: [f32; 3],
    pub outside: [f32; 3],
    /// Draws over the picture rather than over black.
    pub over_picture: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellShape {
    Bubbles,
    Crystals,
    Plates,
    StaticPlates,
    Crystallize,
    Pillow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cells {
    pub shape: CellShape,
    pub invert: bool,
    /// 1 leaves the shades as they are.
    pub contrast: f32,
    /// 0 to 1.5.
    pub disperse: f32,
    pub size: f32,
    pub offset: [f32; 2],
    /// Turns.
    pub evolution: f32,
    /// 0 to 1.
    pub blend: f32,
}

impl Pattern {
    pub const NONE: Self = Self {
        ellipse: None,
        cells: None,
    };

    /// The shader's view, as seven vec4s; Ellipse is off at a zero flag and Cell Pattern
    /// at a negative shape.
    pub(crate) fn uniform(&self) -> [[f32; 4]; 7] {
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let ring = self.ellipse.unwrap_or(EllipseRing {
            center: [0.0; 2],
            size: [0.0; 2],
            thickness: 0.0,
            softness: 0.0,
            inside: [0.0; 3],
            outside: [0.0; 3],
            over_picture: false,
        });
        let [cx, cy] = ring.center;
        let [width, height] = ring.size.map(|side| side.max(0.0));
        let [ir, ig, ib] = ring.inside;
        let [or, og, ob] = ring.outside;
        let cells = self.cells.unwrap_or(Cells {
            shape: CellShape::Bubbles,
            invert: false,
            contrast: 1.0,
            disperse: 0.0,
            size: 1.0,
            offset: [0.0; 2],
            evolution: 0.0,
            blend: 0.0,
        });
        let shape = self.cells.map_or(-1.0, |cells| match cells.shape {
            CellShape::Bubbles => 0.0,
            CellShape::Crystals => 1.0,
            CellShape::Plates => 2.0,
            CellShape::StaticPlates => 3.0,
            CellShape::Crystallize => 4.0,
            CellShape::Pillow => 5.0,
        });
        let [ox, oy] = cells.offset;
        [
            [cx, cy, width, height],
            [
                ring.thickness.max(0.0),
                ring.softness.clamp(0.0, 1.0),
                flag(ring.over_picture),
                flag(self.ellipse.is_some()),
            ],
            [ir, ig, ib, 0.0],
            [or, og, ob, 0.0],
            [shape, cells.size.max(1.0), cells.disperse, cells.contrast],
            [ox, oy, cells.evolution, cells.blend.clamp(0.0, 1.0)],
            [flag(cells.invert), 0.0, 0.0, 0.0],
        ]
    }
}
