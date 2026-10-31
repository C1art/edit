//! The export dialog's choice to bring the sound to a loudness, as Premiere's Loudness
//! Normalization.

use egui::DragValue;
use playback::LoudnessTarget;

use crate::loudness_window::TARGETS;

/// What a new choice starts at: streaming services' level, peaks kept a decibel clear
/// of clipping for lossy encoders.
const DEFAULT: LoudnessTarget = LoudnessTarget {
    lufs: -14,
    peak_db: -1,
};

/// The Loudness row of the export settings grid.
pub fn row(ui: &mut egui::Ui, loudness: &mut Option<LoudnessTarget>) {
    ui.label("Loudness:");
    ui.horizontal(|ui| {
        let mut on = loudness.is_some();
        ui.checkbox(&mut on, "Normalize to").on_hover_text(
            "Measure the whole export first, then turn the sound up or down to this \
             integrated loudness, never letting a peak pass the ceiling",
        );
        *loudness = on.then(|| loudness.unwrap_or(DEFAULT));
        let Some(target) = loudness else {
            return;
        };
        ui.add(
            DragValue::new(&mut target.lufs)
                .range(-40..=-5)
                .suffix(" LUFS"),
        );
        ui.menu_button("⏷", |ui| {
            for (name, lufs) in TARGETS {
                let lufs = lufs as i32;
                if ui.button(format!("{name}  ({lufs} LUFS)")).clicked() {
                    target.lufs = lufs;
                    ui.close();
                }
            }
        });
        ui.label("peaks at most");
        ui.add(
            DragValue::new(&mut target.peak_db)
                .range(-12..=0)
                .suffix(" dBFS"),
        );
    });
    ui.end_row();
}
