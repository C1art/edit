/// How a layer combines with the picture of another track, which the compositor draws
/// as a group of its own: Difference Matte, Compound Blur or Texturize. Sizes are in
/// canvas pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Combine {
    /// Keys out where the layer matches the other picture to within `tolerance`,
    /// 0 to 1, fading in over `softness` more.
    Difference {
        tolerance: f32,
        softness: f32,
        /// Evens out grain in both before they are compared.
        blur: f32,
        view: DifferenceShown,
    },
    /// Blurs the layer by up to `maximum` where the other picture is bright.
    Blur { maximum: f32, invert: bool },
    /// Lights the layer as though embossed with the other picture's brightness.
    Texture {
        /// Radians clockwise from up, toward the light.
        direction: f32,
        /// 0 to 2.
        contrast: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifferenceShown {
    Keyed,
    Source,
    Matte,
}

impl Combine {
    /// The shader's view, as two vec4s, the first starting with the kind: 1 difference,
    /// 2 blur, 3 texture.
    pub(crate) fn uniform(combine: Option<&Self>) -> [[f32; 4]; 2] {
        match combine {
            None => [[0.0; 4]; 2],
            Some(Self::Difference {
                tolerance,
                softness,
                blur,
                view,
            }) => {
                let view = match view {
                    DifferenceShown::Keyed => 0.0,
                    DifferenceShown::Source => 1.0,
                    DifferenceShown::Matte => 2.0,
                };
                [
                    [1.0, tolerance.max(0.0), softness.max(0.0), blur.max(0.0)],
                    [view, 0.0, 0.0, 0.0],
                ]
            }
            Some(Self::Blur { maximum, invert }) => [
                [2.0, maximum.max(0.0), f32::from(u8::from(*invert)), 0.0],
                [0.0; 4],
            ],
            Some(Self::Texture {
                direction,
                contrast,
            }) => [[3.0, *direction, contrast.clamp(0.0, 2.0), 0.0], [0.0; 4]],
        }
    }
}
