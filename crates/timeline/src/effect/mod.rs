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
    /// Keys out colors whose encoded values lie near `color`, with no spill handling.
    ColorKey {
        color: Rgb,
        /// Percent of the largest distance between colors.
        tolerance: Param,
        softness: Param,
    },
    /// Turns everything but colors near `color` grey.
    LeaveColor {
        color: Rgb,
        /// Percent the other colors lose.
        amount: Param,
        tolerance: Param,
        softness: Param,
        matching: ColorMatch,
    },
    /// Fills the picture with a blend from one color to another.
    GradientRamp {
        /// Percent of the picture's width and height.
        start: [Param; 2],
        start_color: Rgb,
        end: [Param; 2],
        end_color: Rgb,
        shape: RampShape,
        /// Percent of the original showing through.
        blend: Param,
    },
    /// Reflects the picture across a line through `center`, the side the line's
    /// normal points away from showing on both.
    Mirror {
        /// Percent of the picture's width and height.
        center: [Param; 2],
        /// Degrees clockwise; at 0 the left half shows on the right.
        angle: Param,
    },
    /// Moves the picture's corners, each in percent of its width and height: upper left,
    /// upper right, lower left, lower right.
    CornerPin {
        corners: [[Param; 2]; 4],
    },
    /// Holds each picture for a while, as footage shot at `frame_rate`.
    PosterizeTime {
        frame_rate: Param,
    },
    /// Flashes a color over the picture at regular intervals of the media's time.
    StrobeLight {
        color: Rgb,
        /// Percent of the color over the picture while it flashes.
        blend: Param,
        /// Seconds each flash lasts.
        duration: Param,
        /// Seconds from the start of one flash to the next.
        period: Param,
    },
    /// Burns the time into the picture, white on a box.
    Timecode {
        /// The box's center, in percent of the picture's width and height.
        position: [Param; 2],
        /// Text height in percent of the picture's height.
        size: Param,
        /// Percent opacity of the box behind the text.
        opacity: Param,
        source: TimecodeSource,
    },
    /// Moves, scales, skews and turns the picture about its anchor point, as Motion
    /// does but in its place among the effects.
    Transform {
        /// The point of the picture that turns and scales in place, in percent of its
        /// width and height.
        anchor: [Param; 2],
        /// Where the anchor point goes, in percent of the picture's width and height.
        position: [Param; 2],
        /// Percent of the width, then of the height.
        scale: [Param; 2],
        /// Degrees the picture slants by along the skew axis.
        skew: Param,
        /// Degrees clockwise from vertical.
        skew_axis: Param,
        /// Degrees clockwise.
        rotation: Param,
        /// Percent.
        opacity: Param,
    },
    /// Turns the picture in depth about its middle, as a card seen in perspective.
    Basic3D {
        /// Degrees about the vertical axis, the right side coming nearer.
        swivel: Param,
        /// Degrees about the horizontal axis, the bottom coming nearer.
        tilt: Param,
        /// How far the picture moves away, in percent of its height.
        distance: Param,
    },
    /// Bulges the picture inside a circle as if it were wrapped around a ball.
    Spherize {
        /// Pixels of the clip's picture.
        radius: Param,
        /// Percent of the picture's width and height.
        center: [Param; 2],
    },
    /// Shows the picture's edges in grey relief, lit from one side.
    Emboss {
        /// Degrees clockwise from up that the light comes from.
        direction: Param,
        /// Pixels of the clip's picture the relief stands out by.
        relief: Param,
        /// Percent.
        contrast: Param,
        /// Percent of the original showing through.
        blend: Param,
    },
}

/// Which time the Timecode effect shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimecodeSource {
    /// The time in the media file, at its own frame rate.
    #[default]
    Media,
    /// The time since the clip's first frame, at the sequence's frame rate.
    Clip,
}

impl TimecodeSource {
    pub const ALL: [Self; 2] = [Self::Media, Self::Clip];

    pub fn name(self) -> &'static str {
        match self {
            Self::Media => "Media",
            Self::Clip => "Clip",
        }
    }
}

/// How Leave Color tells colors apart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColorMatch {
    /// By distance between the encoded colors.
    #[default]
    Rgb,
    /// By hue alone, keeping every shade of it.
    Hue,
}

impl ColorMatch {
    pub const ALL: [Self; 2] = [Self::Rgb, Self::Hue];

    pub fn name(self) -> &'static str {
        match self {
            Self::Rgb => "Using RGB",
            Self::Hue => "Using Hue",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RampShape {
    #[default]
    Linear,
    /// Rings around the start, reaching the end color at the end point's distance.
    Radial,
}

impl RampShape {
    pub const ALL: [Self; 2] = [Self::Linear, Self::Radial];

    pub fn name(self) -> &'static str {
        match self {
            Self::Linear => "Linear Ramp",
            Self::Radial => "Radial Ramp",
        }
    }
}

/// The groups the effects menu lists effects in, as Premiere's Effects panel does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectCategory {
    BlurAndSharpen,
    ColorCorrection,
    Distort,
    Generate,
    ImageControl,
    Keying,
    Perspective,
    Stylize,
    Time,
    Transform,
    Video,
}

impl EffectCategory {
    pub const ALL: [Self; 11] = [
        Self::BlurAndSharpen,
        Self::ColorCorrection,
        Self::Distort,
        Self::Generate,
        Self::ImageControl,
        Self::Keying,
        Self::Perspective,
        Self::Stylize,
        Self::Time,
        Self::Transform,
        Self::Video,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::BlurAndSharpen => "Blur & Sharpen",
            Self::ColorCorrection => "Color Correction",
            Self::Distort => "Distort",
            Self::Generate => "Generate",
            Self::ImageControl => "Image Control",
            Self::Keying => "Keying",
            Self::Perspective => "Perspective",
            Self::Stylize => "Stylize",
            Self::Time => "Time",
            Self::Transform => "Transform",
            Self::Video => "Video",
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
