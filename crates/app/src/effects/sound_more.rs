use timeline::AudioEffect;

use super::look::Rows;

const HEARING_HZ: std::ops::RangeInclusive<f32> = 20.0..=20_000.0;

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
        _ => {}
    }
}
