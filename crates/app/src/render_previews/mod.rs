mod files;

use std::collections::{HashSet, VecDeque};
use std::ops::Range;
use std::sync::Arc;

use egui::{Key, ProgressBar, RichText};
use media::{RateControl, VideoCodec};
use playback::{ExportEvent, ExportJob, ExportSettings, Preview, VideoEncoding, VideoExport};
use timeline::{render_segments, segment_content, Sequence, Ticks};

use crate::editor::Editor;
use files::{PreviewFiles, Reply};

/// A segment to render, named by the fingerprint of what it shows.
type Segment = (Range<Ticks>, u64);

/// Render previews of the costly parts of the sequence, as Premiere keeps: files that
/// play in their place, found again by what they show for as long as it's unchanged.
pub(crate) struct RenderPreviews {
    files: PreviewFiles,
    /// Fingerprints with a preview on disk, once the folder has been read.
    rendered: HashSet<u64>,
    /// The costly segments as of `revision`.
    segments: Vec<Segment>,
    revision: Option<u64>,
    /// What the program player was last told to play.
    sent: Option<Vec<Preview>>,
    job: Option<RenderJob>,
}

struct RenderJob {
    queue: VecDeque<Range<Ticks>>,
    total: usize,
    running: Option<Running>,
}

struct Running {
    job: ExportJob,
    fingerprint: u64,
    frames_done: i64,
    frames_total: i64,
}

impl RenderPreviews {
    pub(crate) fn new() -> std::io::Result<Self> {
        let folder = crate::project_store::app_folder().join("Preview Files");
        Ok(Self {
            files: PreviewFiles::new(folder)?,
            rendered: HashSet::new(),
            segments: Vec::new(),
            revision: None,
            sent: None,
            job: None,
        })
    }

    /// For a timeline replaced by another, whose revisions start again.
    pub(crate) fn forget(&mut self) {
        self.revision = None;
        self.sent = None;
        self.job = None;
    }

    /// The costly segments, each with whether a preview of it is ready.
    pub(crate) fn bar(&self) -> impl Iterator<Item = (Range<Ticks>, bool)> + '_ {
        self.segments
            .iter()
            .map(|(range, fingerprint)| (range.clone(), self.rendered.contains(fingerprint)))
    }

    fn previews(&self) -> Vec<Preview> {
        self.segments
            .iter()
            .filter(|(_, fingerprint)| self.rendered.contains(fingerprint))
            .map(|(range, fingerprint)| Preview {
                range: range.clone(),
                path: self.files.path(*fingerprint),
            })
            .collect()
    }
}

/// What a segment shows, as a number; `None` when it can't be written out.
fn fingerprint(sequence: &Sequence, range: &Range<Ticks>) -> Option<u64> {
    let content = segment_content(sequence, range);
    project::fingerprint(&(content, range.start, range.end)).ok()
}

impl Editor {
    pub(crate) fn poll_render_previews(&mut self, ctx: &egui::Context) {
        let Some(previews) = &mut self.render_previews else {
            return;
        };
        let mut notices = Vec::new();
        for reply in previews.files.poll() {
            match reply {
                Reply::Rendered(rendered) => previews.rendered = rendered,
                Reply::Kept(fingerprint) => {
                    previews.rendered.insert(fingerprint);
                }
                Reply::Deleted => previews.rendered.clear(),
                Reply::Failed(error) => notices.push(format!("Render files: {error}")),
            }
        }
        let revision = self.timeline.revision();
        if previews.revision != Some(revision) {
            let sequence = self.timeline.sequence();
            previews.revision = Some(revision);
            previews.segments = render_segments(sequence)
                .into_iter()
                .filter_map(|range| Some((range.clone(), fingerprint(sequence, &range)?)))
                .collect();
        }
        if previews.job.is_some() {
            ctx.request_repaint_after(crate::export_progress::REFRESH);
            notices.extend(self.advance_render());
        }
        let Some(previews) = &mut self.render_previews else {
            return;
        };
        // The multi-camera view shows each camera, not the program alone.
        let wanted = if self.multicam_layout.is_some() {
            Vec::new()
        } else {
            previews.previews()
        };
        if previews.sent.as_ref() != Some(&wanted) {
            if let Some(program) = &mut self.program {
                program.player.set_previews(wanted.clone());
            }
            previews.sent = Some(wanted);
        }
        self.view.render_bar = previews.bar().collect();
        for notice in notices {
            self.notice(notice);
        }
    }

    /// Renders the costly segments between the sequence's in and out marks, or all of
    /// it when it has none, that have no preview yet.
    pub(crate) fn render_in_to_out(&mut self) {
        let sequence = self.timeline.sequence();
        let range = sequence.marks.range(sequence.duration());
        let Some(previews) = &mut self.render_previews else {
            self.notice("Render previews are unavailable");
            return;
        };
        let in_range: Vec<_> = previews
            .segments
            .iter()
            .filter(|(segment, _)| segment.start < range.end && range.start < segment.end)
            .collect();
        let queue: VecDeque<Range<Ticks>> = in_range
            .iter()
            .filter(|(_, fingerprint)| !previews.rendered.contains(fingerprint))
            .map(|(segment, _)| segment.clone())
            .collect();
        let any_costly = !in_range.is_empty();
        if queue.is_empty() {
            self.notice(if !any_costly {
                "Nothing to render: this part of the sequence already plays in real time"
            } else {
                "Nothing to render: its previews are up to date"
            });
            return;
        }
        previews.job = Some(RenderJob {
            total: queue.len(),
            queue,
            running: None,
        });
    }

    /// Whether Delete Render Files has anything to delete or stop.
    pub(crate) fn has_render_files(&self) -> bool {
        self.render_previews
            .as_ref()
            .is_some_and(|previews| !previews.rendered.is_empty() || previews.job.is_some())
    }

    pub(crate) fn delete_render_files(&mut self) {
        if let Some(previews) = &mut self.render_previews {
            previews.job = None;
            // Windows can't remove the files while the player has them open.
            if let Some(program) = &mut self.program {
                program.player.set_previews(Vec::new());
            }
            previews.sent = Some(Vec::new());
            // Gone already, so they aren't handed back to the player before the reply.
            previews.rendered.clear();
            previews.files.delete_all();
        }
    }

    /// Starts the next segment or follows the one rendering; returns notices.
    fn advance_render(&mut self) -> Vec<String> {
        let sequence = self.timeline.sequence();
        let Some(previews) = &mut self.render_previews else {
            return Vec::new();
        };
        let Some(job) = &mut previews.job else {
            return Vec::new();
        };
        let Some(running) = &mut job.running else {
            let Some(range) = job.queue.pop_front() else {
                previews.job = None;
                return Vec::new();
            };
            // Named by what it shows now, which is what the render will show.
            let Some(fingerprint) = fingerprint(sequence, &range) else {
                return Vec::new();
            };
            let Some(render_state) = &self.render_state else {
                previews.job = None;
                return vec!["Cannot render: there is no GPU renderer".to_owned()];
            };
            let format = sequence.format;
            let settings = ExportSettings {
                path: previews.files.partial_path(fingerprint),
                video: Some(VideoExport {
                    encoding: VideoEncoding::Movie(VideoCodec::ProRes422, RateControl::Quality(18)),
                    size: (format.width, format.height),
                }),
                audio: None,
                range: Some(range),
            };
            let started = ExportJob::start(
                Arc::new(sequence.clone()),
                render_state.device.clone(),
                render_state.queue.clone(),
                settings,
            );
            return match started {
                Ok(export) => {
                    job.running = Some(Running {
                        job: export,
                        fingerprint,
                        frames_done: 0,
                        frames_total: 0,
                    });
                    Vec::new()
                }
                Err(error) => {
                    previews.job = None;
                    vec![format!("Cannot render: {error}")]
                }
            };
        };
        let mut notices = Vec::new();
        for event in running.job.poll() {
            match event {
                ExportEvent::Progress {
                    frames_done,
                    frames_total,
                } => {
                    running.frames_done = frames_done;
                    running.frames_total = frames_total;
                }
                ExportEvent::Warning(warning) => notices.push(format!("Render: {warning}")),
                ExportEvent::Finished => {
                    let fingerprint = running.fingerprint;
                    previews
                        .files
                        .keep(previews.files.partial_path(fingerprint), fingerprint);
                    job.running = None;
                    break;
                }
                ExportEvent::Cancelled => {
                    previews.job = None;
                    break;
                }
                ExportEvent::Failed(error) => {
                    previews.job = None;
                    notices.push(format!("Render failed: {error}"));
                    break;
                }
            }
        }
        notices
    }
}

/// Progress of a render, with a button to stop it.
pub(crate) fn window(editor: &mut Editor, ctx: &egui::Context) {
    let Some(job) = editor
        .render_previews
        .as_mut()
        .and_then(|previews| previews.job.as_mut())
    else {
        return;
    };
    let done = job.total - job.queue.len() - usize::from(job.running.is_some());
    let segment = job.running.as_ref().map_or(0.0, |running| {
        running.frames_done as f32 / running.frames_total.max(1) as f32
    });
    let fraction = (done as f32 + segment) / job.total.max(1) as f32;
    let mut cancel = false;
    crate::dialog::window(ctx, "Rendering").show(ctx, |ui| {
        let current = (done + 1).min(job.total);
        ui.label(format!("Rendering segment {current} of {}", job.total));
        ui.add(
            ProgressBar::new(fraction)
                .show_percentage()
                .desired_width(260.0),
        );
        ui.label(RichText::new("Previews play in place of the segments they cover.").weak());
        crate::dialog::footer(ui, |ui| {
            cancel =
                ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape));
        });
    });
    if cancel {
        // Dropping the export cancels it and removes its partial file.
        job.running = None;
        job.queue.clear();
        if let Some(previews) = &mut editor.render_previews {
            previews.job = None;
        }
    }
}
