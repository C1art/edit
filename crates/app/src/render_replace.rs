use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use crossbeam::channel::{self, Receiver};
use egui::{Key, ProgressBar, RichText};
use media::{AudioCodec, AudioSettings, MediaError, RateControl, VideoCodec};
use playback::{ExportEvent, ExportJob, ExportSettings, VideoEncoding, VideoExport};
use timeline::{
    ClipId, FrameRate, ImportMedia, RenderAndReplace, RestoreUnrendered, SourceId, Ticks, Timebase,
    TrackKind,
};

use crate::editor::Editor;
use crate::error::AppError;

/// Extra source rendered past the last frame the clips use, so they can still be
/// trimmed out a little.
const HANDLE_SECONDS: i64 = 1;

/// Renders nests and graphics to files and puts the files in their place on the clips
/// that show them, one source at a time.
pub(crate) struct RenderReplace {
    queue: VecDeque<Pending>,
    total: usize,
    running: Option<Running>,
}

struct Pending {
    original: SourceId,
    name: String,
    clips: Vec<ClipId>,
    /// How much of the source, from its start, to render.
    end: Ticks,
}

struct Running {
    pending: Pending,
    stage: Stage,
}

enum Stage {
    /// Making the folder and choosing a free name, off the UI thread.
    Naming(Receiver<std::io::Result<PathBuf>>),
    Rendering {
        job: ExportJob,
        path: PathBuf,
        frames_done: i64,
        frames_total: i64,
    },
    /// Reading the finished file back, as an import does.
    Probing(Receiver<Result<ImportMedia, MediaError>>),
}

impl Editor {
    /// The selected clips' sources that Render and Replace can render.
    fn renderable_selection(&self) -> Vec<Pending> {
        let sequence = self.timeline.sequence();
        let handle = Timebase::new(1, 1).map_or(Ticks::ZERO, |seconds| {
            Ticks(HANDLE_SECONDS).rescale(seconds, sequence.timebase)
        });
        let mut pending: Vec<Pending> = Vec::new();
        let selected = sequence.links_of(&self.view.selection);
        let clips = sequence
            .tracks()
            .filter(|(track, _)| track.kind == TrackKind::Video)
            .flat_map(|(_, track)| &track.clips)
            .filter(|clip| selected.contains(&clip.id) && clip.unrendered.is_none());
        for clip in clips {
            if let Some(entry) = pending.iter_mut().find(|p| p.original == clip.source) {
                entry.clips.push(clip.id);
                entry.end = entry.end.max(clip.source_out());
                continue;
            }
            let Some(item) = sequence.media_item(clip.source) else {
                continue;
            };
            if !sequence.can_render(clip.source) {
                continue;
            }
            pending.push(Pending {
                original: clip.source,
                name: item.name.clone(),
                clips: vec![clip.id],
                end: clip.source_out(),
            });
        }
        for entry in &mut pending {
            let length = sequence
                .media_item(entry.original)
                .map_or(entry.end, |item| item.duration);
            entry.end = (entry.end + handle).min(length);
        }
        pending
    }

    pub(crate) fn can_render_and_replace(&self) -> bool {
        self.render_replace.is_none() && !self.renderable_selection().is_empty()
    }

    pub(crate) fn can_restore_unrendered(&self) -> bool {
        let sequence = self.timeline.sequence();
        sequence
            .links_of(&self.view.selection)
            .iter()
            .filter_map(|&id| sequence.clip(id))
            .any(|(_, clip)| clip.unrendered.is_some())
    }

    pub(crate) fn render_and_replace(&mut self) {
        let queue: VecDeque<Pending> = self.renderable_selection().into();
        if queue.is_empty() {
            self.notice("Select clips of nested sequences or graphics to render and replace");
            return;
        }
        self.render_replace = Some(RenderReplace {
            total: queue.len(),
            queue,
            running: None,
        });
    }

    pub(crate) fn restore_unrendered(&mut self) {
        let clips = self.view.selection.clone();
        if !self.apply(RestoreUnrendered { clips }) {
            self.notice("None of the selected clips were rendered and replaced");
        }
    }

    pub(crate) fn poll_render_replace(&mut self, ctx: &egui::Context) {
        let Some(work) = &mut self.render_replace else {
            return;
        };
        ctx.request_repaint_after(crate::export_progress::REFRESH);
        let running = match work.running.take() {
            Some(running) => running,
            None => {
                let Some(pending) = work.queue.pop_front() else {
                    self.render_replace = None;
                    return;
                };
                let folder = self.rendered_folder();
                let name = pending.name.clone();
                let (sender, receiver) = channel::bounded(1);
                let spawned = thread::Builder::new()
                    .name("render-replace".to_owned())
                    .spawn(move || {
                        let _ = sender.send(free_path(&folder, &name));
                    });
                if let Err(error) = spawned {
                    self.render_replace = None;
                    self.notice(format!("Cannot render: {error}"));
                    return;
                }
                Running {
                    pending,
                    stage: Stage::Naming(receiver),
                }
            }
        };
        match self.advance(running) {
            Ok(Some(running)) => {
                if let Some(work) = &mut self.render_replace {
                    work.running = Some(running);
                }
            }
            Ok(None) => {}
            Err(error) => {
                self.render_replace = None;
                self.notice(format!("Cannot render and replace: {error}"));
            }
        }
    }

    /// Moves `running` on as far as its work allows; `None` once its source is done.
    fn advance(&mut self, mut running: Running) -> Result<Option<Running>, AppError> {
        match &mut running.stage {
            Stage::Naming(receiver) => {
                let Ok(named) = receiver.try_recv() else {
                    return Ok(Some(running));
                };
                let path = named?;
                let job = self.start_render(&running.pending, path.clone())?;
                running.stage = Stage::Rendering {
                    job,
                    path,
                    frames_done: 0,
                    frames_total: 0,
                };
            }
            Stage::Rendering {
                job,
                path,
                frames_done,
                frames_total,
            } => {
                for event in job.poll() {
                    match event {
                        ExportEvent::Progress {
                            frames_done: done,
                            frames_total: total,
                        } => (*frames_done, *frames_total) = (done, total),
                        ExportEvent::Warning(warning) => self.notice(format!("Render: {warning}")),
                        ExportEvent::Finished => {
                            let sequence = self.timeline.sequence();
                            let (timebase, rate) = (sequence.timebase, sequence.format.frame_rate);
                            running.stage = Stage::Probing(probe(path.clone(), timebase, rate));
                            break;
                        }
                        ExportEvent::Cancelled => return Ok(None),
                        ExportEvent::Failed(error) => return Err(error.into()),
                    }
                }
            }
            Stage::Probing(receiver) => {
                let Ok(probed) = receiver.try_recv() else {
                    return Ok(Some(running));
                };
                let import = probed?;
                self.replace_with_render(running.pending, import);
                return Ok(None);
            }
        }
        Ok(Some(running))
    }

    fn start_render(&self, pending: &Pending, path: PathBuf) -> Result<ExportJob, AppError> {
        let sequence = self.timeline.sequence();
        let view = sequence
            .render_view(pending.original)
            .ok_or_else(|| AppError::NotRenderable(pending.name.clone()))?;
        let render_state = self.render_state.as_ref().ok_or(AppError::NoRenderer)?;
        let has_audio = sequence
            .media_item(pending.original)
            .is_some_and(|item| item.has_audio);
        let format = view.format;
        let settings = ExportSettings {
            path,
            video: Some(VideoExport {
                encoding: VideoEncoding::Movie(VideoCodec::ProRes4444, RateControl::Quality(18)),
                size: (format.width, format.height),
            }),
            audio: has_audio.then_some(AudioSettings {
                codec: AudioCodec::Pcm24,
                sample_rate: 48_000,
                bits_per_second: 0,
            }),
            range: Some(Ticks::ZERO..pending.end),
        };
        ExportJob::start(
            Arc::new(view),
            render_state.device.clone(),
            render_state.queue.clone(),
            settings,
        )
        .map_err(AppError::from)
    }

    fn replace_with_render(&mut self, pending: Pending, import: ImportMedia) {
        if !self.apply(import) {
            return;
        }
        let Some(rendered) = self.timeline.sequence().media.last().map(|item| item.id) else {
            return;
        };
        let replace = RenderAndReplace {
            clips: pending.clips,
            original: pending.original,
            rendered,
        };
        if !self.apply(replace) {
            self.notice(format!(
                "The clips of {} changed while rendering; its render is in the project",
                pending.name
            ));
        }
    }

    /// Next to the project when it has been saved, as Premiere keeps rendered media.
    fn rendered_folder(&self) -> PathBuf {
        let beside = self
            .project_path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_owned);
        beside
            .unwrap_or_else(crate::project_store::app_folder)
            .join("Rendered Media")
    }
}

/// A path in `folder`, made if need be, named after `name` and not yet taken.
fn free_path(folder: &Path, name: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(folder)?;
    let stem: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let stem = format!("{} Rendered", stem.trim());
    (1..)
        .map(|n| match n {
            1 => folder.join(format!("{stem}.mov")),
            n => folder.join(format!("{stem} {n}.mov")),
        })
        .find(|path| !path.exists())
        .ok_or_else(|| std::io::Error::other("no free file name"))
}

/// The file at `path` read as an import would; the receiver disconnects without an
/// answer if the thread can't start.
fn probe(
    path: PathBuf,
    timebase: Timebase,
    rate: FrameRate,
) -> Receiver<Result<ImportMedia, MediaError>> {
    let (sender, receiver) = channel::bounded(1);
    let _ = thread::Builder::new()
        .name("render-replace-probe".to_owned())
        .spawn(move || {
            let _ = sender.send(crate::import::probe(&path, timebase, rate));
        });
    receiver
}

/// Progress of Render and Replace, with a button to stop it.
pub(crate) fn window(editor: &mut Editor, ctx: &egui::Context) {
    let Some(work) = &editor.render_replace else {
        return;
    };
    let done = work.total - work.queue.len() - usize::from(work.running.is_some());
    let (name, part) = match &work.running {
        Some(Running {
            pending,
            stage:
                Stage::Rendering {
                    frames_done,
                    frames_total,
                    ..
                },
        }) => (
            pending.name.as_str(),
            *frames_done as f32 / (*frames_total).max(1) as f32,
        ),
        Some(Running { pending, .. }) => (pending.name.as_str(), 0.0),
        None => ("", 0.0),
    };
    let fraction = (done as f32 + part) / work.total.max(1) as f32;
    let mut cancel = false;
    crate::dialog::window(ctx, "Render and Replace").show(ctx, |ui| {
        ui.label(format!("Rendering {name}"));
        ui.add(
            ProgressBar::new(fraction)
                .show_percentage()
                .desired_width(260.0),
        );
        ui.label(RichText::new("Clips show the rendered file once it is done.").weak());
        crate::dialog::footer(ui, |ui| {
            cancel =
                ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape));
        });
    });
    if cancel {
        // Dropping the export cancels it and removes its partial file.
        editor.render_replace = None;
    }
}
