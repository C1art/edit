use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use audio::SampleRate;
use crossbeam::channel::{Receiver, Sender};
use timeline::Ticks;

use crate::sequence_mix::{SequenceMix, CHUNK_FRAMES};
use crate::{Piece, PlaybackError};

const MEASURE_RATE: SampleRate = SampleRate(48_000);
/// Quieter than any real recording, so silence still has a finite level.
const SILENCE_DB: f32 = -120.0;

#[derive(Debug)]
pub enum EnvelopeEvent {
    /// The fraction measured so far.
    Progress(f32),
    /// A piece of media could not be read and was measured as silence.
    Warning(PlaybackError),
    /// The RMS level in dBFS of each window from the start of the piece.
    Measured(Vec<f32>),
}

/// Mixes a piece of a sequence on a worker thread and measures how loud it is through
/// time. Dropping the job cancels it.
pub struct EnvelopeJob {
    events: Receiver<EnvelopeEvent>,
    cancel: Arc<AtomicBool>,
}

impl EnvelopeJob {
    /// Measures `piece` in windows `window` long, in its sequence's timebase.
    pub fn start(piece: Piece, window: Ticks) -> Result<Self, PlaybackError> {
        let (sender, events) = crossbeam::channel::unbounded();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        thread::Builder::new()
            .name("envelope".into())
            .spawn(move || measure(&piece, window, &sender, &worker_cancel))
            .map_err(PlaybackError::WorkerSpawn)?;
        Ok(Self { events, cancel })
    }

    /// Everything that happened since the last poll; never blocks.
    pub fn poll(&self) -> Vec<EnvelopeEvent> {
        self.events.try_iter().collect()
    }
}

impl Drop for EnvelopeJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

fn measure(
    (sequence, range): &Piece,
    window: Ticks,
    events: &Sender<EnvelopeEvent>,
    cancel: &AtomicBool,
) {
    let mut mix = SequenceMix::new(MEASURE_RATE);
    let rate = mix.rate();
    let to_frames = |time: Ticks| time.rescale(sequence.timebase, rate).0;
    let span = to_frames(range.start)..to_frames(range.end);
    let mut windows = Windows::new(usize::try_from(to_frames(window)).unwrap_or(1));
    let mut warnings = Vec::new();
    for start in span.clone().step_by(CHUNK_FRAMES) {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        mix.mix_chunk(sequence, start, &mut warnings);
        let length = (span.end - start).min(CHUNK_FRAMES as i64);
        windows.push(&mix.mix[..usize::try_from(length).unwrap_or(0) * 2]);
        for warning in warnings.drain(..) {
            let _ = events.send(EnvelopeEvent::Warning(warning));
        }
        let done = (start + length - span.start) as f32 / (span.end - span.start).max(1) as f32;
        let _ = events.send(EnvelopeEvent::Progress(done));
    }
    let _ = events.send(EnvelopeEvent::Measured(windows.finish()));
}

/// RMS levels of stereo audio over windows of a fixed number of frames, however the
/// audio arrives.
struct Windows {
    frames_per_window: usize,
    squares: f64,
    frames: usize,
    levels: Vec<f32>,
}

impl Windows {
    fn new(frames_per_window: usize) -> Self {
        Self {
            frames_per_window: frames_per_window.max(1),
            squares: 0.0,
            frames: 0,
            levels: Vec::new(),
        }
    }

    fn push(&mut self, stereo: &[f32]) {
        for [left, right] in stereo.as_chunks::<2>().0 {
            self.squares += f64::from(left * left + right * right) / 2.0;
            self.frames += 1;
            if self.frames == self.frames_per_window {
                self.close();
            }
        }
    }

    fn close(&mut self) {
        let mean = self.squares / self.frames.max(1) as f64;
        let level = if mean > 0.0 {
            (10.0 * mean.log10()) as f32
        } else {
            SILENCE_DB
        };
        self.levels.push(level.max(SILENCE_DB));
        self.squares = 0.0;
        self.frames = 0;
    }

    /// A last, partial window is measured over the frames it has.
    fn finish(mut self) -> Vec<f32> {
        if self.frames > 0 {
            self.close();
        }
        self.levels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_span_pushes_and_a_partial_last_window_counts() {
        let mut windows = Windows::new(4);
        // Full scale for five frames, then three frames of silence.
        windows.push(&[1.0; 6]);
        windows.push(&[-1.0, -1.0, 1.0, 1.0, 0.0, 0.0]);
        windows.push(&[0.0; 4]);
        let levels = windows.finish();
        assert_eq!(levels.len(), 2);
        assert!(levels[0].abs() < 1e-6);
        // One loud frame in four is a quarter of the power: -6 dB.
        assert!((levels[1] + 6.0206).abs() < 1e-3, "{}", levels[1]);
    }
}
