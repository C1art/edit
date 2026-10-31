use timeline::{ChangeColorView, MatchColors, VideoEffect};

use super::look::Rows;

const PERCENT: std::ops::RangeInclusive<f32> = 0.0..=100.0;
const SIGNED: std::ops::RangeInclusive<f32> = -100.0..=100.0;

impl Rows {
    /// The controls of Change Color and Color Balance; other effects have none here.
    pub(super) fn shift(&mut self, ui: &mut egui::Ui, effect: &mut VideoEffect) {
        match effect {
            VideoEffect::ChangeColor(change) => {
                self.options(
                    ui,
                    "View",
                    "change_color_view",
                    &mut change.view,
                    ChangeColorView::ALL,
                    ChangeColorView::name,
                );
                let hue = &mut change.hue_transform;
                self.param(ui, "Hue Transform", hue, -180.0..=180.0, "°");
                let lightness = &mut change.lightness_transform;
                self.param(ui, "Lightness Transform", lightness, SIGNED, "");
                let saturation = &mut change.saturation_transform;
                self.param(ui, "Saturation Transform", saturation, SIGNED, "");
                self.color(ui, "Color To Change", &mut change.color_to_change);
                let tolerance = &mut change.matching_tolerance;
                self.param(ui, "Matching Tolerance", tolerance, PERCENT, "%");
                let softness = &mut change.matching_softness;
                self.param(ui, "Matching Softness", softness, PERCENT, "%");
                self.options(
                    ui,
                    "Match Colors",
                    "change_color_match",
                    &mut change.match_colors,
                    MatchColors::ALL,
                    MatchColors::name,
                );
                let invert = &mut change.invert_mask;
                self.choice(ui, "Invert Color Correction Mask", |ui| {
                    ui.checkbox(invert, "").changed()
                });
            }
            VideoEffect::ColorBalance(balance) => {
                let ranges = [
                    ("Shadow", &mut balance.shadows),
                    ("Midtone", &mut balance.midtones),
                    ("Highlight", &mut balance.highlights),
                ];
                for (range, channels) in ranges {
                    for (channel, param) in ["Red", "Green", "Blue"].into_iter().zip(channels) {
                        self.param(ui, &format!("{range} {channel} Balance"), param, SIGNED, "");
                    }
                }
                let preserve = &mut balance.preserve_luminosity;
                self.choice(ui, "Preserve Luminosity", |ui| {
                    ui.checkbox(preserve, "").changed()
                });
            }
            _ => {}
        }
    }
}
