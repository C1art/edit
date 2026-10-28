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
    pub fn catalog() -> [Self; 10] {
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
        ]
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
        }
    }

    pub fn params_mut(&mut self) -> Vec<&mut Param> {
        match self {
            Self::GaussianBlur { blurriness } => vec![blurriness],
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
