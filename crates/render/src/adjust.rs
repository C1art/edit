/// Effects that adjust a layer's encoded values, tile it or reread its alpha, all off
/// by default. Levels are gamma-encoded, 0 to 1, as Premiere compares them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Adjust {
    /// Gains on the encoded red, green and blue.
    pub balance: [f32; 3],
    /// Copies of the picture across and down; 1 shows it once.
    pub replicate: f32,
    /// Encoded channels brighter than this are inverted.
    pub solarize: Option<f32>,
    /// Encoded luma below this turns black, the rest white.
    pub threshold: Option<f32>,
    pub luma_key: Option<LumaKey>,
    pub alpha: AlphaAdjust,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LumaKey {
    /// Encoded luma at which the picture is opaque.
    pub threshold: f32,
    /// Encoded luma at which it is fully transparent.
    pub cutoff: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlphaAdjust {
    /// 0 to 1.
    pub opacity: f32,
    pub ignore: bool,
    pub invert: bool,
    pub mask_only: bool,
}

impl Adjust {
    pub const NONE: Self = Self {
        balance: [1.0; 3],
        replicate: 1.0,
        solarize: None,
        threshold: None,
        luma_key: None,
        alpha: AlphaAdjust {
            opacity: 1.0,
            ignore: false,
            invert: false,
            mask_only: false,
        },
    };

    /// The shader's view, as three vec4s; a negative level turns its effect off.
    pub(crate) fn uniform(&self) -> [f32; 12] {
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let [r, g, b] = self.balance;
        let (luma_threshold, luma_cutoff) = self
            .luma_key
            .map_or((-1.0, 0.0), |key| (key.threshold, key.cutoff));
        let alpha = self.alpha;
        [
            r,
            g,
            b,
            self.replicate.max(1.0),
            self.solarize.unwrap_or(-1.0),
            self.threshold.unwrap_or(-1.0),
            luma_threshold,
            luma_cutoff,
            alpha.opacity,
            flag(alpha.ignore),
            flag(alpha.invert),
            flag(alpha.mask_only),
        ]
    }
}
