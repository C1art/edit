//! Remove Pauses: listens to the selected clips and cuts out where they go quiet, as
//! when tightening someone speaking to camera.

use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use egui::{DragValue, Key, ProgressBar};
use playback::{EnvelopeEvent, EnvelopeJob};
use timeline::{
    pauses, ClipId, Gain, PauseRules, RemovePauses, Sequence, Ticks, Timebase, TrackKind, TrackRef,
};

use crate::editor::Editor;

/// Fine enough to find the quiet between sentences without hearing every syllable.
const WINDOWS_PER_SECOND: u32 = 50;

pub(crate) struct PausesDialog {
    /// The selected audio clips, which are listened to.
    heard: Vec<ClipId>,
    /// The tracks holding the selection, which pauses are cut from.
    tracks: Vec<TrackRef>,
    threshold_db: f32,
    shortest_ms: i64,
    padding_ms: i64,
    ripple: bool,
    measuring: Option<Measuring>,
}

struct Measuring {
    job: EnvelopeJob,
    start: Ticks,
    progress: f32,
}

impl Editor {
    pub(crate) fn open_remove_pauses(&mut self) {
        let sequence = self.timeline.sequence();
        let selected = sequence.with_links(&self.view.selection);
        let placed: Vec<_> = selected
            .iter()
            .filter_map(|&id| sequence.clip(id).map(|(track, clip)| (track, clip.id)))
            .collect();
        let heard: Vec<ClipId> = placed
            .iter()
            .filter(|(track, _)| track.kind == TrackKind::Audio)
            .map(|&(_, id)| id)
            .collect();
        if heard.is_empty() {
            self.notice("Select clips with sound to find pauses in");
            return;
        }
        let mut tracks: Vec<TrackRef> = placed.iter().map(|&(track, _)| track).collect();
        tracks.sort_unstable();
        tracks.dedup();
        self.pauses_dialog = Some(PausesDialog {
            heard,
            tracks,
            threshold_db: -40.0,
            shortest_ms: 500,
            padding_ms: 150,
            ripple: true,
            measuring: None,
        });
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(dialog) = &mut editor.pauses_dialog else {
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
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    let mut remove = false;
    let mut cancelled = false;
    crate::dialog::window(ctx, "Remove Pauses").show(ctx, |ui| {
        let idle = dialog.measuring.is_none();
        ui.add_enabled_ui(idle, |ui| settings(ui, dialog));
        if let Some(measuring) = &dialog.measuring {
            ui.add(
                ProgressBar::new(measuring.progress)
                    .show_percentage()
                    .text("Listening…"),
            );
        }
        crate::dialog::footer(ui, |ui| {
            cancelled =
                ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape));
            remove = idle
                && (ui.add(crate::dialog::primary("Remove")).clicked()
                    || ui.input(|input| input.key_pressed(Key::Enter)));
        });
    });
    for warning in warnings {
        editor.notice(format!("Heard as silence: {warning}"));
    }
    if cancelled {
        editor.pauses_dialog = None;
    } else if let Some(levels) = measured {
        cut(editor, &levels);
    } else if remove {
        start(editor);
    }
}

fn settings(ui: &mut egui::Ui, dialog: &mut PausesDialog) {
    egui::Grid::new("pauses")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label("Quieter than");
            ui.add(
                DragValue::new(&mut dialog.threshold_db)
                    .range(-80.0..=-10.0)
                    .speed(0.2)
                    .suffix(" dB"),
            )
            .on_hover_text("Raise it if room noise keeps pauses from being found");
            ui.end_row();
            ui.label("For at least");
            ui.add(
                DragValue::new(&mut dialog.shortest_ms)
                    .range(100..=10_000)
                    .suffix(" ms"),
            );
            ui.end_row();
            ui.label("Keep beside sound");
            ui.add(
                DragValue::new(&mut dialog.padding_ms)
                    .range(0..=2_000)
                    .suffix(" ms"),
            )
            .on_hover_text("Quiet left before and after each phrase so words aren't clipped");
            ui.end_row();
            ui.label("Pauses");
            ui.vertical(|ui| {
                ui.radio_value(&mut dialog.ripple, true, "Ripple delete");
                ui.radio_value(&mut dialog.ripple, false, "Delete, leaving gaps");
            });
            ui.end_row();
        });
    let note = if dialog.ripple {
        "Closing up moves every sync-locked track along."
    } else {
        "Only the selected clips' tracks are cut."
    };
    ui.weak(note);
}

fn start(editor: &mut Editor) {
    let Some(dialog) = editor.pauses_dialog.as_mut() else {
        return;
    };
    let sequence = editor.timeline.sequence();
    let Some(span) = heard_span(sequence, &dialog.heard) else {
        editor.pauses_dialog = None;
        return;
    };
    let heard = heard_alone(sequence, &dialog.heard);
    match EnvelopeJob::start((Arc::new(heard), span.clone()), window(sequence)) {
        Ok(job) => {
            dialog.measuring = Some(Measuring {
                job,
                start: span.start,
                progress: 0.0,
            });
        }
        Err(error) => editor.notice(format!("Cannot listen for pauses: {error}")),
    }
}

fn cut(editor: &mut Editor, levels: &[f32]) {
    let Some(dialog) = editor.pauses_dialog.take() else {
        return;
    };
    let Some(measuring) = dialog.measuring else {
        return;
    };
    let sequence = editor.timeline.sequence();
    let rules = PauseRules {
        threshold_db: dialog.threshold_db,
        shortest: milliseconds(sequence, dialog.shortest_ms),
        padding: milliseconds(sequence, dialog.padding_ms),
    };
    let within = dialog
        .heard
        .iter()
        .filter_map(|&id| sequence.clip(id).map(|(_, clip)| clip.start..clip.end()))
        .collect();
    let found = pauses(
        levels,
        (measuring.start, window(sequence)),
        rules,
        within,
        (sequence.format.frame_rate, sequence.timebase),
    );
    let removed = found
        .iter()
        .fold(Ticks::ZERO, |sum, pause| sum + (pause.end - pause.start));
    let count = found.len();
    let tenths =
        Timebase::new(1, 10).map_or(0, |tenth| removed.rescale(sequence.timebase, tenth).0);
    if editor.apply(RemovePauses {
        pauses: found,
        tracks: dialog.tracks,
        ripple: dialog.ripple,
    }) {
        let (whole, tenth) = (tenths / 10, tenths % 10);
        editor.notice(format!(
            "Removed {count} pause(s), {whole}.{tenth} s in all"
        ));
    } else {
        editor.notice("No pauses that long and that quiet were heard");
    }
}

pub(crate) fn heard_span(sequence: &Sequence, clips: &[ClipId]) -> Option<Range<Ticks>> {
    let spans = clips
        .iter()
        .filter_map(|&id| sequence.clip(id).map(|(_, clip)| clip.start..clip.end()));
    spans.reduce(|joined, span| joined.start.min(span.start)..joined.end.max(span.end))
}

/// Only `clips`, as they are mixed.
pub(crate) fn heard_alone(sequence: &Sequence, clips: &[ClipId]) -> Sequence {
    let mut heard = sequence.clone();
    heard.video.iter_mut().for_each(|track| track.clips.clear());
    heard.master = Gain::UNITY;
    for track in &mut heard.audio {
        track.clips.retain(|clip| clips.contains(&clip.id));
        track.muted = false;
        track.solo = false;
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
