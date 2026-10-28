use egui::RichText;
use timeline::{AudioEffect, Gain, Param};

use super::look::{Output, Rows};

const GAIN_DB: std::ops::RangeInclusive<f32> = -24.0..=24.0;
const HEARING_HZ: std::ops::RangeInclusive<f32> = 20.0..=20_000.0;

/// The add menu's groups, as Premiere's Effects panel files them.
const CATEGORIES: [&str; 5] = [
    "Amplitude and Compression",
    "Filter and EQ",
    "Delay and Echo",
    "Reverb",
    "Channels",
];

fn category(effect: &AudioEffect) -> &'static str {
    match effect {
        AudioEffect::Amplify { .. }
        | AudioEffect::Compressor { .. }
        | AudioEffect::HardLimiter { .. }
        | AudioEffect::NoiseGate { .. } => CATEGORIES[0],
        AudioEffect::Bass { .. }
        | AudioEffect::Treble { .. }
        | AudioEffect::Equalizer { .. }
        | AudioEffect::HighPass { .. }
        | AudioEffect::LowPass { .. }
        | AudioEffect::Notch { .. } => CATEGORIES[1],
        AudioEffect::Delay { .. } => CATEGORIES[2],
        AudioEffect::Reverb { .. } => CATEGORIES[3],
        AudioEffect::Balance { .. }
        | AudioEffect::FillLeftWithRight
        | AudioEffect::FillRightWithLeft
        | AudioEffect::SwapChannels
        | AudioEffect::Invert => CATEGORIES[4],
    }
}

/// Edits an audio clip's volume in `rows`.
pub(super) fn volume_controls(ui: &mut egui::Ui, volume: &mut Param, mut rows: Rows) -> Output {
    egui::CollapsingHeader::new("Volume")
        .default_open(true)
        .show(ui, |ui| {
            let range = Gain::SILENT.db()..=Gain::MAX.db();
            rows.param(ui, "Level", volume, range, " dB");
        });
    rows.output
}

/// Edits an audio clip's effects in `rows`.
pub(super) fn effect_controls(
    ui: &mut egui::Ui,
    effects: &mut Vec<AudioEffect>,
    mut rows: Rows,
) -> Output {
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
                for category in CATEGORIES {
                    ui.menu_button(category, |ui| {
                        let listed = AudioEffect::catalog()
                            .into_iter()
                            .filter(|effect| self::category(effect) == category);
                        for effect in listed {
                            if ui.button(effect.name()).clicked() {
                                effects.push(effect);
                                rows.commit();
                            }
                        }
                    });
                }
            });
        });
    rows.output
}

fn effect_rows(ui: &mut egui::Ui, rows: &mut Rows, effect: &mut AudioEffect) {
    match effect {
        AudioEffect::Amplify { gain } => rows.param(ui, "Gain", gain, GAIN_DB, " dB"),
        AudioEffect::Balance { balance } => rows.param(ui, "Balance", balance, -100.0..=100.0, ""),
        AudioEffect::Bass { boost } | AudioEffect::Treble { boost } => {
            rows.param(ui, "Boost", boost, GAIN_DB, " dB");
        }
        AudioEffect::Notch { frequency, q } => {
            rows.param(ui, "Center", frequency, HEARING_HZ, " Hz");
            rows.param(ui, "Q", q, 0.5..=50.0, "");
        }
        AudioEffect::HardLimiter {
            boost,
            ceiling,
            release,
        } => {
            rows.param(ui, "Input Boost", boost, 0.0..=24.0, " dB");
            rows.param(ui, "Ceiling", ceiling, -24.0..=0.0, " dB");
            rows.param(ui, "Release", release, 10.0..=1_000.0, " ms");
        }
        AudioEffect::NoiseGate {
            threshold,
            attack,
            release,
        } => {
            rows.param(ui, "Threshold", threshold, -90.0..=0.0, " dB");
            rows.param(ui, "Attack", attack, 0.1..=100.0, " ms");
            rows.param(ui, "Release", release, 10.0..=2_000.0, " ms");
        }
        AudioEffect::Reverb { size, damping, mix } => {
            rows.param(ui, "Room Size", size, 0.0..=100.0, "%");
            rows.param(ui, "Damping", damping, 0.0..=100.0, "%");
            rows.param(ui, "Mix", mix, 0.0..=100.0, "%");
        }
        AudioEffect::FillLeftWithRight
        | AudioEffect::FillRightWithLeft
        | AudioEffect::SwapChannels
        | AudioEffect::Invert => {}
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
