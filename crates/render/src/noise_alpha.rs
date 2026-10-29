/// Noise Alpha: random opacity per pixel of the picture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoisyAlpha {
    /// 0 to 1.
    pub amount: f32,
    /// Squared noise leaves most pixels nearly untouched.
    pub squared: bool,
    pub mix: AlphaMix,
    pub overflow: Overflow,
    /// Picks the pattern.
    pub seed: f32,
    /// Animated noise fades from one pattern to the next over each whole number;
    /// still noise ignores it.
    pub phase: Option<f32>,
    /// Patterns after which animated noise repeats.
    pub cycle: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlphaMix {
    Add,
    /// Only where the picture is opaque.
    Clamp,
    /// In proportion to the picture's opacity.
    Scale,
    /// Only where the picture is transparent.
    Mask,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Overflow {
    Clip,
    WrapBack,
    WrapAround,
}

impl NoisyAlpha {
    pub const NONE: Option<Self> = None;

    /// The shader's view, as two vec4s; off at a zero flag.
    pub(crate) fn uniform(noise: Option<&Self>) -> [[f32; 4]; 2] {
        let Some(noise) = noise else {
            return [[0.0; 4]; 2];
        };
        let mix = match noise.mix {
            AlphaMix::Add => 0.0,
            AlphaMix::Clamp => 1.0,
            AlphaMix::Scale => 2.0,
            AlphaMix::Mask => 3.0,
        };
        let overflow = match noise.overflow {
            Overflow::Clip => 0.0,
            Overflow::WrapBack => 1.0,
            Overflow::WrapAround => 2.0,
        };
        let squared = if noise.squared { 1.0 } else { 0.0 };
        let kind = if noise.phase.is_some() { 2.0 } else { 1.0 };
        [
            [noise.amount.max(0.0), squared, mix, overflow],
            [
                noise.seed,
                noise.phase.unwrap_or(0.0),
                noise.cycle.map_or(0.0, |cycle| cycle.max(1.0).round()),
                kind,
            ],
        ]
    }
}
