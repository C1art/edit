use egui::RichText;
use timeline::{AudioEffect, Gain, Param, Ticks};

use super::look::{Output, Rows};
use crate::mixer::Change;

const GAIN_DB: std::ops::RangeInclusive<f32> = -24.0..=24.0;
const HEARING_HZ: std::ops::RangeInclusive<f32> = 20.0..=20_000.0;

/// The add menu's groups, as Premiere's Effects panel files them.
pub(crate) const CATEGORIES: [&str; 8] = [
    "Amplitude and Compression",
    "Filter and EQ",
    "Modulation",
    "Delay and Echo",
    "Reverb",
    "Time and Pitch",
    "Channels",
    "Noise Reduction/Restoration",
];

pub(crate) fn category(effect: &AudioEffect) -> &'static str {
    match effect {
        AudioEffect::Amplify { .. }
        | AudioEffect::Compressor { .. }
        | AudioEffect::HardLimiter { .. }
        | AudioEffect::NoiseGate { .. }
        | AudioEffect::DeEsser { .. }
        | AudioEffect::ChannelVolume { .. }
        | AudioEffect::MultibandCompressor { .. } => CATEGORIES[0],
        AudioEffect::Bass { .. }
        | AudioEffect::Treble { .. }
        | AudioEffect::Equalizer { .. }
        | AudioEffect::GraphicEqualizer { .. }
        | AudioEffect::HighPass { .. }
        | AudioEffect::LowPass { .. }
        | AudioEffect::Notch { .. } => CATEGORIES[1],
        AudioEffect::Chorus { .. } | AudioEffect::Flanger { .. } => CATEGORIES[2],
        AudioEffect::Delay { .. } => CATEGORIES[3],
        AudioEffect::Reverb { .. } => CATEGORIES[4],
        AudioEffect::PitchShifter { .. } => CATEGORIES[5],
        AudioEffect::Balance { .. }
        | AudioEffect::FillLeftWithRight
        | AudioEffect::FillRightWithLeft
        | AudioEffect::SwapChannels
        | AudioEffect::Invert => CATEGORIES[6],
        AudioEffect::DeNoise { .. } | AudioEffect::DeHum { .. } => CATEGORIES[7],
    }
}

/// Edits an audio clip's volume and balance in `rows`.
pub(super) fn volume_controls(
    ui: &mut egui::Ui,
    volume: &mut Param,
    pan: &mut Param,
    mut rows: Rows,
) -> Output {
    egui::CollapsingHeader::new("Volume")
        .default_open(true)
        .show(ui, |ui| {
            let range = Gain::SILENT.db()..=Gain::MAX.db();
            rows.param(ui, "Level", volume, range, " dB");
        });
    egui::CollapsingHeader::new("Panner")
        .default_open(true)
        .show(ui, |ui| {
            rows.param(ui, "Balance", pan, -100.0..=100.0, "");
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

/// Edits a track's inserts, keyframed over sequence time at `time`. Returns the change
/// made and a keyframe to move the playhead to.
pub(crate) fn track_effect_controls(
    ui: &mut egui::Ui,
    effects: &mut Vec<AudioEffect>,
    time: Ticks,
) -> (Option<Change>, Option<Ticks>) {
    let output = effect_controls(ui, effects, Rows::new(time, None, Default::default()));
    (output.change, output.seek)
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
        AudioEffect::DeEsser {
            threshold,
            reduction,
            frequency,
        } => {
            rows.param(ui, "Threshold", threshold, -60.0..=0.0, " dB");
            rows.param(ui, "Reduction", reduction, 0.0..=24.0, " dB");
            rows.param(ui, "Frequency", frequency, 2_000.0..=12_000.0, " Hz");
        }
        AudioEffect::Chorus { rate, depth, mix } => {
            rows.param(ui, "Rate", rate, 0.05..=10.0, " Hz");
            rows.param(ui, "Depth", depth, 0.0..=20.0, " ms");
            rows.param(ui, "Mix", mix, 0.0..=100.0, "%");
        }
        AudioEffect::Flanger {
            rate,
            depth,
            feedback,
            mix,
        } => {
            rows.param(ui, "Rate", rate, 0.05..=5.0, " Hz");
            rows.param(ui, "Depth", depth, 0.0..=10.0, " ms");
            rows.param(ui, "Feedback", feedback, -95.0..=95.0, "%");
            rows.param(ui, "Mix", mix, 0.0..=100.0, "%");
        }
        AudioEffect::PitchShifter { semitones, mix } => {
            rows.param(ui, "Semitones", semitones, -24.0..=24.0, "");
            rows.param(ui, "Mix", mix, 0.0..=100.0, "%");
        }
        AudioEffect::DeNoise { amount } => {
            rows.param(ui, "Amount", amount, 0.0..=100.0, "%");
        }
        AudioEffect::DeHum {
            frequency,
            harmonics,
            reduction,
        } => {
            rows.param(ui, "Frequency", frequency, 40.0..=70.0, " Hz");
            let most = audio::MAX_HARMONICS as f32;
            rows.param(ui, "Harmonics", harmonics, 1.0..=most, "");
            rows.param(ui, "Reduction", reduction, 0.0..=60.0, " dB");
        }
        AudioEffect::ChannelVolume { left, right } => {
            rows.param(ui, "Left", left, GAIN_DB, " dB");
            rows.param(ui, "Right", right, GAIN_DB, " dB");
        }
        AudioEffect::GraphicEqualizer { bands } => {
            for (band, hz) in bands.iter_mut().zip(audio::GRAPHIC_BANDS_HZ) {
                let label = if hz < 1_000.0 {
                    format!("{hz} Hz")
                } else {
                    format!("{} kHz", hz / 1_000.0)
                };
                rows.param(ui, &label, band, GAIN_DB, " dB");
            }
        }
        AudioEffect::MultibandCompressor {
            low_crossover,
            high_crossover,
            low_threshold,
            mid_threshold,
            high_threshold,
            ratio,
            attack,
            release,
            makeup,
        } => {
            rows.param(ui, "Low/Mid", low_crossover, 40.0..=1_000.0, " Hz");
            rows.param(ui, "Mid/High", high_crossover, 1_000.0..=12_000.0, " Hz");
            rows.param(ui, "Low Threshold", low_threshold, -60.0..=0.0, " dB");
            rows.param(ui, "Mid Threshold", mid_threshold, -60.0..=0.0, " dB");
            rows.param(ui, "High Threshold", high_threshold, -60.0..=0.0, " dB");
            rows.param(ui, "Ratio", ratio, 1.0..=20.0, ":1");
            rows.param(ui, "Attack", attack, 0.1..=200.0, " ms");
            rows.param(ui, "Release", release, 10.0..=2_000.0, " ms");
            rows.param(ui, "Makeup", makeup, 0.0..=24.0, " dB");
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
