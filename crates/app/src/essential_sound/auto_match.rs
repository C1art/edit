use std::time::Duration;

use egui::ProgressBar;
use playback::{clip_alone, LoudnessEvent, LoudnessJob};
use timeline::{AudioType, ClipId, Gain, NormalizeClipGains};

use crate::editor::Editor;

/// The clips being measured and the loudness they are to reach.
pub(super) struct Matching {
    job: LoudnessJob,
    clips: Vec<ClipId>,
    target: f32,
    progress: f32,
}

/// The loudness in LUFS each type of sound is matched to, quieter for what sits under
/// the dialogue.
fn target(audio_type: AudioType) -> f32 {
    match audio_type {
        AudioType::Dialogue => -23.0,
        AudioType::Music => -25.0,
        AudioType::Effects => -21.0,
        AudioType::Ambience => -28.0,
    }
}

pub(super) fn section(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    clips: &[ClipId],
    audio_type: AudioType,
) {
    let target = target(audio_type);
    let mut start = false;
    egui::CollapsingHeader::new("Loudness")
        .default_open(true)
        .show(ui, |ui| match &editor.essential_sound.matching {
            Some(matching) => {
                let bar = ProgressBar::new(matching.progress).text("Measuring…");
                ui.add(bar);
            }
            None => {
                let tip = format!("Sets each clip's gain so it plays at {target:.0} LUFS");
                start = ui.button("Auto-Match").on_hover_text(tip).clicked();
            }
        });
    if start {
        let sequence = editor.timeline.sequence();
        let pieces = clips
            .iter()
            .filter_map(|&id| clip_alone(sequence, id))
            .collect();
        match LoudnessJob::start(pieces) {
            Ok(job) => {
                editor.essential_sound.matching = Some(Matching {
                    job,
                    clips: clips.to_vec(),
                    target,
                    progress: 0.0,
                });
            }
            Err(error) => editor.notice(format!("Cannot measure: {error}")),
        }
    }
}

/// Collects the measurements and sets the clips' gains once they are all in; silent
/// clips are left as they are.
pub(crate) fn poll(editor: &mut Editor, ctx: &egui::Context) {
    let Some(matching) = &mut editor.essential_sound.matching else {
        return;
    };
    ctx.request_repaint_after(Duration::from_millis(100));
    let mut warnings = Vec::new();
    let mut measured = None;
    for event in matching.job.poll() {
        match event {
            LoudnessEvent::Progress(progress) => matching.progress = progress,
            LoudnessEvent::Warning(warning) => warnings.push(warning),
            LoudnessEvent::Measured(results) => measured = Some(results),
        }
    }
    for warning in warnings {
        editor.notice(format!("Measured as silence: {warning}"));
    }
    let Some(results) = measured else {
        return;
    };
    let Some(matching) = editor.essential_sound.matching.take() else {
        return;
    };
    let gains: Vec<(ClipId, Gain)> = matching
        .clips
        .into_iter()
        .zip(results)
        .filter_map(|(clip, result)| {
            let integrated = result.integrated?;
            Some((clip, Gain::from_db(matching.target - integrated)))
        })
        .collect();
    if gains.is_empty() {
        editor.notice("The selected clips are silent");
    } else {
        editor.apply_levels(NormalizeClipGains { gains });
    }
}
