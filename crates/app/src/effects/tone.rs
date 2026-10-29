use timeline::{LimiterMethod, Rgb, VideoEffect};

use super::look::Rows;

const LEVEL: std::ops::RangeInclusive<f32> = 0.0..=255.0;
const PERCENT: std::ops::RangeInclusive<f32> = 0.0..=100.0;

impl Rows {
    /// The controls of the tone effects; other effects have none here.
    pub(super) fn tone(&mut self, ui: &mut egui::Ui, effect: &mut VideoEffect) {
        match effect {
            VideoEffect::ConvolutionKernel(convolution) => {
                for (index, weight) in convolution.kernel.iter_mut().enumerate() {
                    let label = format!("M{}{}", index / 3 + 1, index % 3 + 1);
                    self.param(ui, &label, weight, -999.0..=999.0, "");
                }
                self.param(ui, "Offset", &mut convolution.offset, -999.0..=999.0, "");
                self.param(ui, "Scale", &mut convolution.scale, -999.0..=999.0, "");
            }
            VideoEffect::Extract(extract) => {
                let black = &mut extract.black_input_level;
                self.param(ui, "Black Input Level", black, LEVEL, "");
                let white = &mut extract.white_input_level;
                self.param(ui, "White Input Level", white, LEVEL, "");
                self.param(ui, "Softness", &mut extract.softness, LEVEL, "");
                let invert = &mut extract.invert;
                self.choice(ui, "Invert", |ui| ui.checkbox(invert, "").changed());
            }
            VideoEffect::ShadowHighlight(lift) => {
                self.param(ui, "Shadow Amount", &mut lift.shadow_amount, PERCENT, "%");
                let highlight = &mut lift.highlight_amount;
                self.param(ui, "Highlight Amount", highlight, PERCENT, "%");
                let width = &mut lift.shadow_tonal_width;
                self.param(ui, "Shadow Tonal Width", width, PERCENT, "%");
                let width = &mut lift.highlight_tonal_width;
                self.param(ui, "Highlight Tonal Width", width, PERCENT, "%");
                self.param(ui, "Radius", &mut lift.radius, 0.0..=200.0, "px");
                let contrast = &mut lift.midtone_contrast;
                self.param(ui, "Midtone Contrast", contrast, -100.0..=100.0, "%");
                let blend = &mut lift.blend_with_original;
                self.param(ui, "Blend With Original", blend, PERCENT, "%");
            }
            VideoEffect::VideoLimiter(limiter) => {
                self.param(ui, "Clip Level", &mut limiter.clip_level, 90.0..=110.0, "%");
                self.options(
                    ui,
                    "Clip Method",
                    "limiter_method",
                    &mut limiter.method,
                    LimiterMethod::ALL,
                    LimiterMethod::name,
                );
                let knee = &mut limiter.compression_before_clipping;
                self.param(ui, "Compression Before Clipping", knee, 0.0..=20.0, "%");
                let mut warn = limiter.gamut_warning.is_some();
                self.choice(ui, "Gamut Warning", |ui| {
                    ui.checkbox(&mut warn, "").changed()
                });
                match (warn, &mut limiter.gamut_warning) {
                    (true, Some(color)) => self.color(ui, "Gamut Warning Color", color),
                    (true, color) => *color = Some(Rgb::new(255, 0, 0)),
                    (false, color) => *color = None,
                }
            }
            VideoEffect::AscCdl(cdl) => {
                for (channel, ((slope, offset), power)) in ["Red", "Green", "Blue"].into_iter().zip(
                    cdl.slope
                        .iter_mut()
                        .zip(&mut cdl.offset)
                        .zip(&mut cdl.power),
                ) {
                    self.param(ui, &format!("{channel} Slope"), slope, 0.0..=10.0, "");
                    self.param(ui, &format!("{channel} Offset"), offset, -1.0..=1.0, "");
                    self.param(ui, &format!("{channel} Power"), power, 0.0..=10.0, "");
                }
                self.param(ui, "Saturation", &mut cdl.saturation, 0.0..=10.0, "");
            }
            VideoEffect::ColorReplace(replace) => {
                self.param(ui, "Similarity", &mut replace.similarity, PERCENT, "%");
                let solid = &mut replace.solid_colors;
                self.choice(ui, "Solid Colors", |ui| ui.checkbox(solid, "").changed());
                self.color(ui, "Target Color", &mut replace.target_color);
                self.color(ui, "Replace Color", &mut replace.replace_color);
            }
            _ => {}
        }
    }
}
