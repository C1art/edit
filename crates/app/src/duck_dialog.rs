use std::ops::Range;
use std::sync::Arc;

use egui::{Key, ProgressBar};
use playback::{EnvelopeEvent, EnvelopeJob};
use timeline::{
    active_spans, ducked_volume, AutoDuck, ClipId, Gain, Sequence, Ticks, Timebase, Track,
    TrackKind,
};

use crate::editor::Editor;

const WINDOWS_PER_SECOND: u32 = 20;
/// Pauses between words shorter than this keep the music down.
const PAUSE_MS: i64 = 300;

/// Lowers the selected audio clips wherever the chosen tracks are heard, as music is
/// ducked under dialogue.
pub(crate) struct DuckDialog {
    clips: Vec<ClipId>,
    /// One per audio track: whether the clips duck under it.
    against: Vec<bool>,
    amount_db: f32,
    /// 0 to 10; higher hears quieter sound as worth ducking for.
    sensitivity: f32,
    fade_ms: u32,
    measuring: Option<Measuring>,
}

struct Measuring {
    job: EnvelopeJob,
    start: Ticks,
    progress: f32,
}

impl Editor {
    pub(crate) fn open_auto_duck(&mut self) {
        let sequence = self.timeline.sequence();
        let clips: Vec<ClipId> = sequence
            .with_links(&self.view.selection)
            .into_iter()
            .filter(|&id| {
                sequence
                    .clip(id)
                    .is_some_and(|(track, clip)| track.kind == TrackKind::Audio && !clip.hold)
            })
            .collect();
        if clips.is_empty() {
            self.notice("Select the music clips to duck");
            return;
        }
        let holds_target = |track: &Track| track.clips.iter().any(|clip| clips.contains(&clip.id));
        let against = sequence
            .audio
            .iter()
            .map(|track| !holds_target(track))
            .collect();
        self.duck_dialog = Some(DuckDialog {
            clips,
            against,
            amount_db: 18.0,
            sensitivity: 6.0,
            fade_ms: 800,
            measuring: None,
        });
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(dialog) = &mut editor.duck_dialog else {
        return;
    };
    let mut measured = None;
    let mut warnings = Vec::new();
    if let Some(measuring) = &mut dialog.measuring {
        for event in measuring.job.poll() {
            match event {
                EnvelopeEvent::Progress(progress) => measuring.progress = progress,
                EnvelopeEvent::Warning(warning) => warnings.push(warning),
                EnvelopeEvent::Measured(levels) => measured = Some(levels),
            }
        }
        ctx.request_repaint();
    }
    let mut generate = false;
    let mut cancelled = false;
    egui::Window::new("Auto Duck")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            let measuring = dialog.measuring.as_ref();
            ui.add_enabled_ui(measuring.is_none(), |ui| {
                ui.label(format!(
                    "Lower {} selected audio clip(s) while these tracks are heard:",
                    dialog.clips.len()
                ));
                ui.horizontal_wrapped(|ui| {
                    for (index, against) in dialog.against.iter_mut().enumerate() {
                        ui.checkbox(against, format!("A{}", index + 1));
                    }
                });
                egui::Grid::new("duck settings")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Duck Amount");
                        ui.add(
                            egui::DragValue::new(&mut dialog.amount_db)
                                .range(0.0..=-Gain::SILENT.db())
                                .speed(0.1)
                                .suffix(" dB"),
                        );
                        ui.end_row();
                        ui.label("Sensitivity");
                        ui.add(egui::Slider::new(&mut dialog.sensitivity, 0.0..=10.0))
                            .on_hover_text("Higher ducks under quieter sound");
                        ui.end_row();
                        ui.label("Fade Duration");
                        ui.add(
                            egui::DragValue::new(&mut dialog.fade_ms)
                                .range(0..=5_000)
                                .suffix(" ms"),
                        );
                        ui.end_row();
                    });
                ui.weak("Replaces the clips' volume keyframes.");
            });
            if let Some(measuring) = measuring {
                ui.add(
                    ProgressBar::new(measuring.progress)
                        .show_percentage()
                        .text("Listening…"),
                );
            }
            ui.horizontal(|ui| {
                generate = measuring.is_none()
                    && (ui.button("Generate Keyframes").clicked()
                        || ui.input(|input| input.key_pressed(Key::Enter)));
                cancelled = ui.button("Cancel").clicked()
                    || ui.input(|input| input.key_pressed(Key::Escape));
            });
        });
    for warning in warnings {
        editor.notice(format!("Heard as silence: {warning}"));
    }
    if cancelled {
        editor.duck_dialog = None;
    } else if let Some(levels) = measured {
        duck(editor, &levels);
    } else if generate {
        start(editor);
    }
}

fn start(editor: &mut Editor) {
    let Some(dialog) = editor.duck_dialog.as_mut() else {
        return;
    };
    let sequence = editor.timeline.sequence();
    let fade = milliseconds(sequence, i64::from(dialog.fade_ms));
    let Some(span) = targets_span(sequence, &dialog.clips, fade) else {
        editor.duck_dialog = None;
        return;
    };
    if !dialog.against.contains(&true) {
        editor.notice("Choose the tracks to duck under");
        return;
    }
    let heard = heard_alone(sequence, &dialog.against, &dialog.clips);
    match EnvelopeJob::start((Arc::new(heard), span.clone()), window(sequence)) {
        Ok(job) => {
            dialog.measuring = Some(Measuring {
                job,
                start: span.start,
                progress: 0.0,
            });
        }
        Err(error) => editor.notice(format!("Cannot auto duck: {error}")),
    }
}

fn duck(editor: &mut Editor, levels: &[f32]) {
    let Some(dialog) = editor.duck_dialog.take() else {
        return;
    };
    let Some(measuring) = dialog.measuring else {
        return;
    };
    let sequence = editor.timeline.sequence();
    let threshold = -20.0 - 3.0 * dialog.sensitivity;
    let pause = milliseconds(sequence, PAUSE_MS);
    let spans = active_spans(levels, measuring.start, window(sequence), threshold, pause);
    let fade = milliseconds(sequence, i64::from(dialog.fade_ms));
    let volumes: Vec<_> = dialog
        .clips
        .iter()
        .filter_map(|&id| {
            let (_, clip) = sequence.clip(id)?;
            Some((id, ducked_volume(clip, &spans, dialog.amount_db, fade)?))
        })
        .collect();
    if volumes.is_empty() {
        editor.notice("Nothing was heard on the chosen tracks under the selected clips");
    } else {
        editor.apply_levels(AutoDuck { volumes });
    }
}

/// The sequence time the clips cover, widened by a fade either side so sound just
/// before a clip still ducks its head.
fn targets_span(sequence: &Sequence, clips: &[ClipId], fade: Ticks) -> Option<Range<Ticks>> {
    let spans = clips
        .iter()
        .filter_map(|&id| sequence.clip(id).map(|(_, clip)| clip.start..clip.end()));
    let (start, end) = spans.fold(None, |joined: Option<(Ticks, Ticks)>, span| {
        Some(joined.map_or((span.start, span.end), |(start, end)| {
            (start.min(span.start), end.max(span.end))
        }))
    })?;
    Some((start - fade).max(Ticks::ZERO)..end + fade)
}

/// Only the chosen audio tracks, as they are mixed, without the clips being ducked.
fn heard_alone(sequence: &Sequence, against: &[bool], ducked: &[ClipId]) -> Sequence {
    let mut heard = sequence.clone();
    heard.video.iter_mut().for_each(|track| track.clips.clear());
    heard.master = Gain::UNITY;
    for (index, track) in heard.audio.iter_mut().enumerate() {
        if against.get(index).copied().unwrap_or(false) {
            track.clips.retain(|clip| !ducked.contains(&clip.id));
            track.muted = false;
            track.solo = false;
        } else {
            track.clips.clear();
        }
    }
    heard
}

fn window(sequence: &Sequence) -> Ticks {
    let windows = Timebase::new(1, WINDOWS_PER_SECOND).unwrap_or(Timebase::HZ_90K);
    Ticks(1).rescale(windows, sequence.timebase)
}

fn milliseconds(sequence: &Sequence, ms: i64) -> Ticks {
    let millisecond = Timebase::new(1, 1_000).unwrap_or(Timebase::HZ_90K);
    Ticks(ms).rescale(millisecond, sequence.timebase)
}
