use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam::channel::{Receiver, RecvTimeoutError, Sender, TrySendError};
use timeline::{Sequence, SourceId, Ticks};

use crate::command::{coalesce, Coalesced, Command, Event};
use crate::frame_picker::TaggedFrame;
use crate::previews::PreviewPictures;
use crate::{compose, ComposedFrame, Layer, Resolution};

/// How long the worker waits for room in the frame queue before checking commands again.
const BACKOFF: Duration = Duration::from_millis(4);

pub(crate) struct VideoChannels {
    pub(crate) commands: Receiver<Command>,
    pub(crate) events: Sender<Event>,
    pub(crate) frames: Sender<TaggedFrame>,
    /// The worker's own handle on the frame queue, for discarding frames a seek made stale.
    pub(crate) stale_frames: Receiver<TaggedFrame>,
    /// Without an audio worker, video reports seeks so the player can start its clock.
    pub(crate) reports_seeks: bool,
}

pub(crate) fn spawn(
    sequence: Arc<Sequence>,
    channels: VideoChannels,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new().name("video".into()).spawn(move || {
        VideoWorker {
            tracks: Vec::new(),
            sequence,
            channels,
            generation: 0,
            next_frame: 0,
            pending: None,
            finished: true,
            composed: false,
            warned: Vec::new(),
            resolution: Resolution::Full,
            previews: None,
        }
        .run();
    })
}

struct VideoWorker {
    sequence: Arc<Sequence>,
    channels: VideoChannels,
    tracks: Vec<compose::TrackDecoders>,
    generation: u64,
    /// Index of the next frame to compose, at the sequence frame rate.
    next_frame: i64,
    pending: Option<ComposedFrame>,
    finished: bool,
    /// Whether a frame went out since the last seek.
    composed: bool,
    /// Media already reported as unreadable.
    warned: Vec<SourceId>,
    resolution: Resolution,
    /// Rendered previews, played in place of what they cover.
    previews: Option<PreviewPictures>,
}

impl VideoWorker {
    fn run(mut self) {
        loop {
            let command = if let Some(frame) = self.pending.take() {
                let tagged = TaggedFrame {
                    generation: self.generation,
                    frame,
                };
                match self.channels.frames.try_send(tagged) {
                    Ok(()) => continue,
                    Err(TrySendError::Full(tagged)) => self.pending = Some(tagged.frame),
                    Err(TrySendError::Disconnected(_)) => return,
                }
                match self.channels.commands.recv_timeout(BACKOFF) {
                    Ok(command) => command,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            } else if !self.finished {
                match self.channels.commands.try_recv() {
                    Ok(command) => command,
                    Err(crossbeam::channel::TryRecvError::Empty) => {
                        self.compose_next();
                        continue;
                    }
                    Err(crossbeam::channel::TryRecvError::Disconnected) => return,
                }
            } else {
                match self.channels.commands.recv() {
                    Ok(command) => command,
                    Err(_) => return,
                }
            };
            let Some(Coalesced {
                sequence,
                seek,
                resolution,
                previews,
            }) = coalesce(command, &self.channels.commands)
            else {
                return;
            };
            if let Some(resolution) = resolution {
                self.resolution = resolution;
            }
            if let Some(sequence) = sequence {
                self.sequence = sequence;
            }
            if let Some(previews) = previews {
                self.previews = PreviewPictures::new(&previews, &self.sequence);
            }
            if let Some((position, generation)) = seek {
                self.seek(position, generation);
            }
        }
    }

    fn seek(&mut self, position: Ticks, generation: u64) {
        self.generation = generation;
        self.pending = None;
        self.finished = false;
        self.composed = false;
        while self.channels.stale_frames.try_recv().is_ok() {}
        let rate = self.sequence.format.frame_rate;
        self.next_frame = rate.frame_at(position, self.sequence.timebase);
        if self.channels.reports_seeks {
            let _ = self.channels.events.send(Event::Seeked {
                generation,
                frames_played: 0,
            });
        }
    }

    fn compose_next(&mut self) {
        let sequence = Arc::clone(&self.sequence);
        let pts = sequence
            .format
            .frame_rate
            .frame_start(self.next_frame, sequence.timebase);
        // A seek to the end shows the last frame, or nothing in an empty sequence, so
        // the picture from before the seek doesn't linger, as after undoing every edit.
        if pts >= sequence.duration() && !self.composed {
            let last = sequence.duration() - Ticks(1);
            let shown = (last >= Ticks::ZERO).then(|| {
                let rate = sequence.format.frame_rate;
                rate.frame_start(rate.frame_at(last, sequence.timebase), sequence.timebase)
            });
            self.compose(&sequence, shown);
            return;
        }
        if pts >= sequence.duration() {
            self.finished = true;
            let _ = self.channels.events.send(Event::Ended {
                generation: self.generation,
            });
            return;
        }
        self.compose(&sequence, Some(pts));
        self.next_frame += 1;
    }

    /// Queues the frame at `pts`, or an empty one for `None`.
    fn compose(&mut self, sequence: &Sequence, pts: Option<Ticks>) {
        let mut warnings = Vec::new();
        let previewed = pts.and_then(|pts| self.previewed(pts));
        let layers = previewed.unwrap_or_else(|| {
            let Some(pts) = pts else {
                return Vec::new();
            };
            compose::layers_at(
                sequence,
                pts,
                &mut self.tracks,
                &mut self.warned,
                &mut warnings,
                self.resolution,
            )
        });
        for warning in warnings {
            let _ = self.channels.events.send(Event::Warning(warning));
        }
        self.pending = Some(ComposedFrame {
            pts: pts.unwrap_or(Ticks::ZERO),
            layers,
        });
        self.composed = true;
    }

    /// The preview's picture where one covers `pts`. A preview that can't be read is
    /// dropped, and the sequence composed from then on.
    fn previewed(&mut self, pts: Ticks) -> Option<Vec<Layer>> {
        let previews = self
            .previews
            .as_mut()
            .filter(|previews| previews.covers(pts))?;
        let layers = previews.layers_at(pts, self.resolution);
        if layers.is_none() {
            self.previews = None;
        }
        layers
    }
}
