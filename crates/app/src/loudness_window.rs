use std::sync::Arc;

use egui::ProgressBar;
use playback::{Loudness, LoudnessEvent, LoudnessJob};
use timeline::{Gain, SetMasterGain, Ticks};

use crate::editor::Editor;
use crate::gain_dialog::db;

/// Common delivery targets in LUFS.
const TARGETS: [(&str, f32); 3] = [
    ("EBU R128", -23.0),
    ("ATSC A/85", -24.0),
    ("Streaming", -14.0),
];

/// The program's loudness between the marks, or over the whole sequence, and the master
/// volume change that would bring it to a target.
pub(crate) struct LoudnessWindow {
    job: Option<LoudnessJob>,
    progress: f32,
    marked: bool,
    measured: Option<Loudness>,
    /// The master volume the program was measured at.
    master: Gain,
    target: f32,
}

/// Starts measuring the sequence, keeping the target of a window already open.
pub(crate) fn measure(editor: &mut Editor) {
    let sequence = editor.timeline.sequence();
    let marked = sequence.marks.is_set();
    let range = if marked {
        sequence.marks.range(sequence.duration())
    } else {
        Ticks::ZERO..sequence.duration()
    };
    if range.is_empty() {
        editor.notice("There is nothing to measure");
        return;
    }
    let master = sequence.master;
    let job = LoudnessJob::start(vec![(Arc::new(sequence.clone()), range)]);
    match job {
        Ok(job) => {
            let target = editor.loudness.as_ref().map_or(-23.0, |open| open.target);
            editor.loudness = Some(LoudnessWindow {
                job: Some(job),
                progress: 0.0,
                marked,
                measured: None,
                master,
                target,
            });
        }
        Err(error) => editor.notice(format!("Cannot measure: {error}")),
    }
}

pub(crate) fn window(editor: &mut Editor, ctx: &egui::Context) {
    let Some(shown) = &mut editor.loudness else {
        return;
    };
    let mut warnings = Vec::new();
    if let Some(job) = &shown.job {
        for event in job.poll() {
            match event {
                LoudnessEvent::Progress(progress) => shown.progress = progress,
                LoudnessEvent::Warning(warning) => warnings.push(warning),
                LoudnessEvent::Measured(results) => shown.measured = results.first().copied(),
            }
        }
        if shown.measured.is_some() {
            shown.job = None;
        }
        ctx.request_repaint();
    }
    let mut open = true;
    let mut again = false;
    let mut master = None;
    egui::Window::new("Loudness")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(if shown.marked {
                "Sequence In to Out"
            } else {
                "Whole sequence"
            });
            let Some(measured) = shown.measured else {
                ui.add(
                    ProgressBar::new(shown.progress)
                        .show_percentage()
                        .text("Measuring…"),
                );
                return;
            };
            egui::Grid::new("loudness").num_columns(2).show(ui, |ui| {
                ui.label("Integrated loudness");
                ui.label(
                    measured
                        .integrated
                        .map_or_else(|| "Silent".to_owned(), |lufs| format!("{lufs:.1} LUFS")),
                );
                ui.end_row();
                ui.label("Sample peak");
                ui.label(level(db(measured.peak), "dBFS"));
                ui.end_row();
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Target");
                ui.add(
                    egui::DragValue::new(&mut shown.target)
                        .range(-60.0..=0.0)
                        .speed(0.1)
                        .suffix(" LUFS"),
                );
                for (name, target) in TARGETS {
                    let button = ui.small_button(name);
                    if button.on_hover_text(format!("{target} LUFS")).clicked() {
                        shown.target = target;
                    }
                }
            });
            if let Some(integrated) = measured.integrated {
                let change = shown.target - integrated;
                let gain = Gain::from_db(shown.master.db() + change);
                let reached = gain.db() - shown.master.db();
                ui.label(format!(
                    "Master volume {} to {}, peaks at {}",
                    level(shown.master.db(), "dB"),
                    level(gain.db(), "dB"),
                    level(db(measured.peak) + reached, "dBFS"),
                ));
                if ui.button("Set Master Volume").clicked() {
                    master = Some(gain);
                }
            }
            if ui.button("Measure Again").clicked() {
                again = true;
            }
        });
    for warning in warnings {
        editor.notice(format!("Measured as silence: {warning}"));
    }
    if !open {
        editor.loudness = None;
    } else if let Some(gain) = master {
        editor.apply_levels(SetMasterGain { gain });
        measure(editor);
    } else if again {
        measure(editor);
    }
}

fn level(db: f32, unit: &str) -> String {
    if db.is_finite() {
        format!("{db:+.1} {unit}")
    } else {
        format!("-inf {unit}")
    }
}
