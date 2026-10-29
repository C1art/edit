/// Convolution Kernel, Extract, Shadow/Highlight, Video Limiter, ASC CDL and Color
/// Replace, all off by default. They work on gamma-encoded levels, 0 to 1, as their
/// settings are given in; sizes are in the picture's pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tone {
    pub convolution: Option<Convolution>,
    pub extract: Option<ExtractRange>,
    pub shadow_highlight: Option<ShadowHighlight>,
    pub limiter: Option<Limiter>,
    pub cdl: Option<Cdl>,
    pub color_replace: Option<ColorReplace>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Convolution {
    /// Row by row from the top left, already divided by the scale.
    pub kernel: [f32; 9],
    pub offset: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtractRange {
    pub black: f32,
    pub white: f32,
    pub softness: f32,
    pub invert: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowHighlight {
    /// 0 to 1.
    pub shadow: f32,
    pub highlight: f32,
    pub shadow_width: f32,
    pub highlight_width: f32,
    pub radius: f32,
    /// −1 to 1.
    pub midtone_contrast: f32,
    pub blend: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LimitBy {
    Luma,
    Chroma,
    ChromaAndLuma,
    /// Chroma first, then luma for what chroma alone can't bring down.
    Smart,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limiter {
    pub level: f32,
    /// Where under the level, as a fraction of it, the signal starts easing in.
    pub knee: f32,
    pub by: LimitBy,
    /// Shows what would be limited in this linear color instead of limiting it.
    pub warning: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cdl {
    pub slope: [f32; 3],
    pub offset: [f32; 3],
    pub power: [f32; 3],
    pub saturation: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorReplace {
    pub target: [f32; 3],
    pub replacement: [f32; 3],
    /// 0 to 1.
    pub similarity: f32,
    pub solid: bool,
}

impl Tone {
    pub const NONE: Self = Self {
        convolution: None,
        extract: None,
        shadow_highlight: None,
        limiter: None,
        cdl: None,
        color_replace: None,
    };

    /// The shader's view, as thirteen vec4s; each effect is off at a zero flag.
    pub(crate) fn uniform(&self) -> [[f32; 4]; 13] {
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let (kernel, offset, convolving) = match self.convolution {
            Some(convolution) => (convolution.kernel, convolution.offset, 1.0),
            None => ([0.0; 9], 0.0, 0.0),
        };
        let extract = self.extract.map_or([0.0; 4], |extract| {
            let mode = if extract.invert { -1.0 } else { 1.0 };
            [extract.black, extract.white, extract.softness, mode]
        });
        let lift = self.shadow_highlight.unwrap_or(ShadowHighlight {
            shadow: 0.0,
            highlight: 0.0,
            shadow_width: 0.0,
            highlight_width: 0.0,
            radius: 0.0,
            midtone_contrast: 0.0,
            blend: 1.0,
        });
        let (limit, warning) = self.limiter.map_or(([0.0; 4], [0.0; 4]), |limiter| {
            let by = match limiter.by {
                LimitBy::Luma => 1.0,
                LimitBy::Chroma => 2.0,
                LimitBy::ChromaAndLuma => 3.0,
                LimitBy::Smart => 4.0,
            };
            let [r, g, b] = limiter.warning.unwrap_or([0.0; 3]);
            (
                [limiter.level, limiter.knee, by, 0.0],
                [r, g, b, flag(limiter.warning.is_some())],
            )
        });
        let cdl = self.cdl.unwrap_or(Cdl {
            slope: [1.0; 3],
            offset: [0.0; 3],
            power: [1.0; 3],
            saturation: 1.0,
        });
        let [sr, sg, sb] = cdl.slope;
        let [or, og, ob] = cdl.offset;
        let [pr, pg, pb] = cdl.power;
        let replace = self.color_replace.unwrap_or(ColorReplace {
            target: [0.0; 3],
            replacement: [0.0; 3],
            similarity: 0.0,
            solid: false,
        });
        let [tr, tg, tb] = replace.target;
        let [rr, rg, rb] = replace.replacement;
        let replacing = match self.color_replace {
            Some(ColorReplace { solid: true, .. }) => 2.0,
            Some(_) => 1.0,
            None => 0.0,
        };
        [
            [kernel[0], kernel[1], kernel[2], kernel[3]],
            [kernel[4], kernel[5], kernel[6], kernel[7]],
            [kernel[8], offset, convolving, 0.0],
            extract,
            [
                lift.shadow,
                lift.highlight,
                lift.shadow_width,
                lift.highlight_width,
            ],
            [lift.radius, lift.midtone_contrast, lift.blend, 0.0],
            limit,
            warning,
            [sr, sg, sb, cdl.saturation],
            [or, og, ob, flag(self.cdl.is_some())],
            [pr, pg, pb, 0.0],
            [tr, tg, tb, replace.similarity],
            [rr, rg, rb, replacing],
        ]
    }
}
