use serde::{Deserialize, Serialize};

use crate::Param;

/// An sRGB color as stored in a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Self = Self::new(0, 0, 0);
    pub const WHITE: Self = Self::new(255, 255, 255);
    pub const GREEN: Self = Self::new(0, 255, 0);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// A video effect from the effect stack. Percentages are stored as their numbers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VideoEffect {
    GaussianBlur {
        /// Blur radius in sequence pixels.
        blurriness: Param,
    },
    HorizontalFlip,
    VerticalFlip,
    BlackAndWhite,
    /// Maps the picture's luminance onto a ramp from `black` to `white`.
    Tint {
        black: Rgb,
        white: Rgb,
        amount: Param,
    },
    Invert {
        amount: Param,
    },
    Sharpen {
        amount: Param,
    },
    /// Darkens (negative amounts) or lightens the picture's edges.
    Vignette {
        amount: Param,
        midpoint: Param,
        feather: Param,
    },
    /// Keys out colors near `color`, as a green or blue screen.
    UltraKey {
        color: Rgb,
        tolerance: Param,
        softness: Param,
    },
    /// Shows the clip only where the picture of another video track is, as Premiere's
    /// Track Matte Key does. The matte is that track's picture in sequence space, so
    /// moving the clip moves it behind the matte.
    TrackMatteKey {
        /// The track's index among the video tracks the clip sits with, bottom first.
        matte: Option<usize>,
        channel: MatteChannel,
        reverse: bool,
    },
    /// Both -100 to 100, on the encoded picture as a display shows it.
    BrightnessContrast {
        brightness: Param,
        contrast: Param,
    },
    /// An exponent on the encoded picture; above 1 darkens the midtones.
    GammaCorrection {
        gamma: Param,
    },
    /// Steps of brightness per channel, 2 to 64.
    Posterize {
        levels: Param,
    },
    /// Grain that changes every frame, in percent.
    Noise {
        amount: Param,
    },
    /// Outlines where the picture changes, dark lines on white unless inverted.
    FindEdges {
        invert: bool,
        /// Percent of the original showing through.
        blend: Param,
    },
    /// Blocks across and down the picture, each one color.
    Mosaic {
        horizontal: Param,
        vertical: Param,
    },
    /// Shifts the picture, wrapping it around, by percent of its width and height.
    Offset {
        shift: [Param; 2],
        /// Percent of the original showing through.
        blend: Param,
    },
    /// Turns the picture around its center, most at the center.
    Twirl {
        /// Degrees.
        angle: Param,
        /// Percent of half the picture's diagonal.
        radius: Param,
    },
    /// -100 (pinched) to 100 (bulging), as through a wide lens.
    LensDistortion {
        curvature: Param,
    },
    /// Smears the picture both ways along a direction.
    DirectionalBlur {
        /// Degrees clockwise from vertical.
        direction: Param,
        /// Sequence pixels.
        length: Param,
    },
}

/// The groups the effects menu lists effects in, as Premiere's Effects panel does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectCategory {
    BlurAndSharpen,
    ColorCorrection,
    Distort,
    ImageControl,
    Keying,
    Stylize,
    Transform,
}

impl EffectCategory {
    pub const ALL: [Self; 7] = [
        Self::BlurAndSharpen,
        Self::ColorCorrection,
        Self::Distort,
        Self::ImageControl,
        Self::Keying,
        Self::Stylize,
        Self::Transform,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::BlurAndSharpen => "Blur & Sharpen",
            Self::ColorCorrection => "Color Correction",
            Self::Distort => "Distort",
            Self::ImageControl => "Image Control",
            Self::Keying => "Keying",
            Self::Stylize => "Stylize",
            Self::Transform => "Transform",
        }
    }
}

/// What of the matte's picture decides how much of the clip shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MatteChannel {
    #[default]
    Alpha,
    /// The brightness of the matte over black.
    Luma,
}

impl MatteChannel {
    pub const ALL: [Self; 2] = [Self::Alpha, Self::Luma];

    pub fn name(self) -> &'static str {
        match self {
            Self::Alpha => "Matte Alpha",
            Self::Luma => "Matte Luma",
        }
    }
}

impl VideoEffect {
    /// Each effect with its default settings, in the order the effects menu lists them.
    pub fn catalog() -> [Self; 20] {
        [
            Self::GaussianBlur {
                blurriness: Param::from_value(10.0),
            },
            Self::HorizontalFlip,
            Self::VerticalFlip,
            Self::BlackAndWhite,
            Self::Tint {
                black: Rgb::BLACK,
                white: Rgb::WHITE,
                amount: Param::from_value(100.0),
            },
            Self::Invert {
                amount: Param::from_value(100.0),
            },
            Self::Sharpen {
                amount: Param::from_value(50.0),
            },
            Self::Vignette {
                amount: Param::from_value(-50.0),
                midpoint: Param::from_value(50.0),
                feather: Param::from_value(50.0),
            },
            Self::UltraKey {
                color: Rgb::GREEN,
                tolerance: Param::from_value(30.0),
                softness: Param::from_value(10.0),
            },
            Self::TrackMatteKey {
                matte: None,
                channel: MatteChannel::Alpha,
                reverse: false,
            },
            Self::BrightnessContrast {
                brightness: Param::default(),
                contrast: Param::default(),
            },
            Self::GammaCorrection {
                gamma: Param::from_value(1.0),
            },
            Self::Posterize {
                levels: Param::from_value(7.0),
            },
            Self::Noise {
                amount: Param::from_value(20.0),
            },
            Self::FindEdges {
                invert: false,
                blend: Param::default(),
            },
            Self::Mosaic {
                horizontal: Param::from_value(10.0),
                vertical: Param::from_value(10.0),
            },
            Self::Offset {
                shift: [Param::from_value(50.0), Param::from_value(50.0)],
                blend: Param::default(),
            },
            Self::Twirl {
                angle: Param::from_value(50.0),
                radius: Param::from_value(75.0),
            },
            Self::LensDistortion {
                curvature: Param::from_value(-30.0),
            },
            Self::DirectionalBlur {
                direction: Param::default(),
                length: Param::from_value(10.0),
            },
        ]
    }

    pub fn category(&self) -> EffectCategory {
        match self {
            Self::GaussianBlur { .. } | Self::Sharpen { .. } | Self::DirectionalBlur { .. } => {
                EffectCategory::BlurAndSharpen
            }
            Self::Tint { .. } | Self::BrightnessContrast { .. } => EffectCategory::ColorCorrection,
            Self::Twirl { .. } | Self::LensDistortion { .. } | Self::Offset { .. } => {
                EffectCategory::Distort
            }
            Self::BlackAndWhite | Self::GammaCorrection { .. } => EffectCategory::ImageControl,
            Self::UltraKey { .. } | Self::TrackMatteKey { .. } => EffectCategory::Keying,
            Self::Invert { .. }
            | Self::Vignette { .. }
            | Self::Posterize { .. }
            | Self::Noise { .. }
            | Self::FindEdges { .. }
            | Self::Mosaic { .. } => EffectCategory::Stylize,
            Self::HorizontalFlip | Self::VerticalFlip => EffectCategory::Transform,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::GaussianBlur { .. } => "Gaussian Blur",
            Self::HorizontalFlip => "Horizontal Flip",
            Self::VerticalFlip => "Vertical Flip",
            Self::BlackAndWhite => "Black & White",
            Self::Tint { .. } => "Tint",
            Self::Invert { .. } => "Invert",
            Self::Sharpen { .. } => "Sharpen",
            Self::Vignette { .. } => "Vignette",
            Self::UltraKey { .. } => "Ultra Key",
            Self::TrackMatteKey { .. } => "Track Matte Key",
            Self::BrightnessContrast { .. } => "Brightness & Contrast",
            Self::GammaCorrection { .. } => "Gamma Correction",
            Self::Posterize { .. } => "Posterize",
            Self::Noise { .. } => "Noise",
            Self::FindEdges { .. } => "Find Edges",
            Self::Mosaic { .. } => "Mosaic",
            Self::Offset { .. } => "Offset",
            Self::Twirl { .. } => "Twirl",
            Self::LensDistortion { .. } => "Lens Distortion",
            Self::DirectionalBlur { .. } => "Directional Blur",
        }
    }

    pub fn params_mut(&mut self) -> Vec<&mut Param> {
        match self {
            Self::GaussianBlur { blurriness } => vec![blurriness],
            Self::GammaCorrection { gamma: one }
            | Self::Posterize { levels: one }
            | Self::Noise { amount: one }
            | Self::FindEdges { blend: one, .. }
            | Self::LensDistortion { curvature: one } => vec![one],
            Self::BrightnessContrast {
                brightness: first,
                contrast: second,
            }
            | Self::Mosaic {
                horizontal: first,
                vertical: second,
            }
            | Self::Twirl {
                angle: first,
                radius: second,
            }
            | Self::DirectionalBlur {
                direction: first,
                length: second,
            } => vec![first, second],
            Self::Offset {
                shift: [x, y],
                blend,
            } => vec![x, y, blend],
            Self::HorizontalFlip
            | Self::VerticalFlip
            | Self::BlackAndWhite
            | Self::TrackMatteKey { .. } => Vec::new(),
            Self::Tint { amount, .. } | Self::Invert { amount } | Self::Sharpen { amount } => {
                vec![amount]
            }
            Self::Vignette {
                amount,
                midpoint,
                feather,
            } => vec![amount, midpoint, feather],
            Self::UltraKey {
                tolerance,
                softness,
                ..
            } => vec![tolerance, softness],
        }
    }

    pub fn params(&self) -> Vec<&Param> {
        match self {
            Self::GaussianBlur { blurriness } => vec![blurriness],
            Self::GammaCorrection { gamma: one }
            | Self::Posterize { levels: one }
            | Self::Noise { amount: one }
            | Self::FindEdges { blend: one, .. }
            | Self::LensDistortion { curvature: one } => vec![one],
            Self::BrightnessContrast {
                brightness: first,
                contrast: second,
            }
            | Self::Mosaic {
                horizontal: first,
                vertical: second,
            }
            | Self::Twirl {
                angle: first,
                radius: second,
            }
            | Self::DirectionalBlur {
                direction: first,
                length: second,
            } => vec![first, second],
            Self::Offset {
                shift: [x, y],
                blend,
            } => vec![x, y, blend],
            Self::HorizontalFlip
            | Self::VerticalFlip
            | Self::BlackAndWhite
            | Self::TrackMatteKey { .. } => Vec::new(),
            Self::Tint { amount, .. } | Self::Invert { amount } | Self::Sharpen { amount } => {
                vec![amount]
            }
            Self::Vignette {
                amount,
                midpoint,
                feather,
            } => vec![amount, midpoint, feather],
            Self::UltraKey {
                tolerance,
                softness,
                ..
            } => vec![tolerance, softness],
        }
    }
}
