use std::time::Duration;

use egui::{Key, ProgressBar};
use playback::{clip_alone, Loudness, LoudnessEvent, LoudnessJob};
use timeline::{ClipId, Gain, NormalizeClipGains, SetClipGain, TrackKind};

use crate::editor::Editor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GainMode {
    Set,
    /// Each clip's loudest sample reaches the target.
    MaxPeak,
    /// The loudest sample of all the clips reaches the target, every clip set alike.
    AllPeaks,
    /// Each clip's integrated loudness reaches the target.
    Loudness,
}

impl GainMode {
    const ALL: [(Self, &'static str); 4] = [
        (Self::Set, "Set Gain to"),
        (Self::MaxPeak, "Normalize Max Peak to"),
        (Self::AllPeaks, "Normalize All Peaks to"),
        (Self::Loudness, "Normalize Loudness to"),
    ];
}

/// The clip gain being typed or chosen, and the clips being measured to normalize them.
pub(crate) struct GainDialog {
    mode: GainMode,
    gain_db: f32,
    peak_db: f32,
    loudness_lufs: f32,
    measuring: Option<Measuring>,
}

struct Measuring {
    job: LoudnessJob,
    clips: Vec<ClipId>,
    progress: f32,
}

impl GainDialog {
    pub(crate) fn new(gain_db: f32) -> Self {
        Self {
            mode: GainMode::Set,
            gain_db,
            peak_db: -1.0,
            loudness_lufs: -23.0,
            measuring: None,
        }
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(dialog) = &mut editor.gain_dialog else {
        return;
    };
    let mut measured = None;
    let mut warnings = Vec::new();
    if let Some(measuring) = &mut dialog.measuring {
        for event in measuring.job.poll() {
            match event {
                LoudnessEvent::Progress(progress) => measuring.progress = progress,
                LoudnessEvent::Warning(warning) => warnings.push(warning),
                LoudnessEvent::Measured(results) => measured = Some(results),
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    let mut apply = false;
    let mut cancelled = false;
    crate::dialog::window(ctx, "Audio Gain").show(ctx, |ui| {
        let measuring = dialog.measuring.as_ref();
        ui.add_enabled_ui(measuring.is_none(), |ui| {
            egui::Grid::new("gain_modes").num_columns(2).show(ui, |ui| {
                for (mode, label) in GainMode::ALL {
                    ui.radio_value(&mut dialog.mode, mode, label);
                    // Both peak modes share one target.
                    let (value, range, suffix) = match mode {
                        GainMode::Set => (
                            &mut dialog.gain_db,
                            Gain::SILENT.db()..=Gain::MAX.db(),
                            " dB",
                        ),
                        GainMode::MaxPeak | GainMode::AllPeaks => {
                            (&mut dialog.peak_db, -60.0..=0.0, " dBFS")
                        }
                        GainMode::Loudness => (&mut dialog.loudness_lufs, -60.0..=0.0, " LUFS"),
                    };
                    let field = ui.add_enabled(
                        dialog.mode == mode,
                        egui::DragValue::new(value)
                            .range(range)
                            .speed(0.1)
                            .suffix(suffix),
                    );
                    if field.gained_focus() {
                        dialog.mode = mode;
                    }
                    ui.end_row();
                }
            });
        });
        if let Some(measuring) = measuring {
            ui.add(
                ProgressBar::new(measuring.progress)
                    .show_percentage()
                    .text("Measuring…"),
            );
        }
        crate::dialog::footer(ui, |ui| {
            cancelled =
                ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape));
            apply = measuring.is_none()
                && (ui.add(crate::dialog::primary("OK")).clicked()
                    || ui.input(|input| input.key_pressed(Key::Enter)));
        });
    });
    for warning in warnings {
        editor.notice(format!("Measured as silence: {warning}"));
    }
    if cancelled {
        editor.gain_dialog = None;
    } else if let Some(results) = measured {
        normalize(editor, &results);
    } else if apply {
        start(editor);
    }
}

fn start(editor: &mut Editor) {
    let Some(dialog) = editor.gain_dialog.as_mut() else {
        return;
    };
    let sequence = editor.timeline.sequence();
    let clips = editor.view.selection.clone();
    if dialog.mode == GainMode::Set {
        let gain = Gain::from_db(dialog.gain_db);
        editor.gain_dialog = None;
        editor.apply_levels(SetClipGain { clips, gain });
        return;
    }
    let audio: Vec<ClipId> = sequence
        .with_links(&clips)
        .into_iter()
        .filter(|&id| {
            sequence
                .clip(id)
                .is_some_and(|(track, _)| track.kind == TrackKind::Audio)
        })
        .collect();
    let pieces = audio
        .iter()
        .filter_map(|&id| clip_alone(sequence, id))
        .collect();
    match LoudnessJob::start(pieces) {
        Ok(job) => {
            dialog.measuring = Some(Measuring {
                job,
                clips: audio,
                progress: 0.0,
            });
        }
        Err(error) => editor.notice(format!("Cannot measure: {error}")),
    }
}

/// Sets each measured clip's gain so it reaches the dialog's target; silent clips are
/// left as they are.
fn normalize(editor: &mut Editor, results: &[Loudness]) {
    let Some(dialog) = editor.gain_dialog.take() else {
        return;
    };
    let Some(measuring) = dialog.measuring else {
        return;
    };
    let measured = measuring.clips.into_iter().zip(results);
    let gains: Vec<(ClipId, Gain)> = match dialog.mode {
        GainMode::Set => Vec::new(),
        GainMode::MaxPeak => measured
            .filter(|(_, result)| result.peak > 0.0)
            .map(|(clip, result)| (clip, Gain::from_db(dialog.peak_db - db(result.peak))))
            .collect(),
        GainMode::AllPeaks => {
            let measured: Vec<_> = measured.collect();
            let loudest = measured
                .iter()
                .map(|(_, result)| result.peak)
                .fold(0.0, f32::max);
            let gain = Gain::from_db(dialog.peak_db - db(loudest));
            measured
                .into_iter()
                .filter(|_| loudest > 0.0)
                .map(|(clip, _)| (clip, gain))
                .collect()
        }
        GainMode::Loudness => measured
            .filter_map(|(clip, result)| {
                let integrated = result.integrated?;
                Some((clip, Gain::from_db(dialog.loudness_lufs - integrated)))
            })
            .collect(),
    };
    if gains.is_empty() {
        editor.notice("The selected clips are silent");
    } else {
        editor.apply_levels(NormalizeClipGains { gains });
    }
}

pub(crate) fn db(amplitude: f32) -> f32 {
    20.0 * amplitude.log10()
}
