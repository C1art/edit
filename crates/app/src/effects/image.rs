use timeline::{CineonConversion, VideoEffect};

use super::look::Rows;

const CODES: std::ops::RangeInclusive<f32> = 0.0..=1023.0;

impl Rows {
    /// The controls of Color Pass, Reduce Interlace Flicker and Cineon Converter;
    /// other effects have none here.
    pub(super) fn image(&mut self, ui: &mut egui::Ui, effect: &mut VideoEffect) {
        match effect {
            VideoEffect::ColorPass(pass) => {
                self.color(ui, "Color", &mut pass.color);
                self.param(ui, "Similarity", &mut pass.similarity, 0.0..=100.0, "%");
                let reverse = &mut pass.reverse;
                self.choice(ui, "Reverse", |ui| ui.checkbox(reverse, "").changed());
            }
            VideoEffect::ReduceInterlaceFlicker(flicker) => {
                self.param(ui, "Softness", &mut flicker.softness, 0.0..=10.0, "");
            }
            VideoEffect::CineonConverter(cineon) => {
                self.options(
                    ui,
                    "Conversion Type",
                    "cineon_conversion",
                    &mut cineon.conversion,
                    CineonConversion::ALL,
                    CineonConversion::name,
                );
                let percent = 0.0..=100.0;
                self.param(
                    ui,
                    "10 Bit Black Point",
                    &mut cineon.black_point_10_bit,
                    CODES,
                    "",
                );
                let black = &mut cineon.internal_black_point;
                self.param(ui, "Internal Black Point", black, percent.clone(), "%");
                self.param(
                    ui,
                    "10 Bit White Point",
                    &mut cineon.white_point_10_bit,
                    CODES,
                    "",
                );
                let white = &mut cineon.internal_white_point;
                self.param(ui, "Internal White Point", white, percent.clone(), "%");
                self.param(ui, "Gamma", &mut cineon.gamma, 0.1..=5.0, "");
                let rolloff = &mut cineon.highlight_rolloff;
                self.param(ui, "Highlight Rolloff", rolloff, percent, "%");
            }
            _ => {}
        }
    }
}
