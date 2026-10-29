use serde::{Deserialize, Serialize};

/// The width of one stored pixel over its height, as the exact fraction `num / den`
/// (e.g. `10/11` for NTSC DV, whose pixels are narrower than they are tall).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PixelAspect {
    num: u32,
    den: u32,
}

impl Default for PixelAspect {
    fn default() -> Self {
        Self::SQUARE
    }
}

impl PixelAspect {
    pub const SQUARE: Self = Self { num: 1, den: 1 };

    /// The ratios Premiere offers to conform footage to, by name.
    pub const PRESETS: [(&'static str, Self); 8] = [
        ("Square Pixels (1.0)", Self::SQUARE),
        ("D1/DV NTSC (0.9091)", Self { num: 10, den: 11 }),
        (
            "D1/DV NTSC Widescreen 16:9 (1.2121)",
            Self { num: 40, den: 33 },
        ),
        ("D1/DV PAL (1.0940)", Self { num: 59, den: 54 }),
        (
            "D1/DV PAL Widescreen 16:9 (1.4587)",
            Self { num: 118, den: 81 },
        ),
        ("Anamorphic 2:1 (2.0)", Self { num: 2, den: 1 }),
        ("HD Anamorphic 1080 (1.333)", Self { num: 4, den: 3 }),
        ("DVCPRO HD (1.5)", Self { num: 3, den: 2 }),
    ];

    /// In lowest terms, so equal shapes compare equal however a file spells them.
    pub const fn new(num: u32, den: u32) -> Option<Self> {
        if num == 0 || den == 0 {
            return None;
        }
        let (mut a, mut b) = (num, den);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        Some(Self {
            num: num / a,
            den: den / a,
        })
    }

    pub const fn num(self) -> u32 {
        self.num
    }

    pub const fn den(self) -> u32 {
        self.den
    }

    pub fn is_square(&self) -> bool {
        self.num == self.den
    }

    /// For display and drawing only; sizes in the model stay in whole pixels.
    pub fn ratio(self) -> f32 {
        self.num as f32 / self.den as f32
    }

    /// How many square pixels wide `width` stored pixels look, to the nearest one.
    pub fn display_width(self, width: u32) -> u32 {
        let den = u64::from(self.den);
        let wide = (u64::from(width) * u64::from(self.num) + den / 2) / den;
        u32::try_from(wide).unwrap_or(u32::MAX)
    }
}

/// How a file's alpha channel is read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlphaInterpretation {
    /// Colors are stored apart from coverage.
    #[default]
    Straight,
    /// Colors are stored already multiplied by coverage.
    Premultiplied,
    /// Every pixel is opaque.
    Ignore,
    /// Transparent becomes opaque and the other way round.
    Invert,
}

impl AlphaInterpretation {
    pub const ALL: [Self; 4] = [
        Self::Straight,
        Self::Premultiplied,
        Self::Ignore,
        Self::Invert,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Straight => "Straight",
            Self::Premultiplied => "Premultiplied",
            Self::Ignore => "Ignore Alpha Channel",
            Self::Invert => "Invert Alpha Channel",
        }
    }

    pub fn is_straight(&self) -> bool {
        *self == Self::Straight
    }
}

/// The primaries and transfer curve a file's pictures were encoded in, as Premiere's
/// Color Space Override names them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColorSpace {
    #[default]
    Rec709,
    Rec2020,
    Rec2100Pq,
    Rec2100Hlg,
    P3D65,
    P3D65Pq,
}

/// The colors a color space's pure red, green and blue are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Primaries {
    Rec709,
    Rec2020,
    P3D65,
}

/// How a color space's code values map to light.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transfer {
    /// The display gamma of standard dynamic range.
    Sdr,
    /// Perceptual Quantizer: absolute light up to 10,000 nits.
    Pq,
    /// Hybrid Log-Gamma: scene light, gamma below and logarithmic above.
    Hlg,
}

impl ColorSpace {
    pub const ALL: [Self; 6] = [
        Self::Rec709,
        Self::Rec2020,
        Self::Rec2100Pq,
        Self::Rec2100Hlg,
        Self::P3D65,
        Self::P3D65Pq,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Rec709 => "Rec. 709",
            Self::Rec2020 => "Rec. 2020",
            Self::Rec2100Pq => "Rec. 2100 PQ",
            Self::Rec2100Hlg => "Rec. 2100 HLG",
            Self::P3D65 => "P3 D65",
            Self::P3D65Pq => "P3 D65 PQ",
        }
    }

    pub fn primaries(self) -> Primaries {
        match self {
            Self::Rec709 => Primaries::Rec709,
            Self::Rec2020 | Self::Rec2100Pq | Self::Rec2100Hlg => Primaries::Rec2020,
            Self::P3D65 | Self::P3D65Pq => Primaries::P3D65,
        }
    }

    pub fn transfer(self) -> Transfer {
        match self {
            Self::Rec709 | Self::Rec2020 | Self::P3D65 => Transfer::Sdr,
            Self::Rec2100Pq | Self::P3D65Pq => Transfer::Pq,
            Self::Rec2100Hlg => Transfer::Hlg,
        }
    }

    pub fn is_rec709(&self) -> bool {
        *self == Self::Rec709
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_width_rounds_to_the_nearest_square_pixel() {
        let ntsc = PixelAspect::new(10, 11).unwrap();
        assert_eq!(ntsc.display_width(720), 655);
        let hd_anamorphic = PixelAspect::new(4, 3).unwrap();
        assert_eq!(hd_anamorphic.display_width(1440), 1920);
        assert_eq!(PixelAspect::new(1440, 1080), Some(hd_anamorphic));
    }
}
