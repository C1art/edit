use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam::channel::{self, Receiver, Sender, TryRecvError};
use egui::{Key, ProgressBar};
use project::ProjectFile;
use timeline::{collect_files, Collect, FileCopy, Sequence};

use crate::editor::Editor;
use crate::error::AppError;

/// Premiere's Project Manager: copies the files a project uses into one folder and saves
/// a copy of the project that reads them from there, to archive or hand it on.
pub(crate) struct ProjectManager {
    folder: String,
    keep: Collect,
    job: Option<CollectJob>,
}

struct CollectJob {
    events: Receiver<CollectEvent>,
    cancel: Arc<AtomicBool>,
    copied: usize,
    total: usize,
}

enum CollectEvent {
    Copied,
    /// The saved project and the files that could not be copied.
    Finished(Result<(PathBuf, Vec<String>), AppError>),
}

impl Editor {
    pub(crate) fn open_project_manager(&mut self) {
        let name = self.project_name();
        let folder = self
            .project_path
            .as_ref()
            .and_then(|path| path.parent())
            .map_or_else(PathBuf::new, Path::to_path_buf)
            .join(format!("{name} Copied"));
        self.project_manager = Some(ProjectManager {
            folder: folder.display().to_string(),
            keep: Collect::UsedOnly,
            job: None,
        });
    }

    fn project_name(&self) -> String {
        self.project_path
            .as_ref()
            .and_then(|path| path.file_stem())
            .map_or_else(
                || "Untitled".to_owned(),
                |stem| stem.to_string_lossy().into_owned(),
            )
    }
}

pub(crate) fn window(editor: &mut Editor, ctx: &egui::Context) {
    poll(editor, ctx);
    let name = editor.project_name();
    let Some(manager) = &mut editor.project_manager else {
        return;
    };
    let mut start = false;
    let mut close = false;
    crate::dialog::window(ctx, "Project Manager").show(ctx, |ui| match &manager.job {
        Some(job) => {
            ui.label(format!("Copying {} of {} files…", job.copied, job.total));
            let fraction = job.copied as f32 / job.total.max(1) as f32;
            ui.add(ProgressBar::new(fraction).show_percentage());
            close = ui.button("Cancel").clicked();
        }
        None => {
            ui.label("Copy the project's media, proxies and LUTs to:");
            ui.text_edit_singleline(&mut manager.folder);
            ui.radio_value(&mut manager.keep, Collect::UsedOnly, "Exclude unused clips");
            ui.radio_value(&mut manager.keep, Collect::All, "Keep every item");
            ui.weak(format!(
                "Saves {name}.{} there, reading the copies.",
                project::EXTENSION
            ));
            crate::dialog::footer(ui, |ui| {
                close = ui.button("Cancel").clicked()
                    || ui.input(|input| input.key_pressed(Key::Escape));
                start = ui
                    .add(crate::dialog::primary("Collect Files and Copy"))
                    .clicked()
                    || ui.input(|input| input.key_pressed(Key::Enter));
            });
        }
    });
    if close {
        if let Some(job) = manager.job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
        editor.project_manager = None;
    } else if start && !manager.folder.trim().is_empty() {
        let folder = PathBuf::from(manager.folder.trim());
        let (sequence, copies) = collect_files(editor.timeline.sequence(), &folder, manager.keep);
        let project = folder.join(format!("{name}.{}", project::EXTENSION));
        let total = copies.len();
        match spawn(sequence, copies, project) {
            Ok((events, cancel)) => {
                manager.job = Some(CollectJob {
                    events,
                    cancel,
                    copied: 0,
                    total,
                });
            }
            Err(error) => editor.notice(format!("Cannot start copying: {error}")),
        }
    }
}

fn poll(editor: &mut Editor, ctx: &egui::Context) {
    let Some(job) = editor
        .project_manager
        .as_mut()
        .and_then(|manager| manager.job.as_mut())
    else {
        return;
    };
    let finished = loop {
        match job.events.try_recv() {
            Ok(CollectEvent::Copied) => job.copied += 1,
            Ok(CollectEvent::Finished(result)) => break Some(result),
            Err(TryRecvError::Empty) => break None,
            Err(TryRecvError::Disconnected) => break Some(Err(AppError::WorkerStopped)),
        }
    };
    let Some(result) = finished else {
        ctx.request_repaint_after(Duration::from_millis(100));
        return;
    };
    editor.project_manager = None;
    match result {
        Ok((project, failed)) if failed.is_empty() => {
            editor.notice(format!("Collected the project into {}", project.display()));
        }
        Ok((project, failed)) => editor.notice(format!(
            "Collected the project into {}, without {}",
            project.display(),
            failed.join(", ")
        )),
        Err(error) => editor.notice(format!("Cannot collect the project: {error}")),
    }
}

fn spawn(
    sequence: Sequence,
    copies: Vec<FileCopy>,
    project: PathBuf,
) -> std::io::Result<(Receiver<CollectEvent>, Arc<AtomicBool>)> {
    let (events, received) = channel::unbounded();
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = Arc::clone(&cancel);
    thread::Builder::new()
        .name("project manager".to_owned())
        .spawn(move || {
            let result = collect(sequence, &copies, &project, &stop, &events);
            // A closed window no longer listens.
            let _ = events.send(CollectEvent::Finished(result));
        })?;
    Ok((received, cancel))
}

fn collect(
    sequence: Sequence,
    copies: &[FileCopy],
    project: &Path,
    stop: &AtomicBool,
    events: &Sender<CollectEvent>,
) -> Result<(PathBuf, Vec<String>), AppError> {
    let folder = project.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(folder)?;
    let mut failed = Vec::new();
    for copy in copies {
        if stop.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        if copy.from != copy.to {
            if let Err(error) = copy_source(&copy.from, &copy.to) {
                failed.push(format!("{} ({error})", copy.from.display()));
            }
        }
        let _ = events.send(CollectEvent::Copied);
    }
    ProjectFile::new(sequence).save(project)?;
    Ok((project.to_path_buf(), failed))
}

/// Copies a file, or each frame of an image sequence to the pattern it is copied to.
fn copy_source(from: &Path, to: &Path) -> std::io::Result<()> {
    let Some(frames) = media::sequence_frames(from) else {
        return std::fs::copy(from, to).map(drop);
    };
    frames.into_iter().try_for_each(|(number, frame)| {
        let target = media::sequence_frame(to, number)
            .ok_or_else(|| std::io::Error::other("the copy is not named as a sequence"))?;
        std::fs::copy(frame, target).map(drop)
    })
}
