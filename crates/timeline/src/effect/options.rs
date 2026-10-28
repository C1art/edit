use serde::{Deserialize, Serialize};

/// What Change to Color changes of a matching color.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColorChange {
    #[default]
    Hue,
    HueLightness,
    HueLightnessSaturation,
    /// Takes the target's hue and saturation, keeping its own lightness.
    Colorize,
}

impl ColorChange {
    pub const ALL: [Self; 4] = [
        Self::Hue,
        Self::HueLightness,
        Self::HueLightnessSaturation,
        Self::Colorize,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Hue => "Hue",
            Self::HueLightness => "Hue & Lightness",
            Self::HueLightnessSaturation => "Hue, Lightness & Saturation",
            Self::Colorize => "Colorize",
        }
    }
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
