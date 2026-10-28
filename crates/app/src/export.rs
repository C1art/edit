use std::path::PathBuf;
use std::sync::Arc;

use egui::{Key, ProgressBar};
use playback::{ExportEvent, ExportJob, ExportSettings};

use crate::editor::Editor;

const DEFAULT_CRF: u8 = 18;
const SAMPLE_RATE: u32 = 48_000;
const AUDIO_BITS_PER_SECOND: u32 = 192_000;

struct Running {
    job: ExportJob,
    path: PathBuf,
    frames_done: i64,
    frames_total: i64,
}

/// The export dialog: settings while idle, progress while a job runs.
pub struct ExportDialog {
    path: String,
    crf: u8,
    running: Option<Running>,
}

/// Opens the dialog, suggesting a file next to the first imported media.
pub fn open(editor: &mut Editor) {
    if editor.export.is_some() {
        return;
    }
    let folder = editor
        .timeline
        .sequence()
        .media
        .first()
        .and_then(|item| item.path.parent())
        .map(PathBuf::from)
        .unwrap_or_default();
    editor.export = Some(ExportDialog {
        path: folder.join("Sequence.mp4").display().to_string(),
        crf: DEFAULT_CRF,
        running: None,
    });
}

pub fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    poll(editor);
    let Some(dialog) = &mut editor.export else {
        return;
    };
    let mut start = false;
    let mut close = false;
    egui::Window::new("Export Media")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| match &dialog.running {
            Some(running) => {
                ui.label(format!("Exporting to {}", running.path.display()));
                let fraction = if running.frames_total > 0 {
                    running.frames_done as f32 / running.frames_total as f32
                } else {
                    0.0
                };
                ui.add(
                    ProgressBar::new(fraction)
                        .desired_width(420.0)
                        .text(format!(
                            "frame {} of {}",
                            running.frames_done, running.frames_total
                        )),
                );
                if ui.button("Cancel").clicked() {
                    running.job.cancel();
                }
                ctx.request_repaint();
            }
            None => {
                ui.label("Output file (.mp4, .mov or .mkv):");
                ui.add(egui::TextEdit::singleline(&mut dialog.path).desired_width(420.0));
                ui.horizontal(|ui| {
                    ui.label("Quality (CRF, lower is better):");
                    ui.add(egui::Slider::new(&mut dialog.crf, 0..=40));
                });
                ui.label("H.264 and AAC at the sequence's frame size and rate.");
                ui.horizontal(|ui| {
                    start = ui.button("Export").clicked();
                    close = ui.button("Close").clicked()
                        || ui.input(|input| input.key_pressed(Key::Escape));
                });
            }
        });
    if close {
        editor.export = None;
    } else if start {
        start_job(editor);
    }
}

fn start_job(editor: &mut Editor) {
    let Some(dialog) = &mut editor.export else {
        return;
    };
    let path = PathBuf::from(dialog.path.trim().trim_matches('"'));
    let sequence = editor.timeline.sequence();
    let problem = if path.as_os_str().is_empty() {
        Some("Choose an output file first".to_owned())
    } else if sequence.duration() == timeline::Ticks::ZERO {
        Some("The sequence is empty".to_owned())
    } else if !sequence.format.width.is_multiple_of(2) || !sequence.format.height.is_multiple_of(2)
    {
        Some("The frame size must be even to export".to_owned())
    } else {
        None
    };
    if let Some(problem) = problem {
        editor.notice(format!("Cannot export: {problem}"));
        return;
    }
    let Some(render_state) = &editor.render_state else {
        editor.notice("Cannot export: there is no GPU renderer");
        return;
    };
    let settings = ExportSettings {
        path: path.clone(),
        crf: dialog.crf,
        sample_rate: SAMPLE_RATE,
        audio_bits_per_second: AUDIO_BITS_PER_SECOND,
    };
    match ExportJob::start(
        Arc::new(sequence.clone()),
        render_state.device.clone(),
        render_state.queue.clone(),
        settings,
    ) {
        Ok(job) => {
            dialog.running = Some(Running {
                job,
                path,
                frames_done: 0,
                frames_total: 0,
            });
        }
        Err(error) => editor.notice(format!("Cannot export: {error}")),
    }
}

fn poll(editor: &mut Editor) {
    let Some(running) = editor
        .export
        .as_mut()
        .and_then(|dialog| dialog.running.as_mut())
    else {
        return;
    };
    let mut notices = Vec::new();
    let mut ended = false;
    for event in running.job.poll() {
        match event {
            ExportEvent::Progress {
                frames_done,
                frames_total,
            } => {
                running.frames_done = frames_done;
                running.frames_total = frames_total;
            }
            ExportEvent::Warning(error) => notices.push(format!("Export: {error}")),
            ExportEvent::Finished => {
                notices.push(format!("Exported {}", running.path.display()));
                ended = true;
            }
            ExportEvent::Cancelled => {
                notices.push("Export cancelled".to_owned());
                ended = true;
            }
            ExportEvent::Failed(error) => {
                notices.push(format!("Export failed: {error}"));
                ended = true;
            }
        }
    }
    if ended {
        if let Some(dialog) = &mut editor.export {
            dialog.running = None;
        }
    }
    for notice in notices {
        editor.notice(notice);
    }
}
