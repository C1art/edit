use serde::{Deserialize, Serialize};

use crate::{ColorCorrection, Mask, Param, Ticks, VideoEffect};

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

/// Everything that changes how a video clip's picture looks; ignored for audio.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Look {
    #[serde(default)]
    pub motion: Motion,
    /// Percent.
    #[serde(default = "full_opacity")]
    pub opacity: Param,
    /// What of the picture the opacity applies to; none keeps all of it.
    #[serde(default)]
    pub masks: Vec<Mask>,
    #[serde(default)]
    pub crop: Crop,
    /// Applied in order, before motion.
    #[serde(default)]
    pub effects: Vec<VideoEffect>,
    #[serde(default)]
    pub color: ColorCorrection,
}

fn full_opacity() -> Param {
    Param::new(10_000)
}

impl Default for Look {
    fn default() -> Self {
        Self {
            motion: Motion::default(),
            opacity: full_opacity(),
            masks: Vec::new(),
            crop: Crop::default(),
            effects: Vec::new(),
            color: ColorCorrection::default(),
        }
    }
}

impl Look {
    /// The blurriness of all Gaussian blurs together: blurring twice adds the squares.
    pub fn blurriness_at(&self, time: Ticks) -> f32 {
        self.effects
            .iter()
            .map(|effect| match effect {
                VideoEffect::GaussianBlur { blurriness } => blurriness.value_at(time).powi(2),
                _ => 0.0,
            })
            .sum::<f32>()
            .sqrt()
    }
}

impl Look {
    pub fn params(&self) -> impl Iterator<Item = &Param> {
        let motion = &self.motion;
        let crop = &self.crop;
        let effects = self.effects.iter().flat_map(VideoEffect::params);
        motion
            .position
            .iter()
            .chain([&motion.scale, &motion.rotation, &self.opacity])
            .chain([&crop.left, &crop.top, &crop.right, &crop.bottom])
            .chain(self.masks.iter().flat_map(Mask::params))
            .chain(effects)
            .chain(self.color.params())
    }

    /// Source times of every keyframe, sorted and deduplicated.
    pub fn key_times(&self) -> Vec<Ticks> {
        let mut times: Vec<Ticks> = self
            .params()
            .flat_map(|param| param.keys.iter().map(|key| key.time))
            .collect();
        times.sort_unstable();
        times.dedup();
        times
    }
}
