use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam::channel::{Receiver, RecvTimeoutError, Sender, TrySendError};
use timeline::{Sequence, SourceId, Ticks, TrackRef};

use crate::command::{coalesce, Coalesced, Command, Event};
use crate::frame_picker::TaggedFrame;
use crate::track_video::TrackVideo;
use crate::{ComposedFrame, Layer, PlaybackError};

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
            warned: Vec::new(),
        }
        .run();
    })
}

struct VideoWorker {
    sequence: Arc<Sequence>,
    channels: VideoChannels,
    tracks: Vec<TrackVideo>,
    generation: u64,
    /// Index of the next frame to compose, at the sequence frame rate.
    next_frame: i64,
    pending: Option<ComposedFrame>,
    finished: bool,
    /// Media already reported as unreadable.
    warned: Vec<SourceId>,
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
            let Some(Coalesced { sequence, seek }) = coalesce(command, &self.channels.commands)
            else {
                return;
            };
            if let Some(sequence) = sequence {
                self.sequence = sequence;
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
        while self.channels.stale_frames.try_recv().is_ok() {}
        let rate = self.sequence.format.frame_rate;
        self.next_frame = rate.frame_at(position, self.sequence.timebase);
        self.tracks.resize_with(self.sequence.video.len(), || {
            TrackVideo::new(self.sequence.timebase)
        });
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
        if pts >= sequence.duration() {
            self.finished = true;
            let _ = self.channels.events.send(Event::Ended {
                generation: self.generation,
            });
            return;
        }
        let mut layers = Vec::new();
        for ((index, track), decoder) in sequence.video.iter().enumerate().zip(&mut self.tracks) {
            if track.muted {
                continue;
            }
            let Some(clip) = track.clip_at(pts) else {
                continue;
            };
            let Some(media) = sequence.media_item(clip.source) else {
                continue;
            };
            match decoder.frame_at(media, clip.source_time(pts)) {
                Ok(Some(frame)) => layers.push(Layer {
                    track: TrackRef::video(index),
                    frame,
                }),
                Ok(None) => {}
                Err(error) => {
                    decoder.reset();
                    if !self.warned.contains(&media.id) {
                        self.warned.push(media.id);
                        let _ = self
                            .channels
                            .events
                            .send(Event::Warning(PlaybackError::Media(error)));
                    }
                }
            }
        }
        self.pending = Some(ComposedFrame { pts, layers });
        self.next_frame += 1;
    }
}
