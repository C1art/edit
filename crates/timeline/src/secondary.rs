use serde::{Deserialize, Serialize};

use crate::{ColorWheel, Param};

/// A correction confined to the colors a hue, saturation and lightness key selects, as
/// a grading panel's HSL secondary: skin tones warmed, a sky deepened.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct HslSecondary {
    pub enabled: bool,
    /// Degrees, 0 to 360, red at 0: the hue the key centers on.
    pub hue: Param,
    /// Degrees either side of the hue that are fully keyed.
    pub hue_width: Param,
    /// Percent: the saturations fully keyed, lowest then highest.
    pub saturation_range: [Param; 2],
    /// Percent: the lightnesses fully keyed, lowest then highest.
    pub lightness_range: [Param; 2],
    /// Percent: how gradually the key fades out beyond its ranges.
    pub softness: Param,
    /// Keys everything the ranges leave out instead.
    pub inverted: bool,
    /// Shows the key: kept colors over grey, for tuning the ranges.
    pub show_mask: bool,
    /// -100 (cool) to 100 (warm).
    pub temperature: Param,
    /// -100 (green) to 100 (magenta).
    pub tint: Param,
    /// -100 to 100.
    pub contrast: Param,
    /// Percent; 0 is monochrome.
    pub saturation: Param,
    /// Pushes the keyed colors toward a hue across every tone.
    pub wheel: ColorWheel,
}

impl Default for HslSecondary {
    fn default() -> Self {
        Self {
            enabled: false,
            hue: Param::default(),
            hue_width: Param::from_value(30.0),
            saturation_range: [Param::from_value(15.0), Param::from_value(100.0)],
            lightness_range: [Param::from_value(5.0), Param::from_value(95.0)],
            softness: Param::from_value(10.0),
            inverted: false,
            show_mask: false,
            temperature: Param::default(),
            tint: Param::default(),
            contrast: Param::default(),
            saturation: Param::new(10_000),
            wheel: ColorWheel::default(),
        }
    }
}

impl HslSecondary {
    pub fn params_mut(&mut self) -> impl Iterator<Item = &mut Param> {
        let [saturation_low, saturation_high] = &mut self.saturation_range;
        let [lightness_low, lightness_high] = &mut self.lightness_range;
        [
            &mut self.hue,
            &mut self.hue_width,
            saturation_low,
            saturation_high,
            lightness_low,
            lightness_high,
            &mut self.softness,
            &mut self.temperature,
            &mut self.tint,
            &mut self.contrast,
            &mut self.saturation,
        ]
        .into_iter()
        .chain(self.wheel.params_mut())
    }

    pub fn params(&self) -> impl Iterator<Item = &Param> {
        [&self.hue, &self.hue_width]
            .into_iter()
            .chain(&self.saturation_range)
            .chain(&self.lightness_range)
            .chain([
                &self.softness,
                &self.temperature,
                &self.tint,
                &self.contrast,
                &self.saturation,
            ])
            .chain(self.wheel.params())
    }
}
