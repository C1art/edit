use timeline::{
    Framing, Look, RollingShutterRepair, ScanDirection, StabilizeMethod, Stabilizer, Timebase,
    VideoEffect,
};

use super::look::Rows;

/// Brings each stabilizer's correction in line with its settings.
pub(super) fn retune(look: &mut Look, timebase: Timebase) {
    for effect in &mut look.effects {
        if let VideoEffect::WarpStabilizer(stabilizer) = effect {
            stabilizer.retune(timebase);
        }
    }
}

impl Rows {
    /// The analysis's progress, or whether it has been done and a button to do it.
    fn motion_analysis(&mut self, ui: &mut egui::Ui, analyzed: bool) {
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            if let Some(done) = self.analyzing {
                let text = format!("Analyzing… {:.0}%", done * 100.0);
                let width = ui.available_width().min(180.0);
                ui.add(egui::ProgressBar::new(done).desired_width(width).text(text));
                return;
            }
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
    }

    pub(super) fn stabilizer(&mut self, ui: &mut egui::Ui, stabilizer: &mut Stabilizer) {
        self.motion_analysis(ui, stabilizer.path.is_some());
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
            self.whole(ui, "Smoothness", &mut stabilizer.smoothness, 0..=100, "%");
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

    pub(super) fn rolling_shutter(&mut self, ui: &mut egui::Ui, repair: &mut RollingShutterRepair) {
        self.motion_analysis(ui, repair.path.is_some());
        self.whole(ui, "Rate", &mut repair.rate, 0..=100, "%");
        let scan = &mut repair.scan;
        let (all, name) = (ScanDirection::ALL, ScanDirection::name);
        self.options(
            ui,
            "Scan Direction",
            "rolling_shutter_scan",
            scan,
            all,
            name,
        );
    }
}
