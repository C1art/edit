use std::ops::ControlFlow;
use std::thread;
use std::time::Duration;

use crossbeam::channel::{self, Receiver, TryRecvError};
use media::{MediaError, Motion};
use timeline::{CameraPath, ClipId, SetClipLook, TrackKind, VideoEffect};

use crate::editor::Editor;
use crate::scene_edits::source_span;

enum Event {
    Progress(f32),
    Done(Result<Motion, MediaError>),
}

/// Warp Stabilizer's analysis of one clip's footage on a worker. Dropping it stops the
/// worker at its next frame.
pub struct MotionJob {
    clip: ClipId,
    progress: f32,
    events: Receiver<Event>,
}

impl Editor {
    /// Starts tracking the camera through `clip`'s footage, for its Warp Stabilizer or Rolling Shutter Repair.
    pub(crate) fn analyze_motion(&mut self, clip: ClipId) {
        if let Some(job) = &self.motion_job {
            if job.clip != clip {
                self.notice("Another clip's motion is still being analyzed");
            }
            return;
        }
        let sequence = self.timeline.sequence();
        let Some((track, found)) = sequence.clip(clip) else {
            return;
        };
        let item = self
            .timeline
            .project()
            .media_item(found.source)
            .filter(|item| item.is_file() && item.video.is_some());
        let (Some(item), TrackKind::Video) = (item, track.kind) else {
            self.notice("Motion analysis needs a clip of video footage");
            return;
        };
        let (path, range, timebase) = (item.path.clone(), source_span(found), sequence.timebase);
        let (sender, events) = channel::unbounded();
        let spawned = thread::Builder::new()
            .name("motion analysis".to_owned())
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
                let motion = media::track_motion(&path, timebase, range, progress);
                let _ = sender.send(Event::Done(motion));
            });
        match spawned {
            Ok(_) => {
                self.motion_job = Some(MotionJob {
                    clip,
                    progress: 0.0,
                    events,
                });
            }
            Err(error) => self.notice(format!("Cannot analyze motion: {error}")),
        }
    }

    /// How much of `clip`'s motion has been analyzed, while it is being.
    pub(crate) fn motion_analysis_progress(&self, clip: ClipId) -> Option<f32> {
        self.motion_job
            .as_ref()
            .filter(|job| job.clip == clip)
            .map(|job| job.progress)
    }

    /// Gives the clip's stabilizer the camera path once the worker has tracked it.
    pub(crate) fn poll_motion_analysis(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.motion_job else {
            return;
        };
        let motion = loop {
            match job.events.try_recv() {
                Ok(Event::Progress(done)) => job.progress = done,
                Ok(Event::Done(motion)) => break motion,
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(100));
                    return;
                }
                Err(TryRecvError::Disconnected) => {
                    self.motion_job = None;
                    return;
                }
            }
        };
        let clip = job.clip;
        self.motion_job = None;
        let motion = match motion {
            Ok(motion) => motion,
            Err(error) => {
                self.notice(format!("Cannot analyze motion: {error}"));
                return;
            }
        };
        let sequence = self.timeline.sequence();
        // The clip or its stabilizer may have gone while the footage was read.
        let Some((_, found)) = sequence.clip(clip) else {
            return;
        };
        let mut look = found.look.clone();
        let mut notice = None;
        for effect in &mut look.effects {
            match effect {
                VideoEffect::WarpStabilizer(stabilizer) => {
                    stabilizer.analyzed(motion.size, &motion.steps, sequence.timebase);
                    let scale = f64::from(stabilizer.auto_scale) / 10.0;
                    notice = Some(format!("Stabilized; Auto-scale {scale:.1}%"));
                }
                VideoEffect::RollingShutterRepair(repair) => {
                    repair.path = Some(CameraPath::tracked(motion.size, &motion.steps));
                    notice.get_or_insert_with(|| "Rolling shutter repaired".to_owned());
                }
                _ => {}
            }
        }
        let Some(notice) = notice else {
            return;
        };
        if self.apply(SetClipLook { clip, look }) {
            self.notice(notice);
        }
    }
}
