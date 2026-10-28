use serde::{Deserialize, Serialize};

/// An adjustable clip property, held in hundredths of its display unit so the model
/// stays `Eq` and round-trips exactly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Param {
    pub hundredths: i32,
}

impl Param {
    pub const fn new(hundredths: i32) -> Self {
        Self { hundredths }
    }

    pub fn from_value(value: f32) -> Self {
        Self::new((value * 100.0).round() as i32)
    }

    pub fn value(&self) -> f32 {
        self.hundredths as f32 / 100.0
    }
}

/// Where the clip's picture sits in the frame, after it is fitted to the frame.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Motion {
    /// Offset of the picture's center from the frame's, in sequence pixels, +y down.
    pub position: [Param; 2],
    /// Percent.
    pub scale: Param,
    /// Degrees clockwise.
    pub rotation: Param,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            position: [Param::default(), Param::default()],
            scale: Param::new(10_000),
            rotation: Param::default(),
        }
    }
}

/// Percent of the picture removed from each side.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Crop {
    pub left: Param,
    pub top: Param,
    pub right: Param,
    pub bottom: Param,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VideoEffect {
    GaussianBlur {
        /// Blur radius in sequence pixels.
        blurriness: Param,
    },
}

/// Everything that changes how a video clip's picture looks; ignored for audio.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Look {
    #[serde(default)]
    pub motion: Motion,
    /// Percent.
    #[serde(default = "full_opacity")]
    pub opacity: Param,
    #[serde(default)]
    pub crop: Crop,
    /// Applied in order, before motion.
    #[serde(default)]
    pub effects: Vec<VideoEffect>,
}

fn full_opacity() -> Param {
    Param::new(10_000)
}

impl Default for Look {
    fn default() -> Self {
        Self {
            motion: Motion::default(),
            opacity: full_opacity(),
            crop: Crop::default(),
            effects: Vec::new(),
        }
    }
}

impl Look {
    /// The blurriness of all Gaussian blurs together: blurring twice adds the squares.
    pub fn blurriness(&self) -> f32 {
        self.effects
            .iter()
            .map(|effect| match effect {
                VideoEffect::GaussianBlur { blurriness } => blurriness.value().powi(2),
            })
            .sum::<f32>()
            .sqrt()
    }
}
