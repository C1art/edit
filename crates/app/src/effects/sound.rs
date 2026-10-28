use egui::RichText;
use timeline::{AudioEffect, Gain, Param, Ticks};

use super::look::{Output, Rows};

const GAIN_DB: std::ops::RangeInclusive<f32> = -24.0..=24.0;
const HEARING_HZ: std::ops::RangeInclusive<f32> = 20.0..=20_000.0;

/// Edits an audio clip's volume at source time `time`.
pub(super) fn volume_controls(ui: &mut egui::Ui, volume: &mut Param, time: Ticks) -> Output {
    let mut rows = Rows::new(time);
    egui::CollapsingHeader::new("Volume")
        .default_open(true)
        .show(ui, |ui| {
            let range = Gain::SILENT.db()..=Gain::MAX.db();
            rows.param(ui, "Level", volume, range, " dB");
        });
    rows.output
}

/// Edits an audio clip's effects at source time `time`.
pub(super) fn effect_controls(
    ui: &mut egui::Ui,
    effects: &mut Vec<AudioEffect>,
    time: Ticks,
) -> Output {
    let mut rows = Rows::new(time);
    egui::CollapsingHeader::new("Audio Effects")
        .default_open(true)
        .show(ui, |ui| {
            let mut removed = None;
            for (index, effect) in effects.iter_mut().enumerate() {
                ui.push_id(index, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(effect.name()).strong());
                        if ui.small_button("Remove").clicked() {
                            removed = Some(index);
                        }
                    });
                    effect_rows(ui, &mut rows, effect);
                });
            }
            if let Some(index) = removed {
                effects.remove(index);
                rows.commit();
            }
            ui.menu_button("Add Audio Effect", |ui| {
                for effect in AudioEffect::catalog() {
                    if ui.button(effect.name()).clicked() {
                        effects.push(effect);
                        rows.commit();
                    }
                }
            });
        });
    rows.output
}

fn effect_rows(ui: &mut egui::Ui, rows: &mut Rows, effect: &mut AudioEffect) {
    match effect {
        AudioEffect::Equalizer {
            low,
            mid,
            mid_frequency,
            high,
        } => {
            rows.param(ui, "Low", low, GAIN_DB, " dB");
            rows.param(ui, "Mid", mid, GAIN_DB, " dB");
            rows.param(ui, "Mid Freq", mid_frequency, HEARING_HZ, " Hz");
            rows.param(ui, "High", high, GAIN_DB, " dB");
        }
        AudioEffect::HighPass { frequency } | AudioEffect::LowPass { frequency } => {
            rows.param(ui, "Cutoff", frequency, HEARING_HZ, " Hz");
        }
        AudioEffect::Compressor {
            threshold,
            ratio,
            attack,
            release,
            makeup,
        } => {
            rows.param(ui, "Threshold", threshold, -60.0..=0.0, " dB");
            rows.param(ui, "Ratio", ratio, 1.0..=20.0, ":1");
            rows.param(ui, "Attack", attack, 0.1..=200.0, " ms");
            rows.param(ui, "Release", release, 10.0..=2_000.0, " ms");
            rows.param(ui, "Makeup", makeup, 0.0..=24.0, " dB");
        }
        AudioEffect::Delay {
            time,
            feedback,
            mix,
        } => {
            rows.param(ui, "Delay", time, 1.0..=2_000.0, " ms");
            rows.param(ui, "Feedback", feedback, 0.0..=95.0, "%");
            rows.param(ui, "Mix", mix, 0.0..=100.0, "%");
        }
    }
}
