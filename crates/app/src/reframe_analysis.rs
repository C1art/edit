use std::collections::VecDeque;
use std::ops::ControlFlow;
use std::thread;
use std::time::Duration;

use crossbeam::channel::{self, Receiver, TryRecvError};
use media::MediaError;
use timeline::{ClipId, SetClipLook, Ticks, TrackKind, VideoEffect};

use crate::editor::Editor;
use crate::scene_edits::source_span;

enum Event {
    Progress(f32),
    Done(Result<Vec<(Ticks, [f32; 2])>, MediaError>),
}

struct Running {
    clip: ClipId,
    progress: f32,
    events: Receiver<Event>,
}

/// Auto Reframe's analysis of clips' footage, one clip at a time on a worker, as a
/// reframed sequence asks for many at once. Dropping it stops the worker at its next
/// frame.
#[derive(Default)]
pub struct ReframeJobs {
    queue: VecDeque<ClipId>,
    running: Option<Running>,
}

impl Editor {
    /// Queues finding the subject through `clip`'s footage, for its Auto Reframe.
    pub(crate) fn analyze_reframe(&mut self, clip: ClipId) {
        let jobs = &mut self.reframe_jobs;
        let running = jobs.running.as_ref().is_some_and(|job| job.clip == clip);
        if !running && !jobs.queue.contains(&clip) {
            jobs.queue.push_back(clip);
        }
        self.start_reframe_analysis();
    }

    fn start_reframe_analysis(&mut self) {
        while self.reframe_jobs.running.is_none() {
            let Some(clip) = self.reframe_jobs.queue.pop_front() else {
                return;
            };
            let sequence = self.timeline.sequence();
            let Some((track, found)) = sequence.clip(clip) else {
                continue;
            };
            let item = self
                .timeline
                .project()
                .media_item(found.source)
                .filter(|item| item.is_file() && item.video.is_some());
            let (Some(item), TrackKind::Video) = (item, track.kind) else {
                self.notice("Auto Reframe needs a clip of video footage");
                continue;
            };
            let (path, range, timebase) =
                (item.path.clone(), source_span(found), sequence.timebase);
            let (sender, events) = channel::unbounded();
            let spawned = thread::Builder::new()
                .name("reframe analysis".to_owned())
                .spawn(move || {
                    let mut reported = -1.0;
                    let progress = |done: f32| {
                        if done - reported < 0.01 {
                            return ControlFlow::Continue(());
                        }
                        reported = done;
                        match sender.send(Event::Progress(done)) {
                            Ok(()) => ControlFlow::Continue(()),
                            Err(_) => ControlFlow::Break(()),
                        }
                    };
                    let centers = media::track_subject(&path, timebase, range, progress);
                    let _ = sender.send(Event::Done(centers));
                });
            match spawned {
                Ok(_) => {
                    self.reframe_jobs.running = Some(Running {
                        clip,
                        progress: 0.0,
                        events,
                    });
                }
                Err(error) => {
                    self.notice(format!("Cannot analyze for Auto Reframe: {error}"));
                    return;
                }
            }
        }
    }

    /// How much of `clip`'s footage has been analyzed for Auto Reframe, while it is
    /// being or waits to be.
    pub(crate) fn reframe_analysis_progress(&self, clip: ClipId) -> Option<f32> {
        let jobs = &self.reframe_jobs;
        match &jobs.running {
            Some(job) if job.clip == clip => Some(job.progress),
            _ => jobs.queue.contains(&clip).then_some(0.0),
        }
    }

    /// Gives the clip's Auto Reframe its subject once the worker has found it, and
    /// starts on the next clip.
    pub(crate) fn poll_reframe_analysis(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.reframe_jobs.running else {
            return;
        };
        let centers = loop {
            match job.events.try_recv() {
                Ok(Event::Progress(done)) => job.progress = done,
                Ok(Event::Done(centers)) => break Some(centers),
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(100));
                    return;
                }
                Err(TryRecvError::Disconnected) => break None,
            }
        };
        let clip = job.clip;
        self.reframe_jobs.running = None;
        match centers {
            Some(Ok(centers)) => self.reframe(clip, &centers),
            Some(Err(error)) => self.notice(format!("Cannot analyze for Auto Reframe: {error}")),
            None => {}
        }
        self.start_reframe_analysis();
    }

    fn reframe(&mut self, clip: ClipId, centers: &[(Ticks, [f32; 2])]) {
        let sequence = self.timeline.sequence();
        // The clip or its effect may have gone while the footage was read.
        let Some((_, found)) = sequence.clip(clip) else {
            return;
        };
        let mut look = found.look.clone();
        let reframe = look
            .effects
            .iter_mut()
            .rev()
            .find_map(|effect| match effect {
                VideoEffect::AutoReframe(reframe) => Some(reframe),
                _ => None,
            });
        let Some(reframe) = reframe else {
            return;
        };
        reframe.analyzed(centers, sequence.timebase);
        self.apply(SetClipLook { clip, look });
    }
}
