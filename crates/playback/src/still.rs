use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use crossbeam::channel::Receiver;
use media::StillFormat;
use render::{Compositor, LayerImage};
use timeline::{Sequence, Ticks};

use crate::{compose, Layer, PlaybackError, Resolution};

/// Saves the sequence's picture at one time as an image file, at the sequence's frame
/// size, on a worker thread.
pub struct StillJob {
    done: Receiver<Result<Saved, PlaybackError>>,
}

#[derive(Debug)]
pub struct Saved {
    pub path: PathBuf,
    /// Media that could not be read, and is missing from the picture.
    pub warnings: Vec<PlaybackError>,
}

impl StillJob {
    pub fn start(
        sequence: Arc<Sequence>,
        time: Ticks,
        device: wgpu::Device,
        queue: wgpu::Queue,
        path: PathBuf,
        format: StillFormat,
    ) -> Result<Self, PlaybackError> {
        let (sender, done) = crossbeam::channel::bounded(1);
        thread::Builder::new()
            .name("export frame".into())
            .spawn(move || {
                let compositor = Compositor::new(&device, &queue);
                let saved = save(&sequence, time, compositor, &path, format)
                    .map(|warnings| Saved { path, warnings });
                let _ = sender.send(saved);
            })
            .map_err(PlaybackError::WorkerSpawn)?;
        Ok(Self { done })
    }

    /// The outcome once done; never blocks.
    pub fn poll(&self) -> Option<Result<Saved, PlaybackError>> {
        self.done.try_recv().ok()
    }
}

fn save(
    sequence: &Sequence,
    time: Ticks,
    mut compositor: Compositor,
    path: &std::path::Path,
    format: StillFormat,
) -> Result<Vec<PlaybackError>, PlaybackError> {
    let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
    // The playhead parked at the end shows the last frame, not the black after it.
    let time = time.min(sequence.duration() - Ticks(1)).max(Ticks::ZERO);
    let frame = rate.frame_start(rate.frame_at(time, timebase), timebase);
    let mut warnings = Vec::new();
    let layers = compose::layers_at(
        sequence,
        frame,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut warnings,
        Resolution::Full,
    );
    let images: Vec<LayerImage<'_>> = layers.iter().map(Layer::image).collect();
    let size = (sequence.format.width, sequence.format.height);
    compositor.composite(size, &images)?;
    media::write_still(path, format, size, &compositor.read_output()?)?;
    Ok(warnings)
}
