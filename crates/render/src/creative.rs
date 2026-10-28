/// Lumetri's Creative tone and Vignette, after the look LUT.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Creative {
    /// 0 to 1: lifts the blacks and dulls the whites as faded print stock does.
    pub faded_film: f32,
    /// Pushes toward blue and red (Cb and Cr, -1 to 1) of the shadows, then of the
    /// highlights.
    pub shadow_tint: [f32; 2],
    pub highlight_tint: [f32; 2],
    /// -1 to 1: above 0 the highlight tint reaches into darker tones.
    pub tint_balance: f32,
    pub vignette: CreativeVignette,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreativeVignette {
    /// -1 darkens the edges to black, 1 lightens them to white.
    pub amount: f32,
    /// 0 to 1: where the falloff starts, from the center to the corners.
    pub midpoint: f32,
    /// -1 (rectangular) to 1 (circular); 0 follows the picture's shape.
    pub roundness: f32,
    /// 0 to 1.
    pub feather: f32,
}

impl Creative {
    pub const NONE: Self = Self {
        faded_film: 0.0,
        shadow_tint: [0.0; 2],
        highlight_tint: [0.0; 2],
        tint_balance: 0.0,
        vignette: CreativeVignette {
            amount: 0.0,
            midpoint: 0.5,
            roundness: 0.0,
            feather: 0.5,
        },
    };

    /// The shader's view, as three vec4s.
    pub(crate) fn uniform(&self) -> [f32; 12] {
        let [shadow_blue, shadow_red] = self.shadow_tint;
        let [highlight_blue, highlight_red] = self.highlight_tint;
        let vignette = &self.vignette;
        [
            shadow_blue,
            shadow_red,
            self.faded_film,
            self.tint_balance,
            highlight_blue,
            highlight_red,
            0.0,
            0.0,
            vignette.amount,
            vignette.midpoint,
            vignette.roundness,
            vignette.feather,
        ]
    }
}
