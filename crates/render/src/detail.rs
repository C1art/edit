/// Unsharp Mask, Median (and Dust & Scratches), Edge Feather and Noise HLS, all off by
/// default. Sizes are in the picture's pixels and levels run from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detail {
    pub unsharp: Option<UnsharpMask>,
    pub median: Option<MedianFilter>,
    pub edge_feather: f32,
    pub noise: Option<NoiseHls>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnsharpMask {
    /// 1 adds the whole difference from the blur.
    pub amount: f32,
    pub radius: f32,
    /// Differences from the blur smaller than this are left alone.
    pub threshold: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MedianFilter {
    pub radius: f32,
    /// Only pixels further than this from the median are replaced.
    pub threshold: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoiseHls {
    /// Fractions of a full turn of hue, of the lightness range and of saturation.
    pub hue: f32,
    pub lightness: f32,
    pub saturation: f32,
    pub grain_size: f32,
    /// Radians; each turn gives a new pattern.
    pub phase: f32,
}

impl Detail {
    pub const NONE: Self = Self {
        unsharp: None,
        median: None,
        edge_feather: 0.0,
        noise: None,
    };

    /// The shader's view, as three vec4s; each effect is off at zero amount or radius.
    pub(crate) fn uniform(&self) -> [[f32; 4]; 3] {
        let unsharp = self.unsharp.unwrap_or(UnsharpMask {
            amount: 0.0,
            radius: 0.0,
            threshold: 0.0,
        });
        let median = self.median.unwrap_or(MedianFilter {
            radius: 0.0,
            threshold: 0.0,
        });
        let noise = self.noise.unwrap_or(NoiseHls {
            hue: 0.0,
            lightness: 0.0,
            saturation: 0.0,
            grain_size: 1.0,
            phase: 0.0,
        });
        [
            [
                unsharp.amount,
                unsharp.radius,
                unsharp.threshold,
                median.radius,
            ],
            [
                median.threshold,
                self.edge_feather,
                noise.grain_size.max(1.0),
                noise.phase,
            ],
            [noise.hue, noise.lightness, noise.saturation, 0.0],
        ]
    }
}
