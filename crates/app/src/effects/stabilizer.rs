use egui::DragValue;
use timeline::{Framing, Look, StabilizeMethod, Stabilizer, Timebase, VideoEffect};

use super::look::Rows;
use crate::mixer::response_state;

/// Brings each stabilizer's correction in line with its settings.
pub(super) fn retune(look: &mut Look, timebase: Timebase) {
    for effect in &mut look.effects {
        if let VideoEffect::WarpStabilizer(stabilizer) = effect {
            stabilizer.retune(timebase);
        }
    }
}

impl Rows {
    pub(super) fn stabilizer(&mut self, ui: &mut egui::Ui, stabilizer: &mut Stabilizer) {
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            if let Some(done) = self.analyzing {
                let text = format!("Analyzing… {:.0}%", done * 100.0);
                ui.add(egui::ProgressBar::new(done).desired_width(180.0).text(text));
                return;
            }
            let analyzed = stabilizer.path.is_some();
            ui.label(if analyzed { "Analyzed" } else { "Not analyzed" });
            let button = if analyzed { "Reanalyze" } else { "Analyze" };
            if ui
                .button(button)
                .on_hover_text("Track the camera's motion through the clip's footage")
                .clicked()
            {
                self.output.analyze_motion = true;
            }
        });
        self.choice(ui, "Result", |ui| {
            let mut chosen = false;
            egui::ComboBox::from_id_salt("stabilize_method")
                .selected_text(stabilizer.method.name())
                .show_ui(ui, |ui| {
                    for method in StabilizeMethod::ALL {
                        chosen |= ui
                            .selectable_value(&mut stabilizer.method, method, method.name())
                            .clicked();
                    }
                });
            chosen
        });
        if stabilizer.method == StabilizeMethod::SmoothMotion {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.add_sized([72.0, 18.0], egui::Label::new("Smoothness"));
                let response = ui.add(
                    DragValue::new(&mut stabilizer.smoothness)
                        .range(0..=100)
                        .suffix("%"),
                );
                self.output.change = response_state(&response, self.output.change);
            });
        }
        self.choice(ui, "Framing", |ui| {
            let mut chosen = false;
            egui::ComboBox::from_id_salt("stabilize_framing")
                .selected_text(stabilizer.framing.name())
                .show_ui(ui, |ui| {
                    for framing in Framing::ALL {
                        chosen |= ui
                            .selectable_value(&mut stabilizer.framing, framing, framing.name())
                            .clicked();
                    }
                });
            chosen
        });
        if stabilizer.path.is_some() && stabilizer.framing == Framing::CropAutoScale {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                let percent = f64::from(stabilizer.auto_scale) / 10.0;
                ui.weak(format!("Auto-scale enlarges the picture to {percent:.1}%"));
            });
        }
    }
}
