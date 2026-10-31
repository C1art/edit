use std::ops::Range;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crossbeam::channel::{self, Receiver};
use timeline::Sequence;

use crate::previews::{self, Preview, PreviewPictures};
use crate::{compose, Layer, PlaybackError, Resolution};

/// Frames decoded ahead of the one compositing. A few smooth over slow frames, such as
/// the first of a clip, without holding many decoded pictures.
const AHEAD: usize = 4;

/// One frame's layers and what went wrong decoding them.
pub(crate) struct Decoded {
    pub(crate) layers: Vec<Layer>,
    pub(crate) warnings: Vec<PlaybackError>,
}

/// Decodes an export's frames in order on a thread of its own, so decoding overlaps
/// compositing and the GPU's work instead of taking turns with them.
pub(crate) struct DecodeAhead {
    frames: Receiver<Decoded>,
    thread: Option<JoinHandle<()>>,
}

impl DecodeAhead {
    pub(crate) fn start(
        sequence: Arc<Sequence>,
        previews: &[Preview],
        frames: Range<i64>,
    ) -> Result<Self, PlaybackError> {
        let previews = previews.to_vec();
        let (sender, receiver) = channel::bounded(AHEAD);
        let thread = thread::Builder::new()
            .name("export decode".into())
            .spawn(move || {
                let format = sequence.format;
                let (rate, timebase) = (format.frame_rate, sequence.timebase);
                let mut previews = PreviewPictures::new(&previews, &sequence);
                let mut tracks = Vec::new();
                let mut warned = Vec::new();
                for frame in frames {
                    let pts = rate.frame_start(frame, timebase);
                    let mut warnings = Vec::new();
                    let layers = previews::previewed(&mut previews, pts, Resolution::Full)
                        .unwrap_or_else(|| {
                            compose::layers_at(
                                &sequence,
                                pts,
                                &mut tracks,
                                &mut warned,
                                &mut warnings,
                                Resolution::Full,
                            )
                        });
                    // A closed channel means the export stopped.
                    if sender.send(Decoded { layers, warnings }).is_err() {
                        return;
                    }
                }
            })
            .map_err(PlaybackError::WorkerSpawn)?;
        Ok(Self {
            frames: receiver,
            thread: Some(thread),
        })
    }

    /// The next frame's layers, or None if the decoding thread died.
    pub(crate) fn next(&self) -> Option<Decoded> {
        self.frames.recv().ok()
    }
}

impl Drop for DecodeAhead {
    fn drop(&mut self) {
        // Closing the channel stops the thread at its next frame.
        drop(std::mem::replace(&mut self.frames, channel::never()));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
