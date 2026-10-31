/// Cineon Converter, on gamma-encoded levels. Codes are 10-bit printing density,
/// 0 to 1023; the internal points are fractions of the picture's range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cineon {
    pub conversion: CineonKind,
    pub black_code: f32,
    pub white_code: f32,
    pub internal_black: f32,
    pub internal_white: f32,
    /// The display gamma of the linear side; Kodak's is 1.7.
    pub gamma: f32,
    /// 0 to 1: how much of the top of the range eases into white instead of clipping.
    pub highlight_rolloff: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CineonKind {
    LogToLinear,
    LinearToLog,
    LogToLog,
}

impl Cineon {
    pub const NONE: Option<Self> = None;

    /// The shader's view, as two vec4s; off at a zero kind.
    pub(crate) fn uniform(cineon: Option<&Self>) -> [[f32; 4]; 2] {
        let Some(cineon) = cineon else {
            return [[0.0; 4]; 2];
        };
        let kind = match cineon.conversion {
            CineonKind::LogToLinear => 1.0,
            CineonKind::LinearToLog => 2.0,
            CineonKind::LogToLog => 3.0,
        };
        [
            [
                cineon.black_code,
                cineon.white_code.max(cineon.black_code + 1.0),
                cineon.internal_black,
                cineon.internal_white,
            ],
            [
                cineon.gamma.max(0.01),
                cineon.highlight_rolloff.clamp(0.0, 1.0),
                kind,
                0.0,
            ],
        ]
    }
}
