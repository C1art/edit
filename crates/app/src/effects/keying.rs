use timeline::{KeyOutput, MatteChannel, VideoEffect};

use super::look::Rows;

impl Rows {
    /// The controls of the Keying category's effects; other effects have none here.
    pub(super) fn keying(
        &mut self,
        ui: &mut egui::Ui,
        effect: &mut VideoEffect,
        video_tracks: usize,
    ) {
        let set_matte = matches!(effect, VideoEffect::SetMatte { .. });
        match effect {
            VideoEffect::UltraKey {
                color,
                tolerance,
                softness,
                output,
                pedestal,
                contrast,
                choke,
                soften,
                spill,
            } => {
                self.options(
                    ui,
                    "Output",
                    "key_output",
                    output,
                    KeyOutput::ALL,
                    KeyOutput::name,
                );
                self.color(ui, "Key Color", color);
                self.param(ui, "Tolerance", tolerance, 0.0..=100.0, "%");
                self.param(ui, "Softness", softness, 0.0..=100.0, "%");
                self.param(ui, "Pedestal", pedestal, 0.0..=100.0, "%");
                self.param(ui, "Choke", choke, 0.0..=100.0, "%");
                self.param(ui, "Soften", soften, 0.0..=100.0, "%");
                self.param(ui, "Contrast", contrast, 0.0..=100.0, "%");
                self.param(ui, "Spill", spill, 0.0..=100.0, "%");
            }
            VideoEffect::TrackMatteKey {
                matte,
                channel,
                reverse,
            }
            | VideoEffect::SetMatte {
                matte,
                channel,
                reverse,
            } => {
                let (channels, reverse_label) = match set_matte {
                    true => (MatteChannel::ALL.as_slice(), "Invert Matte"),
                    false => (MatteChannel::TRACK_MATTE.as_slice(), "Reverse"),
                };
                self.track_choice(ui, "Matte", matte, video_tracks);
                self.options(
                    ui,
                    "Composite Using",
                    "matte_channel",
                    channel,
                    channels.iter().copied(),
                    MatteChannel::name,
                );
                self.choice(ui, reverse_label, |ui| ui.checkbox(reverse, "").changed());
            }
            VideoEffect::ColorKey {
                color,
                tolerance,
                softness,
            } => {
                self.color(ui, "Key Color", color);
                self.param(ui, "Tolerance", tolerance, 0.0..=100.0, "%");
                self.param(ui, "Softness", softness, 0.0..=100.0, "%");
            }
            VideoEffect::LumaKey { threshold, cutoff } => {
                self.param(ui, "Threshold", threshold, 0.0..=100.0, "%");
                self.param(ui, "Cutoff", cutoff, 0.0..=100.0, "%");
            }
            VideoEffect::AlphaAdjust {
                opacity,
                ignore_alpha,
                invert_alpha,
                mask_only,
            } => {
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
                self.choice(ui, "Ignore Alpha", |ui| {
                    ui.checkbox(ignore_alpha, "").changed()
                });
                self.choice(ui, "Invert Alpha", |ui| {
                    ui.checkbox(invert_alpha, "").changed()
                });
                self.choice(ui, "Mask Only", |ui| ui.checkbox(mask_only, "").changed());
            }
            VideoEffect::GarbageMatte(matte) => {
                for (number, [x, y]) in (1..).zip(&mut matte.points) {
                    self.param(ui, &format!("Point {number} X"), x, -100.0..=200.0, "%");
                    self.param(ui, &format!("Point {number} Y"), y, -100.0..=200.0, "%");
                }
            }
            _ => {}
        }
    }
}
