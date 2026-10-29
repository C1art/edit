use timeline::{AutoReframe, Look, ReframeMotion, Timebase, VideoEffect};

use super::look::Rows;

/// Brings each Auto Reframe's framing in line with its motion preset.
pub(super) fn retune(look: &mut Look, timebase: Timebase) {
    for effect in &mut look.effects {
        if let VideoEffect::AutoReframe(reframe) = effect {
            reframe.retune(timebase);
        }
    }
}

impl Rows {
    pub(super) fn auto_reframe(&mut self, ui: &mut egui::Ui, reframe: &mut AutoReframe) {
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            if let Some(done) = self.reframing {
                let text = format!("Analyzing… {:.0}%", done * 100.0);
                ui.add(egui::ProgressBar::new(done).desired_width(180.0).text(text));
                return;
            }
            let analyzed = reframe.subject.is_some();
            ui.label(if analyzed { "Analyzed" } else { "Not analyzed" });
            let button = if analyzed { "Reanalyze" } else { "Analyze" };
            if ui
                .button(button)
                .on_hover_text("Find the subject the frame follows through the footage")
                .clicked()
            {
                self.output.analyze_reframe = true;
            }
        });
        self.options(
            ui,
            "Motion Preset",
            "reframe_motion",
            &mut reframe.motion,
            ReframeMotion::ALL,
            ReframeMotion::name,
        );
        let [x, y] = &mut reframe.offset;
        self.param(ui, "Offset X", x, -4000.0..=4000.0, " px");
        self.param(ui, "Offset Y", y, -4000.0..=4000.0, " px");
        self.param(ui, "Reframe Scale", &mut reframe.scale, 0.0..=1000.0, "%");
    }
}
