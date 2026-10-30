use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam::channel::{self, Receiver, TryRecvError};
use media::{MediaError, RegionTrack, TrackDirection, TrackedFrame};
use timeline::{ClipId, Look, MaskRef, SetClipLook, Similarity, Ticks, TrackKind};

use crate::editor::Editor;
use crate::effects::{commit_look, keyframe_time, preview_look};
use crate::scene_edits::source_span;

/// How far one press tracks a mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrackSpan {
    OneFrame,
    ToEnd,
}

enum Event {
    Frame(TrackedFrame),
    Done(Result<(), MediaError>),
}

/// A clip's mask being tracked through its footage on a worker, keyed once it stops.
pub struct MaskTrackJob {
    clip: ClipId,
    mask: MaskRef,
    from: Ticks,
    poses: Vec<(Ticks, Similarity)>,
    size: [u32; 2],
    progress: f32,
    events: Receiver<Event>,
    stop: Arc<AtomicBool>,
}

impl Editor {
    /// Starts following the picture under `clip`'s `mask` from the playhead.
    pub(crate) fn track_mask(
        &mut self,
        clip: ClipId,
        mask: MaskRef,
        direction: TrackDirection,
        span: TrackSpan,
    ) {
        if self.mask_track_job.is_some() {
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
            .filter(|item| item.is_file());
        let video = item.and_then(|item| item.video.map(|video| (item, video)));
        let (Some((item, video)), TrackKind::Video) = (video, track.kind) else {
            self.notice("Mask tracking needs a clip of video footage");
            return;
        };
        let Some(shape) = found.look.mask(mask) else {
            return;
        };
        let from = keyframe_time(self, found);
        let request = RegionTrack {
            within: source_span(found),
            start: from,
            direction,
            frames: match span {
                TrackSpan::OneFrame => Some(1),
                TrackSpan::ToEnd => None,
            },
            region: shape.region_at(from, [video.width, video.height]),
            tracking: shape.tracking,
        };
        if request.region.len() < 3 {
            self.notice("Close the mask's path before tracking it");
            return;
        }
        let (path, timebase) = (item.path.clone(), sequence.timebase);
        let (sender, events) = channel::unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let spawned = thread::Builder::new()
            .name("mask tracking".to_owned())
            .spawn(move || {
                let result = media::track_region(&path, timebase, &request, |frame| {
                    if stopped.load(Ordering::Relaxed) {
                        return ControlFlow::Break(());
                    }
                    match sender.send(Event::Frame(frame)) {
                        Ok(()) => ControlFlow::Continue(()),
                        Err(_) => ControlFlow::Break(()),
                    }
                });
                let _ = sender.send(Event::Done(result));
            });
        match spawned {
            Ok(_) => {
                self.mask_track_job = Some(MaskTrackJob {
                    clip,
                    mask,
                    from,
                    poses: Vec::new(),
                    size: [video.width, video.height],
                    progress: 0.0,
                    events,
                    stop,
                });
            }
            Err(error) => self.notice(format!("Cannot track the mask: {error}")),
        }
    }

    /// Which of `clip`'s masks is being tracked and how far along, while one is.
    pub(crate) fn mask_tracking(&self, clip: ClipId) -> Option<(MaskRef, f32)> {
        self.mask_track_job
            .as_ref()
            .filter(|job| job.clip == clip)
            .map(|job| (job.mask, job.progress))
    }

    /// Stops tracking, keeping the frames tracked so far.
    pub(crate) fn stop_mask_tracking(&self) {
        if let Some(job) = &self.mask_track_job {
            job.stop.store(true, Ordering::Relaxed);
        }
    }

    /// Shows the mask following the picture as frames are tracked, and keys it once the
    /// tracking stops.
    pub(crate) fn poll_mask_tracking(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.mask_track_job else {
            return;
        };
        let mut tracked = false;
        let result = loop {
            match job.events.try_recv() {
                Ok(Event::Frame(frame)) => {
                    job.poses.push((frame.time, frame.pose));
                    job.size = frame.size;
                    job.progress = frame.progress;
                    tracked = true;
                }
                Ok(Event::Done(result)) => break Some(result),
                Err(TryRecvError::Empty) => break None,
                Err(TryRecvError::Disconnected) => break Some(Ok(())),
            }
        };
        let Some(result) = result else {
            if tracked {
                self.show_tracked_mask();
            }
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        };
        if let Err(error) = result {
            self.notice(format!("Cannot track the mask further: {error}"));
        }
        let Some((edit, last)) = self.tracked_look() else {
            self.mask_track_job = None;
            return;
        };
        self.mask_track_job = None;
        commit_look(self, edit);
        self.seek_program(last);
    }

    /// Moves the playhead to the latest tracked frame, showing the mask there.
    fn show_tracked_mask(&mut self) {
        let Some((edit, last)) = self.tracked_look() else {
            return;
        };
        self.seek_program(last);
        preview_look(self, edit);
    }

    /// The tracked clip's look with the mask keyed along the frames tracked so far, and
    /// the sequence position of the latest of them.
    fn tracked_look(&self) -> Option<(SetClipLook, Ticks)> {
        let job = self.mask_track_job.as_ref()?;
        let (last, _) = job.poses.last()?;
        let (_, clip) = self.timeline.sequence().clip(job.clip)?;
        let mut look: Look = clip.look.clone();
        look.mask_mut(job.mask)?
            .follow(job.from, &job.poses, job.size);
        let position = clip
            .position_of(*last)
            .clamp(clip.start, clip.end() - Ticks(1));
        Some((
            SetClipLook {
                clip: job.clip,
                look,
            },
            position,
        ))
    }
}
