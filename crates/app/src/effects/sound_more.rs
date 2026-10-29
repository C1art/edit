use timeline::{AudioEffect, FilterMode, FilterType, VocalMode};

use super::look::Rows;

const HEARING_HZ: std::ops::RangeInclusive<f32> = 20.0..=20_000.0;
const LEVEL_DB: std::ops::RangeInclusive<f32> = -80.0..=0.0;

/// The rows of the audio effects added after the first set.
pub(super) fn effect_rows(ui: &mut egui::Ui, rows: &mut Rows, effect: &mut AudioEffect) {
    match effect {
        AudioEffect::BandPass { frequency, q } => {
            rows.param(ui, "Center", frequency, HEARING_HZ, " Hz");
            rows.param(ui, "Q", q, 0.1..=20.0, "");
        }
        AudioEffect::Phaser {
            stages,
            frequency,
            depth,
            rate,
            feedback,
            mix,
        } => {
            let most = audio::MAX_PHASER_STAGES as f32;
            rows.param(ui, "Stages", stages, 1.0..=most, "");
            rows.param(ui, "Frequency", frequency, 100.0..=5_000.0, " Hz");
            rows.param(ui, "Depth", depth, 0.0..=100.0, "%");
            rows.param(ui, "Mod Rate", rate, 0.01..=10.0, " Hz");
            rows.param(ui, "Feedback", feedback, -95.0..=95.0, "%");
            rows.param(ui, "Mix", mix, 0.0..=100.0, "%");
        }
        AudioEffect::Distortion { drive, mix } => {
            rows.param(ui, "Drive", drive, 0.0..=40.0, " dB");
            rows.param(ui, "Mix", mix, 0.0..=100.0, "%");
        }
        AudioEffect::StereoExpander { width } => {
            rows.param(ui, "Stereo Expand", width, 0.0..=300.0, "%");
        }
        AudioEffect::MultitapDelay { times, levels } => {
            for (index, (time, level)) in times.iter_mut().zip(levels).enumerate() {
                let tap = index + 1;
                rows.param(ui, &format!("Tap {tap} Delay"), time, 1.0..=2_000.0, " ms");
                rows.param(ui, &format!("Tap {tap} Level"), level, 0.0..=100.0, "%");
            }
        }
        AudioEffect::AnalogDelay {
            time,
            feedback,
            saturation,
            tone,
            mix,
        } => {
            rows.param(ui, "Delay", time, 1.0..=2_000.0, " ms");
            rows.param(ui, "Feedback", feedback, 0.0..=95.0, "%");
            rows.param(ui, "Trash", saturation, 0.0..=100.0, "%");
            rows.param(ui, "Tone", tone, 200.0..=12_000.0, " Hz");
            rows.param(ui, "Mix", mix, 0.0..=100.0, "%");
        }
        AudioEffect::Dynamics(dynamics) => {
            if let Some(gate) = section(ui, rows, "AutoGate", &mut dynamics.gate) {
                rows.param(ui, "Threshold", &mut gate.threshold, LEVEL_DB, " dB");
                rows.param(ui, "Attack", &mut gate.attack, 0.1..=100.0, " ms");
                rows.param(ui, "Release", &mut gate.release, 10.0..=1_000.0, " ms");
            }
            if let Some(expander) = section(ui, rows, "Expander", &mut dynamics.expander) {
                rows.param(ui, "Threshold", &mut expander.threshold, LEVEL_DB, " dB");
                rows.param(ui, "Ratio", &mut expander.ratio, 1.0..=10.0, ":1");
            }
            if let Some(compressor) = section(ui, rows, "Compressor", &mut dynamics.compressor) {
                rows.param(ui, "Threshold", &mut compressor.threshold, LEVEL_DB, " dB");
                rows.param(ui, "Ratio", &mut compressor.ratio, 1.0..=20.0, ":1");
                rows.param(ui, "Attack", &mut compressor.attack, 0.1..=200.0, " ms");
                rows.param(
                    ui,
                    "Release",
                    &mut compressor.release,
                    10.0..=2_000.0,
                    " ms",
                );
                rows.param(ui, "Makeup", &mut compressor.makeup, 0.0..=24.0, " dB");
            }
            if let Some(limiter) = section(ui, rows, "Limiter", &mut dynamics.limiter) {
                rows.param(ui, "Threshold", &mut limiter.threshold, -20.0..=0.0, " dB");
                rows.param(ui, "Release", &mut limiter.release, 10.0..=1_000.0, " ms");
            }
        }
        AudioEffect::AutomaticClickRemover { threshold } => {
            rows.param(ui, "Threshold", threshold, 0.0..=100.0, "");
        }
        AudioEffect::VocalEnhancer { mode } => {
            let all = VocalMode::ALL;
            rows.options(ui, "Mode", "vocal_mode", mode, all, VocalMode::name);
        }
        AudioEffect::TubeCompressor {
            threshold,
            ratio,
            attack,
            release,
            makeup,
            warmth,
        } => {
            rows.param(ui, "Threshold", threshold, LEVEL_DB, " dB");
            rows.param(ui, "Ratio", ratio, 1.0..=20.0, ":1");
            rows.param(ui, "Attack", attack, 0.1..=200.0, " ms");
            rows.param(ui, "Release", release, 10.0..=2_000.0, " ms");
            rows.param(ui, "Makeup", makeup, 0.0..=24.0, " dB");
            rows.param(ui, "Warmth", warmth, 0.0..=100.0, "%");
        }
        AudioEffect::Mastering {
            low,
            mid,
            high,
            reverb,
            exciter,
            width,
            maximizer,
            output,
        } => {
            rows.param(ui, "Low", low, -12.0..=12.0, " dB");
            rows.param(ui, "Mid", mid, -12.0..=12.0, " dB");
            rows.param(ui, "High", high, -12.0..=12.0, " dB");
            rows.param(ui, "Reverb", reverb, 0.0..=100.0, "%");
            rows.param(ui, "Exciter", exciter, 0.0..=100.0, "%");
            rows.param(ui, "Widener", width, 0.0..=300.0, "%");
            rows.param(ui, "Loudness Maximizer", maximizer, 0.0..=100.0, "%");
            rows.param(ui, "Output Gain", output, -24.0..=12.0, " dB");
        }
        AudioEffect::DeReverb { amount, gain } => {
            rows.param(ui, "Amount", amount, 0.0..=100.0, "%");
            rows.param(ui, "Gain", gain, -12.0..=12.0, " dB");
        }
        AudioEffect::SpeechVolumeLeveler {
            target,
            amount,
            noise_threshold,
        } => {
            rows.param(ui, "Target Volume Level", target, -40.0..=0.0, " dB");
            rows.param(ui, "Leveling Amount", amount, 0.0..=100.0, "%");
            rows.param(ui, "Noise Threshold", noise_threshold, LEVEL_DB, " dB");
        }
        AudioEffect::SimpleParametricEq { frequency, gain, q } => {
            rows.param(ui, "Center", frequency, HEARING_HZ, " Hz");
            rows.param(ui, "Gain", gain, -24.0..=24.0, " dB");
            rows.param(ui, "Q", q, 0.1..=20.0, "");
        }
        AudioEffect::ScientificFilter {
            filter_type,
            mode,
            order,
            cutoff,
            high_cutoff,
            ripple,
            gain,
        } => {
            let (all, name) = (FilterType::ALL, FilterType::name);
            rows.options(ui, "Type", "scientific_type", filter_type, all, name);
            let (all, name) = (FilterMode::ALL, FilterMode::name);
            rows.options(ui, "Mode", "scientific_mode", mode, all, name);
            rows.whole(ui, "Order", order, 1..=8, "");
            match mode.is_band() {
                true => {
                    rows.param(ui, "Low Cutoff", cutoff, HEARING_HZ, " Hz");
                    rows.param(ui, "High Cutoff", high_cutoff, HEARING_HZ, " Hz");
                }
                false => rows.param(ui, "Cutoff", cutoff, HEARING_HZ, " Hz"),
            }
            if *filter_type == FilterType::Chebyshev {
                rows.param(ui, "Passband Ripple", ripple, 0.01..=6.0, " dB");
            }
            rows.param(ui, "Gain", gain, -24.0..=24.0, " dB");
        }
        _ => {}
    }
}

/// A checkbox that turns one of Dynamics' sections on, with its defaults, or off.
fn section<'a, T: Default>(
    ui: &mut egui::Ui,
    rows: &mut Rows,
    label: &str,
    section: &'a mut Option<T>,
) -> Option<&'a mut T> {
    let mut on = section.is_some();
    rows.choice(ui, label, |ui| ui.checkbox(&mut on, "").changed());
    match (on, section.is_some()) {
        (true, false) => *section = Some(T::default()),
        (false, true) => *section = None,
        _ => {}
    }
    section.as_mut()
}
