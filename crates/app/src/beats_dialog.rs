//! Add Markers at Beats: listens to the selected music and drops a marker on its beats,
//! ready for cutting to the music or for Automate to Sequence.

use std::sync::Arc;
use std::time::Duration;

use egui::{Key, ProgressBar};
use playback::{EnvelopeEvent, EnvelopeJob};
use timeline::{beats, AddMarkers, ClipId, MarkerColor, MarkerOwner, Ticks, Timebase, TrackKind};

use crate::editor::Editor;
use crate::pauses_dialog::{heard_alone, heard_span};

/// Fine enough to place a beat within a hundredth of a second.
const WINDOWS_PER_SECOND: u32 = 100;
const EVERY: [(usize, &str); 4] = [
    (1, "Every beat"),
    (2, "Every 2nd beat"),
    (4, "Every 4th beat"),
    (8, "Every 8th beat"),
];

pub(crate) struct BeatsDialog {
    heard: Vec<ClipId>,
    every: usize,
    on_clip: bool,
    measuring: Option<Measuring>,
}

struct Measuring {
    job: EnvelopeJob,
    start: Ticks,
    progress: f32,
}

impl Editor {
    pub(crate) fn open_beat_markers(&mut self) {
        let sequence = self.timeline.sequence();
        let heard: Vec<ClipId> = sequence
            .with_links(&self.view.selection)
            .into_iter()
            .filter(|&id| {
                sequence
                    .clip(id)
                    .is_some_and(|(track, _)| track.kind == TrackKind::Audio)
            })
            .collect();
        if heard.is_empty() {
            self.notice("Select the music to find beats in");
            return;
        }
        self.beats_dialog = Some(BeatsDialog {
            heard,
            every: 1,
            on_clip: false,
            measuring: None,
        });
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(dialog) = &mut editor.beats_dialog else {
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
    let mut add = false;
    let mut cancelled = false;
    crate::dialog::window(ctx, "Add Markers at Beats").show(ctx, |ui| {
        let idle = dialog.measuring.is_none();
        ui.add_enabled_ui(idle, |ui| {
            for (every, label) in EVERY {
                ui.radio_value(&mut dialog.every, every, label);
            }
            ui.separator();
            ui.add_enabled_ui(dialog.heard.len() == 1, |ui| {
                ui.radio_value(&mut dialog.on_clip, false, "Sequence markers");
                ui.radio_value(&mut dialog.on_clip, true, "Clip markers")
                    .on_hover_text("The beats stay with the music when it is moved");
            });
        });
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
            add = idle
                && (ui.add(crate::dialog::primary("Add")).clicked()
                    || ui.input(|input| input.key_pressed(Key::Enter)));
        });
    });
    for warning in warnings {
        editor.notice(format!("Heard as silence: {warning}"));
    }
    if cancelled {
        editor.beats_dialog = None;
    } else if let Some(levels) = measured {
        mark(editor, &levels);
    } else if add {
        start(editor);
    }
}

fn start(editor: &mut Editor) {
    let Some(dialog) = editor.beats_dialog.as_mut() else {
        return;
    };
    let sequence = editor.timeline.sequence();
    let Some(span) = heard_span(sequence, &dialog.heard) else {
        editor.beats_dialog = None;
        return;
    };
    let heard = heard_alone(sequence, &dialog.heard);
    match EnvelopeJob::start((Arc::new(heard), span.clone()), window(sequence.timebase)) {
        Ok(job) => {
            dialog.measuring = Some(Measuring {
                job,
                start: span.start,
                progress: 0.0,
            });
        }
        Err(error) => editor.notice(format!("Cannot listen for beats: {error}")),
    }
}

fn mark(editor: &mut Editor, levels: &[f32]) {
    let Some(dialog) = editor.beats_dialog.take() else {
        return;
    };
    let Some(measuring) = dialog.measuring else {
        return;
    };
    let sequence = editor.timeline.sequence();
    let timebase = sequence.timebase;
    let Some(found) = beats(levels, (measuring.start, window(timebase)), timebase) else {
        editor.notice("No steady beat was heard");
        return;
    };
    let placed: Vec<Ticks> = found.at.into_iter().step_by(dialog.every).collect();
    let music = dialog
        .heard
        .first()
        .and_then(|&id| sequence.clip(id))
        .map(|(_, clip)| clip);
    let (owner, at) = match music {
        Some(clip) if dialog.on_clip => (
            MarkerOwner::Clip(clip.id),
            placed
                .into_iter()
                .filter(|&at| clip.covers(at))
                .map(|at| clip.source_time(at))
                .collect(),
        ),
        _ => (MarkerOwner::Sequence, placed),
    };
    let count = at.len();
    let added = editor.apply(AddMarkers {
        owner,
        at,
        color: MarkerColor::Blue,
    });
    if added {
        editor.notice(format!(
            "Added {count} marker(s) at {:.0} BPM",
            found.per_minute
        ));
    } else {
        editor.notice("Every beat already has a marker");
    }
}

fn window(timebase: Timebase) -> Ticks {
    let windows = Timebase::new(1, WINDOWS_PER_SECOND).unwrap_or(Timebase::HZ_90K);
    Ticks(1).rescale(windows, timebase)
}
