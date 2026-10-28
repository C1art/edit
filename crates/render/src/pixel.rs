use crate::recolor::Recolor;
use crate::reshape::Reshape;
use crate::style::PictureStyle;

/// Per-pixel effects on a layer, all off by default. Colors are linear RGB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelEffects {
    /// Mirrors the picture left to right and top to bottom.
    pub flip: [bool; 2],
    /// Colors within this chroma distance of `key_color` become transparent.
    pub key: Option<ChromaKey>,
    pub sharpen: f32,
    /// 0 to 1: blend toward the luminance mapped from `tint_black` to `tint_white`.
    pub tint: f32,
    pub tint_black: [f32; 3],
    pub tint_white: [f32; 3],
    /// 0 to 1.
    pub invert: f32,
    pub vignette: Vignette,
    pub style: PictureStyle,
    pub reshape: Reshape,
    pub recolor: Recolor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromaKey {
    pub color: [f32; 3],
    /// 0 to 1.
    pub tolerance: f32,
    /// 0 to 1.
    pub softness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vignette {
    /// -1 darkens the edges to black, 1 lightens them to white.
    pub amount: f32,
    /// 0 to 1: where the falloff starts, from the center to the corners.
    pub midpoint: f32,
    /// 0 to 1.
    pub feather: f32,
}

impl PixelEffects {
    pub const NONE: Self = Self {
        flip: [false; 2],
        key: None,
        sharpen: 0.0,
        tint: 0.0,
        tint_black: [0.0; 3],
        tint_white: [1.0; 3],
        invert: 0.0,
        vignette: Vignette {
            amount: 0.0,
            midpoint: 0.5,
            feather: 0.5,
        },
        style: PictureStyle::NONE,
        reshape: Reshape::NONE,
        recolor: Recolor::NONE,
    };

    /// The shader's view, as five vec4s. A negative tolerance turns keying off.
    pub(crate) fn uniform(&self) -> [f32; 20] {
        let key = self.key.unwrap_or(ChromaKey {
            color: [0.0; 3],
            tolerance: -1.0,
            softness: 0.0,
        });
        let [kr, kg, kb] = key.color;
        let [br, bg, bb] = self.tint_black;
        let [wr, wg, wb] = self.tint_white;
        let vignette = &self.vignette;
        [
            kr,
            kg,
            kb,
            key.tolerance,
            key.softness,
            self.sharpen,
            self.invert,
            self.tint,
            br,
            bg,
            bb,
            vignette.amount,
            wr,
            wg,
            wb,
            0.0,
            vignette.midpoint,
            vignette.feather,
            0.0,
            0.0,
        ]
    }
}
