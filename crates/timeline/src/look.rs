use serde::{Deserialize, Serialize};

use crate::{BlendMode, ColorCorrection, Mask, Param, Ticks, VideoEffect};

/// Where the clip's picture sits in the frame, after it is fitted to the frame.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Motion {
    /// Offset of the picture's center from the frame's, in sequence pixels, +y down.
    pub position: [Param; 2],
    /// Percent; of the height alone unless the scale is uniform.
    pub scale: Param,
    /// Percent, used only when the scale isn't uniform.
    #[serde(default = "full_size")]
    pub scale_width: Param,
    #[serde(default = "uniform")]
    pub uniform_scale: bool,
    /// Degrees clockwise.
    pub rotation: Param,
    /// The point of the fitted picture placed at `position` and scaled and turned
    /// about, from the picture's center in sequence pixels, +y down.
    #[serde(default)]
    pub anchor: [Param; 2],
}

fn full_size() -> Param {
    Param::new(10_000)
}

fn uniform() -> bool {
    true
}

impl Motion {
    pub fn params_mut(&mut self) -> impl Iterator<Item = &mut Param> {
        let [x, y] = &mut self.position;
        let [anchor_x, anchor_y] = &mut self.anchor;
        [
            x,
            y,
            &mut self.scale,
            &mut self.scale_width,
            &mut self.rotation,
            anchor_x,
            anchor_y,
        ]
        .into_iter()
    }

    pub fn params(&self) -> impl Iterator<Item = &Param> {
        let [x, y] = &self.position;
        let [anchor_x, anchor_y] = &self.anchor;
        [
            x,
            y,
            &self.scale,
            &self.scale_width,
            &self.rotation,
            anchor_x,
            anchor_y,
        ]
        .into_iter()
    }

    /// Percent of the width and of the height at source time `time`.
    pub fn scale_at(&self, time: Ticks) -> [f32; 2] {
        let height = self.scale.value_at(time);
        let width = if self.uniform_scale {
            height
        } else {
            self.scale_width.value_at(time)
        };
        [width, height]
    }
}

impl Crop {
    pub fn params_mut(&mut self) -> [&mut Param; 4] {
        [
            &mut self.left,
            &mut self.top,
            &mut self.right,
            &mut self.bottom,
        ]
    }
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            position: [Param::default(), Param::default()],
            scale: full_size(),
            scale_width: full_size(),
            uniform_scale: true,
            rotation: Param::default(),
            anchor: [Param::default(), Param::default()],
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
    #[serde(default, skip_serializing_if = "is_default")]
    pub motion: Motion,
    /// Percent.
    #[serde(default = "full_opacity")]
    pub opacity: Param,
    #[serde(default)]
    pub blend: BlendMode,
    /// What of the picture the opacity applies to; none keeps all of it.
    #[serde(default)]
    pub masks: Vec<Mask>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub crop: Crop,
    /// Applied in order, before motion.
    #[serde(default)]
    pub effects: Vec<VideoEffect>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub color: ColorCorrection,
}

fn full_opacity() -> Param {
    Param::new(10_000)
}

/// Untouched settings are left out of project files: a clip's color settings alone
/// would otherwise take kilobytes, and long projects have thousands of clips.
fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

impl Default for Look {
    fn default() -> Self {
        Self {
            motion: Motion::default(),
            opacity: full_opacity(),
            blend: BlendMode::Normal,
            masks: Vec::new(),
            crop: Crop::default(),
            effects: Vec::new(),
            color: ColorCorrection::default(),
        }
    }
}

impl Look {
    pub fn params_mut(&mut self) -> impl Iterator<Item = &mut Param> {
        self.motion
            .params_mut()
            .chain([&mut self.opacity])
            .chain(self.masks.iter_mut().flat_map(Mask::params_mut))
            .chain(self.crop.params_mut())
            .chain(self.effects.iter_mut().flat_map(VideoEffect::params_mut))
            .chain(self.color.params_mut())
    }

    /// The blurriness of all the blurs together for each of the red, green, blue and
    /// alpha channels: blurring twice adds the squares.
    pub fn blurriness_at(&self, time: Ticks) -> [f32; 4] {
        let mut squares = [0.0f32; 4];
        for effect in &self.effects {
            let each = match effect {
                VideoEffect::GaussianBlur { blurriness } => [blurriness.value_at(time); 4],
                VideoEffect::CameraBlur { percent } => [percent.value_at(time).max(0.0) / 2.0; 4],
                VideoEffect::ChannelBlur(channels) => [
                    &channels.red,
                    &channels.green,
                    &channels.blue,
                    &channels.alpha,
                ]
                .map(|blurriness| blurriness.value_at(time)),
                _ => continue,
            };
            for (square, blurriness) in squares.iter_mut().zip(each) {
                *square += blurriness.powi(2);
            }
        }
        squares.map(f32::sqrt)
    }
}

impl Look {
    pub fn params(&self) -> impl Iterator<Item = &Param> {
        let crop = &self.crop;
        let effects = self.effects.iter().flat_map(VideoEffect::params);
        self.motion
            .params()
            .chain([&self.opacity])
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_and_changing_visit_the_same_params() {
        let mut look = Look::default();
        for (index, param) in look.params_mut().enumerate() {
            param.toggle_key(Ticks(i64::try_from(index).unwrap()));
        }
        let keyed = look.params().filter(|param| param.is_animated()).count();
        assert_eq!(keyed, look.params_mut().count());
    }
}
