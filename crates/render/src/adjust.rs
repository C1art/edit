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
    pub levels: Levels,
    pub proc_amp: ProcAmp,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Levels {
    /// The encoded black and white levels that map onto `output`'s.
    pub input: [f32; 2],
    pub output: [f32; 2],
    pub gamma: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcAmp {
    /// Added to the encoded luma.
    pub brightness: f32,
    /// A gain on the encoded luma about mid grey.
    pub contrast: f32,
    /// Radians the colors turn about the grey axis.
    pub hue: f32,
    pub saturation: f32,
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
        levels: Levels {
            input: [0.0, 1.0],
            output: [0.0, 1.0],
            gamma: 1.0,
        },
        proc_amp: ProcAmp {
            brightness: 0.0,
            contrast: 1.0,
            hue: 0.0,
            saturation: 1.0,
        },
    };

    /// The shader's view, as six vec4s; a negative level turns its effect off.
    pub(crate) fn uniform(&self) -> [f32; 24] {
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let [r, g, b] = self.balance;
        let (luma_threshold, luma_cutoff) = self
            .luma_key
            .map_or((-1.0, 0.0), |key| (key.threshold, key.cutoff));
        let alpha = self.alpha;
        let Levels {
            input: [in_black, in_white],
            output: [out_black, out_white],
            gamma,
        } = self.levels;
        let proc_amp = self.proc_amp;
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
            in_black,
            // Equal levels would divide by zero.
            in_white.max(in_black + 1e-3),
            out_black,
            out_white,
            proc_amp.brightness,
            proc_amp.contrast.max(0.0),
            proc_amp.hue,
            proc_amp.saturation.max(0.0),
            gamma.max(0.01),
            0.0,
            0.0,
            0.0,
        ]
    }
}
