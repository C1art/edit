use std::sync::Arc;
use std::thread;

use crossbeam::channel::{self, Receiver, Sender};
use timeline::{Sequence, SourceId, Ticks};

use crate::{compose, ComposedFrame, PlaybackError, Resolution};

/// Composes a sequence's pictures at a few times away from the playhead on a worker, as
/// the trim monitor shows them. Only the newest request is worked on; ones that were
/// still waiting are dropped, since a drag makes many a second.
pub struct FrameProbe {
    requests: Sender<(Arc<Sequence>, Vec<Ticks>)>,
    frames: Receiver<Vec<ComposedFrame>>,
}

impl FrameProbe {
    pub fn new() -> Result<Self, PlaybackError> {
        let (requests, received) = channel::unbounded::<(Arc<Sequence>, Vec<Ticks>)>();
        let (sender, frames) = channel::unbounded();
        thread::Builder::new()
            .name("frame probe".into())
            .spawn(move || {
                let mut tracks = Vec::new();
                let mut warned: Vec<SourceId> = Vec::new();
                while let Ok(mut request) = received.recv() {
                    request = received.try_iter().last().unwrap_or(request);
                    let (sequence, times) = request;
                    let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
                    let composed = times
                        .into_iter()
                        .map(|time| {
                            let pts = rate.frame_start(rate.frame_at(time, timebase), timebase);
                            let layers = compose::layers_at(
                                &sequence,
                                pts,
                                &mut tracks,
                                &mut warned,
                                // Playback reports unreadable media already.
                                &mut Vec::new(),
                                Resolution::Half,
                            );
                            ComposedFrame { pts, layers }
                        })
                        .collect();
                    if sender.send(composed).is_err() {
                        return;
                    }
                }
            })
            .map_err(PlaybackError::WorkerSpawn)?;
        Ok(Self { requests, frames })
    }

    /// Asks for the pictures of `sequence` at the frames holding `times`.
    pub fn request(&self, sequence: Arc<Sequence>, times: Vec<Ticks>) {
        // The worker only stops once this probe is dropped.
        let _ = self.requests.send((sequence, times));
    }

    /// The newest pictures composed since the last call, in the order of their times.
    pub fn latest(&self) -> Option<Vec<ComposedFrame>> {
        self.frames.try_iter().last()
    }
}
