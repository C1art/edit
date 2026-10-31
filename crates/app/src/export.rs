use std::path::{Path, PathBuf};

use egui::Key;
use playback::{ExportEvent, ExportJob};
use timeline::{ExportOverlays, Sequence};

use crate::editor::Editor;
use crate::export_captions::{self, CaptionExport};
use crate::export_format::{ExportFormat, Format};
use crate::export_overlays;
use crate::export_progress::{self, ExportProgress};
use crate::export_settings::settings;
use crate::export_summary::{length_text, span_length, summary};
use crate::export_target::{Holder, Target, TargetCheck};
use crate::path_dialog::FolderUse;
use crate::project_files::{quoted_name, PathPurpose};

mod prepare;

pub(crate) use prepare::prepare;
use prepare::prepare_open;

struct Running {
    job: ExportJob,
    path: PathBuf,
    progress: ExportProgress,
}

/// What part of the sequence to export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Span {
    Sequence,
    InToOut,
}

/// The export dialog: settings while idle, progress while a job runs.
pub struct ExportDialog {
    pub(crate) path: String,
    pub(crate) format: ExportFormat,
    span: Span,
    /// Whether rendered previews stand in for what they cover, as Premiere's Use Previews.
    use_previews: bool,
    captions: CaptionExport,
    overlays: ExportOverlays,
    running: Option<Running>,
    /// The file suggested on opening, swapped for a free name while it's left as it is.
    suggested: Option<PathBuf>,
    target: Option<TargetCheck>,
}

/// Opens the dialog, suggesting a file named after the sequence next to the first
/// imported media.
pub fn open(editor: &mut Editor) {
    if editor.export.is_none() {
        editor.export = Some(ExportDialog::new(editor));
    }
}

impl ExportDialog {
    pub(crate) fn new(editor: &Editor) -> Self {
        let sequence = editor.timeline.sequence();
        let span = if sequence.marks.is_set() {
            Span::InToOut
        } else {
            Span::Sequence
        };
        let folder = editor
            .browse_folders
            .get(&FolderUse::Output)
            .cloned()
            .or_else(|| {
                let first = sequence
                    .media
                    .iter()
                    .find(|item| item.generator.is_none() && item.nest.is_none())?;
                first.path.parent().map(PathBuf::from)
            })
            .or_else(|| editor.project_path.as_deref()?.parent().map(PathBuf::from))
            .filter(|folder| folder.is_absolute())
            .or_else(|| {
                std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join("Videos"))
            })
            .unwrap_or_default();
        let format = ExportFormat::default();
        let file = format!("{}.{}", editor.output_stem(), format.format.extension());
        let path = folder.join(file);
        Self {
            path: path.display().to_string(),
            format,
            span,
            use_previews: false,
            captions: CaptionExport::of(sequence),
            overlays: ExportOverlays::default(),
            running: None,
            suggested: Some(path),
            target: TargetCheck::new(editor.ctx.clone()).ok(),
        }
    }

    pub(crate) fn running_path(&self) -> Option<&Path> {
        self.running.as_ref().map(|running| running.path.as_path())
    }

    pub fn output(&self) -> PathBuf {
        let typed = PathBuf::from(self.path.trim().trim_matches('"'));
        let bare = typed
            .extension()
            .is_none_or(|extension| extension.is_empty());
        // FFmpeg picks the container by the extension, so a bare name gets the format's.
        if bare && typed.file_name().is_some() {
            typed.with_extension(self.format.format.extension())
        } else {
            typed
        }
    }

    /// What exporting would replace, after swapping a suggested file that's taken for a
    /// free one.
    /// `sequence` says whether the captions go to a sidecar file it would also replace.
    pub(crate) fn target(
        &mut self,
        sequence: &Sequence,
        queued: impl FnOnce() -> Vec<PathBuf>,
    ) -> Option<Target> {
        let output = self.output();
        let kind = self.format.format;
        let frames = matches!(kind, Format::Stills(_));
        let sidecar = export_captions::chosen(self.captions, kind.has_video(), sequence)
            == CaptionExport::Sidecar;
        let target = self
            .target
            .as_mut()?
            .target(&output, frames, sidecar, queued)?
            .clone();
        match target {
            Target::Taken {
                free: Some(free), ..
            } if self.suggested.as_ref() == Some(&output) => {
                self.path = free.display().to_string();
                self.suggested = Some(free);
                None
            }
            target => Some(target),
        }
    }

    /// Gives the file the extension a newly chosen format makes.
    pub(crate) fn follow_format(&mut self) {
        let suggested = self.suggested.as_ref() == Some(&self.output());
        self.set_output(&self.output());
        if suggested {
            self.suggested = Some(self.output());
        }
    }

    /// Writes to `path`, in the file type the chosen format makes.
    pub fn set_output(&mut self, path: &Path) {
        self.path = path
            .with_extension(self.format.format.extension())
            .display()
            .to_string();
    }
}

const EMPTY_SEQUENCE: &str = "The sequence is empty";

pub fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    poll(editor);
    let sequence = editor.timeline.sequence();
    let marked = sequence.marks.is_set();
    let (marks, duration) = (sequence.marks, sequence.duration());
    let timing = (
        sequence.timebase,
        sequence.format.frame_rate,
        sequence.time_display,
    );
    let sequence_size = (sequence.format.width, sequence.format.height);
    let has_captions = !sequence.captions.items.is_empty();
    let rendered = editor
        .render_previews
        .as_ref()
        .is_some_and(|previews| previews.bar().any(|(_, ready)| ready));
    // Escape in the file browser closes only that.
    let browsing = editor.path_prompt.is_some();
    let export_queue = &editor.export_queue;
    let Some(dialog) = &mut editor.export else {
        return;
    };
    let mut start = false;
    let mut queue = false;
    let mut close = false;
    let mut browse = false;
    crate::dialog::window(ctx, "Export Media").show(ctx, |ui| match &dialog.running {
        Some(running) => {
            ui.label(format!("Exporting to {}", running.path.display()));
            ui.add(running.progress.bar(420.0));
            crate::dialog::footer(ui, |ui| {
                if ui.button("Cancel").clicked() {
                    running.job.cancel();
                }
            });
            ctx.request_repaint_after(export_progress::REFRESH);
        }
        None => {
            let target = dialog.target(sequence, || export_queue.pending_paths());
            let kind = dialog.format.format;
            let format = &mut dialog.format;
            settings(ui, format, sequence_size, |ui, format| {
                ui.label("Output file:");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut dialog.path).desired_width(300.0));
                    browse = ui.button("Browse…").clicked();
                });
                ui.end_row();
                if let Some(Target::Taken { by, free }) = &target {
                    ui.label("");
                    replace_warning(ui, &mut dialog.path, *by, free.as_deref());
                    ui.end_row();
                }
                ui.label("Source range:");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut dialog.span, Span::Sequence, "Entire sequence");
                    ui.add_enabled_ui(marked, |ui| {
                        ui.radio_value(&mut dialog.span, Span::InToOut, "Sequence in/out")
                            .on_disabled_hover_text("Mark in or out on the program first");
                    });
                });
                ui.end_row();
                let length = match dialog.span {
                    Span::InToOut if marked => span_length(marks.range(duration)),
                    _ => duration,
                };
                ui.label("Duration:");
                ui.label(length_text(format, length, timing));
                ui.end_row();
                if format.format.has_video() {
                    export_overlays::rows(ui, &mut dialog.overlays, sequence);
                }
                if has_captions {
                    export_captions::row(ui, &mut dialog.captions, format.format.has_video());
                }
                if format.format.has_video() {
                    let captions = export_captions::chosen(dialog.captions, true, sequence);
                    let differs =
                        export_overlays::differs_from_program(&dialog.overlays, captions, sequence);
                    ui.label("");
                    ui.add_enabled_ui(rendered && !differs, |ui| {
                        ui.checkbox(&mut dialog.use_previews, "Use Previews")
                            .on_hover_text(
                                "Take rendered previews where they cover instead of \
                                 rendering those parts again; faster, though those parts \
                                 are encoded once more",
                            )
                            .on_disabled_hover_text(if differs {
                                "Previews show the program as it plays, without these \
                                 overlays or caption choice"
                            } else {
                                "Render previews of the sequence first"
                            });
                    });
                    ui.end_row();
                }
            });
            if dialog.format.format != kind {
                dialog.follow_format();
            }
            ui.label(summary(&dialog.format, sequence_size));
            crate::dialog::footer(ui, |ui| {
                close = ui.button("Close").clicked()
                    || (!browsing && ui.input(|input| input.key_pressed(Key::Escape)));
                let has_content = duration != timeline::Ticks::ZERO;
                queue = ui
                    .add_enabled(has_content, egui::Button::new("Queue"))
                    .on_hover_text("Add to the export queue and keep editing")
                    .on_disabled_hover_text(EMPTY_SEQUENCE)
                    .clicked();
                start = ui
                    .add_enabled(has_content, crate::dialog::primary("Export"))
                    .on_disabled_hover_text(EMPTY_SEQUENCE)
                    .clicked();
            });
        }
    });
    if browse {
        editor.prompt_path(PathPurpose::ExportMedia);
    } else if close {
        editor.export = None;
    } else if start {
        start_job(editor);
    } else if queue {
        queue_job(editor);
    }
}

/// Says what exporting to the chosen file would replace, offering a free name instead.
fn replace_warning(ui: &mut egui::Ui, path: &mut String, by: Holder, free: Option<&Path>) {
    let name = quoted_name(Path::new(path.trim().trim_matches('"')));
    let warning = match by {
        Holder::File => format!("{name} already exists and will be replaced."),
        Holder::Sidecar => {
            let output = Path::new(path.trim().trim_matches('"'));
            let captions = quoted_name(&export_captions::sidecar_path(output));
            format!("{captions} already exists and the captions will replace it.")
        }
        Holder::Queue => format!("A queued export already writes {name}."),
    };
    ui.horizontal(|ui| {
        ui.colored_label(ui.visuals().warn_fg_color, warning);
        if let Some(free) = free {
            if ui.button(format!("Use {}", quoted_name(free))).clicked() {
                *path = free.display().to_string();
            }
        }
    });
}

fn start_job(editor: &mut Editor) {
    let Some((sequence, settings)) = prepare_open(editor) else {
        return;
    };
    let Some(render_state) = &editor.render_state else {
        editor.notice("Cannot export: there is no GPU renderer");
        return;
    };
    let path = settings.path.clone();
    let started = ExportJob::start(
        sequence,
        render_state.device.clone(),
        render_state.queue.clone(),
        settings,
    );
    match started {
        Ok(job) => {
            if let Some(dialog) = &mut editor.export {
                dialog.running = Some(Running {
                    job,
                    path,
                    progress: ExportProgress::new(),
                });
            }
        }
        Err(error) => editor.notice(format!("Cannot export: {error}")),
    }
}

/// Hands the export to the queue and closes the dialog, so another can be set up.
fn queue_job(editor: &mut Editor) {
    let Some((sequence, settings)) = prepare_open(editor) else {
        return;
    };
    let summary = editor.export.as_ref().map_or_else(String::new, |dialog| {
        summary(
            &dialog.format,
            (sequence.format.width, sequence.format.height),
        )
    });
    editor.export_queue.add(sequence, settings, summary);
    editor.export_queue.open = true;
    editor.export = None;
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
                running.progress.update(frames_done, frames_total);
            }
            ExportEvent::Measuring(fraction) => running.progress.measuring(fraction),
            ExportEvent::Encoder(encoder) => running.progress.set_encoder(encoder),
            ExportEvent::Timings(timings) => notices.push(format!("Export speed: {timings}")),
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
