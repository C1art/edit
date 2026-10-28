use timeline::{EffectCategory, MatteChannel, Rgb, VideoEffect};

use super::look::Rows;

/// Premiere's effect categories, each a submenu of the effects in it.
pub(super) fn add_menu(ui: &mut egui::Ui, rows: &mut Rows, effects: &mut Vec<VideoEffect>) {
    ui.menu_button("Add Effect", |ui| {
        for category in EffectCategory::ALL {
            ui.menu_button(category.name(), |ui| {
                let catalog = VideoEffect::catalog();
                for effect in catalog.into_iter().filter(|e| e.category() == category) {
                    if ui.button(effect.name()).clicked() {
                        effects.push(effect);
                        rows.commit();
                        ui.close();
                    }
                }
            });
        }
    });
}

impl Rows {
    pub(super) fn effect(
        &mut self,
        ui: &mut egui::Ui,
        effect: &mut VideoEffect,
        video_tracks: usize,
    ) {
        match effect {
            VideoEffect::GaussianBlur { blurriness } => {
                self.param(ui, "Blurriness", blurriness, 0.0..=500.0, "");
            }
            VideoEffect::HorizontalFlip
            | VideoEffect::VerticalFlip
            | VideoEffect::BlackAndWhite => {}
            VideoEffect::Tint {
                black,
                white,
                amount,
            } => {
                self.color(ui, "Map Black To", black);
                self.color(ui, "Map White To", white);
                self.param(ui, "Amount", amount, 0.0..=100.0, "%");
            }
            VideoEffect::Invert { amount } => {
                self.param(ui, "Blend", amount, 0.0..=100.0, "%");
            }
            VideoEffect::BrightnessContrast {
                brightness,
                contrast,
            } => {
                self.param(ui, "Brightness", brightness, -100.0..=100.0, "");
                self.param(ui, "Contrast", contrast, -100.0..=100.0, "");
            }
            VideoEffect::GammaCorrection { gamma } => {
                self.param(ui, "Gamma", gamma, 0.1..=5.0, "");
            }
            VideoEffect::Posterize { levels } => {
                self.param(ui, "Level", levels, 2.0..=64.0, "");
            }
            VideoEffect::Noise { amount } => {
                self.param(ui, "Amount", amount, 0.0..=100.0, "%");
            }
            VideoEffect::FindEdges { invert, blend } => {
                self.choice(ui, "Invert", |ui| ui.checkbox(invert, "").changed());
                self.param(ui, "Blend", blend, 0.0..=100.0, "%");
            }
            VideoEffect::Mosaic {
                horizontal,
                vertical,
            } => {
                self.param(ui, "Horizontal", horizontal, 1.0..=4_000.0, "");
                self.param(ui, "Vertical", vertical, 1.0..=4_000.0, "");
            }
            VideoEffect::Offset {
                shift: [x, y],
                blend,
            } => {
                self.param(ui, "Center X", x, -100.0..=200.0, "%");
                self.param(ui, "Center Y", y, -100.0..=200.0, "%");
                self.param(ui, "Blend", blend, 0.0..=100.0, "%");
            }
            VideoEffect::Twirl { angle, radius } => {
                self.param(ui, "Angle", angle, -3_600.0..=3_600.0, "°");
                self.param(ui, "Radius", radius, 0.0..=100.0, "%");
            }
            VideoEffect::LensDistortion { curvature } => {
                self.param(ui, "Curvature", curvature, -100.0..=100.0, "");
            }
            VideoEffect::DirectionalBlur { direction, length } => {
                self.param(ui, "Direction", direction, -360.0..=360.0, "°");
                self.param(ui, "Length", length, 0.0..=500.0, " px");
            }
            VideoEffect::DropShadow {
                color,
                opacity,
                direction,
                distance,
                softness,
            } => {
                self.color(ui, "Shadow Color", color);
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
                self.param(ui, "Direction", direction, -360.0..=360.0, "°");
                self.param(ui, "Distance", distance, 0.0..=1_000.0, " px");
                self.param(ui, "Softness", softness, 0.0..=250.0, " px");
            }
            VideoEffect::WaveWarp {
                height,
                width,
                direction,
                speed,
            } => {
                self.param(ui, "Wave Height", height, -500.0..=500.0, " px");
                self.param(ui, "Wave Width", width, 1.0..=2_000.0, " px");
                self.param(ui, "Direction", direction, -360.0..=360.0, "°");
                self.param(ui, "Wave Speed", speed, -10.0..=10.0, "/s");
            }
            VideoEffect::Sharpen { amount } => {
                self.param(ui, "Amount", amount, 0.0..=500.0, "");
            }
            VideoEffect::Vignette {
                amount,
                midpoint,
                feather,
            } => {
                self.param(ui, "Amount", amount, -100.0..=100.0, "");
                self.param(ui, "Midpoint", midpoint, 0.0..=100.0, "");
                self.param(ui, "Feather", feather, 0.0..=100.0, "");
            }
            VideoEffect::UltraKey {
                color,
                tolerance,
                softness,
            } => {
                self.color(ui, "Key Color", color);
                self.param(ui, "Tolerance", tolerance, 0.0..=100.0, "%");
                self.param(ui, "Softness", softness, 0.0..=100.0, "%");
            }
            VideoEffect::TrackMatteKey {
                matte,
                channel,
                reverse,
            } => {
                self.choice(ui, "Matte", |ui| {
                    let name = |track: Option<usize>| {
                        track.map_or("None".to_owned(), |index| format!("Video {}", index + 1))
                    };
                    let mut chosen = false;
                    egui::ComboBox::from_id_salt("matte")
                        .selected_text(name(*matte))
                        .show_ui(ui, |ui| {
                            let tracks = std::iter::once(None).chain((0..video_tracks).map(Some));
                            for track in tracks {
                                chosen |= ui.selectable_value(matte, track, name(track)).clicked();
                            }
                        });
                    chosen
                });
                self.choice(ui, "Composite Using", |ui| {
                    let mut chosen = false;
                    egui::ComboBox::from_id_salt("matte_channel")
                        .selected_text(channel.name())
                        .show_ui(ui, |ui| {
                            for option in MatteChannel::ALL {
                                chosen |= ui
                                    .selectable_value(channel, option, option.name())
                                    .clicked();
                            }
                        });
                    chosen
                });
                self.choice(ui, "Reverse", |ui| ui.checkbox(reverse, "").changed());
            }
        }
    }

    /// A labelled row of controls that return whether they made a choice.
    fn choice(&mut self, ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui) -> bool) {
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.add_sized([72.0, 18.0], egui::Label::new(label));
            if add(ui) {
                self.commit();
            }
        });
    }

    fn color(&mut self, ui: &mut egui::Ui, label: &str, color: &mut Rgb) {
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.add_sized([72.0, 18.0], egui::Label::new(label));
            let mut rgb = [color.r, color.g, color.b];
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                *color = Rgb::new(rgb[0], rgb[1], rgb[2]);
                self.commit();
            }
        });
    }
}
