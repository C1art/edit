use std::sync::Arc;
use std::thread;

use crossbeam::channel::Receiver;
use render::{Compositor, LayerImage};
use timeline::{ClipId, ColorCorrection, Sequence, Ticks};

use crate::tone::{tone_stats, ToneStats};
use crate::{compose, Layer, PlaybackError, Resolution};

/// A sequence and the time to read the picture of.
pub type Picture = (Arc<Sequence>, Ticks);

/// Reads the tone statistics of sequence pictures on a worker thread.
pub struct ToneJob {
    done: Receiver<Result<Vec<Option<ToneStats>>, PlaybackError>>,
}

impl ToneJob {
    pub fn start(
        pictures: Vec<Picture>,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Result<Self, PlaybackError> {
        let (sender, done) = crossbeam::channel::bounded(1);
        thread::Builder::new()
            .name("tone analysis".into())
            .spawn(move || {
                let mut compositor = Compositor::new(&device, &queue);
                let stats = pictures
                    .iter()
                    .map(|(sequence, time)| measure(sequence, *time, &mut compositor))
                    .collect();
                let _ = sender.send(stats);
            })
            .map_err(PlaybackError::WorkerSpawn)?;
        Ok(Self { done })
    }

    /// One result per picture, in the order given, once done; never blocks.
    pub fn poll(&self) -> Option<Result<Vec<Option<ToneStats>>, PlaybackError>> {
        self.done.try_recv().ok()
    }
}

fn measure(
    sequence: &Sequence,
    time: Ticks,
    compositor: &mut Compositor,
) -> Result<Option<ToneStats>, PlaybackError> {
    let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
    let frame = rate.frame_start(rate.frame_at(time, timebase), timebase);
    let layers = compose::layers_at(
        sequence,
        frame,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut Vec::new(),
        Resolution::Half,
    );
    let images: Vec<LayerImage<'_>> = layers.iter().map(Layer::image).collect();
    compositor.composite((sequence.format.width, sequence.format.height), &images)?;
    Ok(tone_stats(&compositor.read_output()?))
}

/// The sequence with only `clip`'s picture, as it looks before its color correction,
/// which is what Auto and Color Match grade from.
pub fn clip_ungraded(sequence: &Sequence, clip: ClipId) -> Option<Arc<Sequence>> {
    sequence.clip(clip)?;
    let mut alone = sequence.clone();
    alone.audio.iter_mut().for_each(|track| track.clips.clear());
    for track in &mut alone.video {
        track.clips.retain(|kept| kept.id == clip);
        for kept in track.clips.iter_mut().map(Arc::make_mut) {
            kept.look.color = ColorCorrection::default();
            kept.disabled = false;
            kept.transition_in = None;
            kept.transition_out = None;
        }
    }
    Some(Arc::new(alone))
}
