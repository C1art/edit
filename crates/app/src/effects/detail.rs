use timeline::VideoEffect;

use super::look::Rows;

const LEVELS: std::ops::RangeInclusive<f32> = 0.0..=255.0;

impl Rows {
    /// The controls of the detail effects; other effects have none here.
    pub(super) fn detail(&mut self, ui: &mut egui::Ui, effect: &mut VideoEffect) {
        match effect {
            VideoEffect::UnsharpMask(unsharp) => {
                self.param(ui, "Amount", &mut unsharp.amount, 0.0..=500.0, "%");
                self.param(ui, "Radius", &mut unsharp.radius, 0.1..=250.0, "px");
                self.param(ui, "Threshold", &mut unsharp.threshold, LEVELS, "");
            }
            VideoEffect::Median(median) => {
                self.param(ui, "Radius", &mut median.radius, 0.0..=100.0, "px");
            }
            VideoEffect::DustAndScratches(median) => {
                self.param(ui, "Radius", &mut median.radius, 0.0..=100.0, "px");
                self.param(ui, "Threshold", &mut median.threshold, LEVELS, "");
            }
            VideoEffect::EdgeFeather(feather) => {
                self.param(ui, "Amount", &mut feather.amount, 0.0..=500.0, "px");
            }
            VideoEffect::NoiseHls(noise) => {
                self.param(ui, "Hue", &mut noise.hue, 0.0..=100.0, "%");
                self.param(ui, "Lightness", &mut noise.lightness, 0.0..=100.0, "%");
                self.param(ui, "Saturation", &mut noise.saturation, 0.0..=100.0, "%");
                self.param(ui, "Grain Size", &mut noise.grain_size, 1.0..=100.0, "px");
                self.param(ui, "Noise Phase", &mut noise.phase, -3600.0..=3600.0, "°");
            }
            _ => {}
        }
    }
}
