use serde::{Deserialize, Serialize};

use crate::{Param, Stabilizer};

mod catalog;
mod params;

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
    /// A shadow of the picture's shape behind it.
    DropShadow {
        color: Rgb,
        /// Percent.
        opacity: Param,
        /// Degrees clockwise from up that the shadow falls toward.
        direction: Param,
        /// Pixels of the clip's picture.
        distance: Param,
        softness: Param,
    },
    /// Ripples the picture with sine waves that travel as the clip plays.
    WaveWarp {
        /// Pixels the picture moves each way.
        height: Param,
        /// Pixels from one wave to the next.
        width: Param,
        /// Degrees clockwise from up that the waves travel toward.
        direction: Param,
        /// Waves passing a point each second.
        speed: Param,
    },
    /// Steadies a shaky camera from an analysis of the clip's motion.
    WarpStabilizer(Stabilizer),
}

/// The groups the effects menu lists effects in, as Premiere's Effects panel does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectCategory {
    BlurAndSharpen,
    ColorCorrection,
    Distort,
    ImageControl,
    Keying,
    Perspective,
    Stylize,
    Transform,
}

impl EffectCategory {
    pub const ALL: [Self; 8] = [
        Self::BlurAndSharpen,
        Self::ColorCorrection,
        Self::Distort,
        Self::ImageControl,
        Self::Keying,
        Self::Perspective,
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
            Self::Perspective => "Perspective",
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
