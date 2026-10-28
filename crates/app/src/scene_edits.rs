use std::ops::{ControlFlow, Range};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crossbeam::channel::{self, Receiver, TryRecvError};
use media::MediaError;
use timeline::{AddEdits, Clip, ClipId, Ticks, TrackKind};

use crate::editor::Editor;

/// Where new shots start in each clip's footage, in source time.
type Found = Vec<(ClipId, Result<Vec<Ticks>, MediaError>)>;

/// Scene edit detection reading the selected clips' footage on a worker.
pub struct SceneJob {
    found: Receiver<Found>,
}

impl Editor {
    /// Starts finding where the shots change in the selected video clips, to cut them
    /// there once found.
    pub(crate) fn detect_scene_edits(&mut self) {
        if self.scene_job.is_some() {
            self.notice("Scene edits are still being found");
            return;
        }
        let sequence = self.timeline.sequence();
        let pool = self.timeline.project();
        let timebase = sequence.timebase;
        let work: Vec<(ClipId, PathBuf, Range<Ticks>)> = self
            .view
            .selection
            .iter()
            .filter_map(|&id| {
                let (track, clip) = sequence.clip(id)?;
                if track.kind != TrackKind::Video || clip.hold {
                    return None;
                }
                let item = pool
                    .media_item(clip.source)
                    .filter(|item| item.is_file() && item.video.is_some())?;
                Some((id, item.path.clone(), source_span(clip)))
            })
            .collect();
        if work.is_empty() {
            self.notice("Select video clips to find their scene edits");
            return;
        }
        let clips = work.len();
        let (reply, found) = channel::bounded(1);
        let spawned = thread::Builder::new()
            .name("scene edit detection".to_owned())
            .spawn(move || {
                let found = work
                    .into_iter()
                    .map(|(id, path, range)| {
                        let keep_going = |_| ControlFlow::Continue(());
                        (id, media::detect_scenes(&path, timebase, range, keep_going))
                    })
                    .collect();
                let _ = reply.send(found);
            });
        match spawned {
            Ok(_) => {
                self.scene_job = Some(SceneJob { found });
                self.notice(format!("Finding scene edits in {clips} clip(s)…"));
            }
            Err(error) => self.notice(format!("Cannot find scene edits: {error}")),
        }
    }

    /// Cuts the clips where their shots change once the worker has found where.
    pub(crate) fn poll_scene_edits(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.scene_job else {
            return;
        };
        let found = match job.found.try_recv() {
            Ok(found) => found,
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(200));
                return;
            }
            Err(TryRecvError::Disconnected) => {
                self.scene_job = None;
                return;
            }
        };
        self.scene_job = None;
        let sequence = self.timeline.sequence();
        let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
        let mut cuts = Vec::new();
        let mut failed = Vec::new();
        for (id, shots) in found {
            let shots = match shots {
                Ok(shots) => shots,
                Err(error) => {
                    failed.push(error.to_string());
                    continue;
                }
            };
            // The clip may have changed while its footage was read.
            let Some((_, clip)) = sequence.clip(id) else {
                continue;
            };
            for shot in shots {
                // Played backwards, a shot starts after its first frame has played.
                let first = if clip.reverse { shot - Ticks(1) } else { shot };
                let at = clip.position_of(first);
                let at = rate.frame_start(rate.frame_at(at, timebase), timebase);
                if clip.start < at && at < clip.end() {
                    cuts.push((id, at));
                }
            }
        }
        let count = cuts.len();
        if count == 0 {
            self.notice("No scene edits found");
        } else if self.apply(AddEdits { cuts }) {
            self.notice(format!("Added {count} scene edit(s)"));
        }
        if let Some(error) = failed.first() {
            self.notice(format!("Cannot read some footage: {error}"));
        }
    }
}

/// The source a clip shows, from its earliest time to its latest.
fn source_span(clip: &Clip) -> Range<Ticks> {
    let ends = [
        clip.source_time(clip.start),
        clip.source_time(clip.end() - Ticks(1)),
    ];
    let (low, high) = (ends[0].min(ends[1]), ends[0].max(ends[1]));
    low..high + Ticks(1)
}
