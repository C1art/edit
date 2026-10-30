use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use audio::{LoudnessMeter, SampleRate};
use crossbeam::channel::{Receiver, Sender};
use timeline::{ClipId, Gain, Param, Sequence, Ticks, Track};

use crate::sequence_mix::{SequenceMix, CHUNK_FRAMES};
use crate::PlaybackError;

const MEASURE_RATE: SampleRate = SampleRate(48_000);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loudness {
    /// Gated integrated loudness in LUFS; `None` for silence.
    pub integrated: Option<f32>,
    /// The largest sample magnitude, as a linear amplitude.
    pub peak: f32,
}

#[derive(Debug)]
pub enum LoudnessEvent {
    /// The fraction of all the audio measured so far.
    Progress(f32),
    /// A piece of media could not be read and was measured as silence.
    Warning(PlaybackError),
    /// One result per piece, in the order given.
    Measured(Vec<Loudness>),
}

/// A sequence and the part of it to measure.
pub type Piece = (Arc<Sequence>, Range<Ticks>);

/// Mixes pieces of sequences on a worker thread and measures how loud each is.
/// Dropping the job cancels it.
pub struct LoudnessJob {
    events: Receiver<LoudnessEvent>,
    cancel: Arc<AtomicBool>,
}

impl LoudnessJob {
    pub fn start(pieces: Vec<Piece>) -> Result<Self, PlaybackError> {
        let (sender, events) = crossbeam::channel::unbounded();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        thread::Builder::new()
            .name("loudness".into())
            .spawn(move || measure(&pieces, &sender, &worker_cancel))
            .map_err(PlaybackError::WorkerSpawn)?;
        Ok(Self { events, cancel })
    }

    /// Everything that happened since the last poll; never blocks.
    pub fn poll(&self) -> Vec<LoudnessEvent> {
        self.events.try_iter().collect()
    }
}

impl Drop for LoudnessJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

fn measure(pieces: &[Piece], events: &Sender<LoudnessEvent>, cancel: &AtomicBool) {
    let mut mix = SequenceMix::new(MEASURE_RATE);
    let rate = mix.rate();
    let frames = |(sequence, range): &Piece| {
        let to_frames = |time: Ticks| time.rescale(sequence.timebase, rate).0;
        to_frames(range.start)..to_frames(range.end)
    };
    let total: i64 = pieces
        .iter()
        .map(|piece| frames(piece).count() as i64)
        .sum();
    let mut done = 0;
    let mut results = Vec::with_capacity(pieces.len());
    let mut warnings = Vec::new();
    for piece in pieces {
        let mut meter = LoudnessMeter::new(MEASURE_RATE);
        let span = frames(piece);
        mix.reset();
        for start in span.clone().step_by(CHUNK_FRAMES) {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            mix.mix_chunk(&piece.0, start, &mut warnings);
            let length = (span.end - start).min(CHUNK_FRAMES as i64);
            meter.push(&mix.mix[..usize::try_from(length).unwrap_or(0) * 2]);
            done += length;
            for warning in warnings.drain(..) {
                let _ = events.send(LoudnessEvent::Warning(warning));
            }
            let _ = events.send(LoudnessEvent::Progress(done as f32 / total.max(1) as f32));
        }
        results.push(Loudness {
            integrated: meter.integrated(),
            peak: meter.peak(),
        });
    }
    let _ = events.send(LoudnessEvent::Measured(results));
}

/// `sequence` with only `clip` left, heard at unity with no volume, effects (its own or
/// the mix's) or transitions, so measuring it measures the source as the clip's gain
/// scales it.
pub fn clip_alone(sequence: &Sequence, clip: ClipId) -> Option<Piece> {
    let (_, found) = sequence.clip(clip)?;
    let range = found.start..found.end();
    let mut alone = sequence.clone();
    alone.video.iter_mut().for_each(|track| track.clips.clear());
    alone.master = Gain::UNITY;
    alone.master_effects.clear();
    for track in &mut alone.audio {
        track.clips.retain(|kept| kept.id == clip);
        *track = Track {
            clips: std::mem::take(&mut track.clips),
            ..Track::default()
        };
        for kept in track.clips.iter_mut().map(Arc::make_mut) {
            kept.gain = Gain::UNITY;
            kept.volume = Param::default();
            kept.pan = Param::default();
            kept.audio_effects.clear();
            kept.disabled = false;
            kept.transition_in = None;
            kept.transition_out = None;
        }
    }
    Some((Arc::new(alone), range))
}
