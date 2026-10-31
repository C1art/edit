/// Change Color and Color Balance, both off by default. They work on gamma-encoded
/// levels, 0 to 1, as their settings are given in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorShift {
    pub change_color: Option<ShiftColor>,
    pub balance: Option<ToneBalance>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShiftColor {
    /// Encoded.
    pub target: [f32; 3],
    pub match_by: ShiftMatch,
    /// Turns.
    pub hue: f32,
    /// −1 to 1.
    pub lightness: f32,
    pub saturation: f32,
    /// 0 to 1.
    pub tolerance: f32,
    pub softness: f32,
    pub invert: bool,
    /// Shows how strongly each pixel is changed, in grey, instead of changing it.
    pub show_mask: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShiftMatch {
    Rgb,
    Hue,
    Chroma,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToneBalance {
    /// Red, green and blue pushes, −1 to 1.
    pub shadows: [f32; 3],
    pub midtones: [f32; 3],
    pub highlights: [f32; 3],
    pub preserve_luminosity: bool,
}

impl ColorShift {
    pub const NONE: Self = Self {
        change_color: None,
        balance: None,
    };

    /// The shader's view, as six vec4s; Change Color is off at a negative match mode
    /// and Color Balance at a zero flag.
    pub(crate) fn uniform(&self) -> [[f32; 4]; 6] {
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let change =
            self.change_color
                .map_or([[0.0, 0.0, 0.0, -1.0], [0.0; 4], [0.0; 4]], |change| {
                    let [r, g, b] = change.target;
                    let mode = match change.match_by {
                        ShiftMatch::Rgb => 0.0,
                        ShiftMatch::Hue => 1.0,
                        ShiftMatch::Chroma => 2.0,
                    };
                    [
                        [r, g, b, mode],
                        [
                            change.hue,
                            change.lightness,
                            change.saturation,
                            change.tolerance,
                        ],
                        [
                            change.softness,
                            flag(change.invert),
                            flag(change.show_mask),
                            0.0,
                        ],
                    ]
                });
        let balance = self.balance.unwrap_or(ToneBalance {
            shadows: [0.0; 3],
            midtones: [0.0; 3],
            highlights: [0.0; 3],
            preserve_luminosity: false,
        });
        let [sr, sg, sb] = balance.shadows;
        let [mr, mg, mb] = balance.midtones;
        let [hr, hg, hb] = balance.highlights;
        let [target, amounts, detail] = change;
        [
            target,
            amounts,
            detail,
            [sr, sg, sb, flag(self.balance.is_some())],
            [mr, mg, mb, flag(balance.preserve_luminosity)],
            [hr, hg, hb, 0.0],
        ]
    }
}
