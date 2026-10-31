use std::ops::{ControlFlow, Range};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use audio::{LoudnessMeter, SampleRate};
use crossbeam::channel::{Receiver, Sender};
use timeline::{ClipId, Gain, Param, Sequence, Ticks, Timebase, Track};

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

/// Brings an export's sound to a loudness, as far as a ceiling on its peaks allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoudnessTarget {
    /// Integrated loudness, in LUFS.
    pub lufs: i32,
    /// The highest a sample may reach, in dBFS.
    pub peak_db: i32,
}

impl LoudnessTarget {
    /// The linear gain that brings sound measured as `measured` to the target, lowered
    /// so its peak stays under the ceiling; unity for silence.
    pub fn gain(self, measured: Loudness) -> f32 {
        let Some(integrated) = measured.integrated else {
            return 1.0;
        };
        let wanted = db_to_linear(self.lufs as f32 - integrated);
        let ceiling = db_to_linear(self.peak_db as f32) / measured.peak.max(f32::EPSILON);
        wanted.min(ceiling)
    }
}

fn db_to_linear(db: f32) -> f32 {
    10_f32.powf(db / 20.0)
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
    let total: i64 = pieces
        .iter()
        .map(|(sequence, range)| span_frames(sequence, range, rate).count() as i64)
        .sum();
    let mut done = 0;
    let mut results = Vec::with_capacity(pieces.len());
    for (sequence, range) in pieces {
        let measured = measure_span(&mut mix, sequence, range.clone(), |length, warnings| {
            if cancel.load(Ordering::Relaxed) {
                return ControlFlow::Break(());
            }
            done += length;
            for warning in warnings.drain(..) {
                let _ = events.send(LoudnessEvent::Warning(warning));
            }
            let _ = events.send(LoudnessEvent::Progress(done as f32 / total.max(1) as f32));
            ControlFlow::Continue(())
        });
        let Some(measured) = measured else {
            return;
        };
        results.push(measured);
    }
    let _ = events.send(LoudnessEvent::Measured(results));
}

fn span_frames(sequence: &Sequence, range: &Range<Ticks>, rate: Timebase) -> Range<i64> {
    let to_frames = |time: Ticks| time.rescale(sequence.timebase, rate).0;
    to_frames(range.start)..to_frames(range.end)
}

/// Mixes `range` of `sequence` and measures it, calling `step` after each chunk with
/// how many sample frames it held and what went wrong mixing it; `None` once `step`
/// breaks off.
pub(crate) fn measure_span(
    mix: &mut SequenceMix,
    sequence: &Sequence,
    range: Range<Ticks>,
    mut step: impl FnMut(i64, &mut Vec<PlaybackError>) -> ControlFlow<()>,
) -> Option<Loudness> {
    let rate = mix.rate();
    let mut meter = LoudnessMeter::new(mix.sample_rate());
    let span = span_frames(sequence, &range, rate);
    let mut warnings = Vec::new();
    mix.reset();
    for start in span.clone().step_by(CHUNK_FRAMES) {
        mix.mix_chunk(sequence, start, &mut warnings);
        let length = (span.end - start).min(CHUNK_FRAMES as i64);
        meter.push(&mix.mix[..usize::try_from(length).unwrap_or(0) * 2]);
        if step(length, &mut warnings).is_break() {
            return None;
        }
    }
    Some(Loudness {
        integrated: meter.integrated(),
        peak: meter.peak(),
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_gain_stops_at_the_peak_ceiling() {
        let target = LoudnessTarget {
            lufs: -14,
            peak_db: -1,
        };
        let quiet = Loudness {
            integrated: Some(-20.0),
            peak: 0.1,
        };
        assert!((target.gain(quiet) - db_to_linear(6.0)).abs() < 1e-4);
        let peaky = Loudness { peak: 0.5, ..quiet };
        assert!((target.gain(peaky) * 0.5 - db_to_linear(-1.0)).abs() < 1e-4);
        let silent = Loudness {
            integrated: None,
            peak: 0.0,
        };
        assert_eq!(target.gain(silent), 1.0);
    }
}
