use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam::queue::ArrayQueue;

use crate::{AudioError, SampleRate};

/// Enough queued audio to ride out a slow decode without making seeks feel sluggish.
const QUEUE_SECONDS: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputFormat {
    pub sample_rate: SampleRate,
    pub channels: u16,
}

#[derive(Debug)]
struct Shared {
    queue: ArrayQueue<f32>,
    frames_played: AtomicU64,
    paused: AtomicBool,
    /// Identifies the writer allowed to push. Only producers take this lock, never the
    /// callback, so a writer that is shutting down cannot leak stale audio into a new one.
    session: Mutex<u64>,
    next_session: AtomicU64,
    format: OutputFormat,
}

/// The open device stream. Keep it alive on the thread that created it; hand
/// [`AudioWriter`]s to the threads that produce samples.
pub struct AudioOutput {
    _stream: cpal::Stream,
    shared: Arc<Shared>,
}

impl AudioOutput {
    pub fn open_default() -> Result<Self, AudioError> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or(AudioError::NoOutputDevice)?;
        let config = device.default_output_config().map_err(AudioError::Device)?;
        if config.sample_format() != cpal::SampleFormat::F32 {
            return Err(AudioError::UnsupportedSampleFormat);
        }
        let format = OutputFormat {
            sample_rate: SampleRate(config.sample_rate()),
            channels: config.channels(),
        };
        let channels = usize::from(format.channels);
        let shared = Arc::new(Shared {
            queue: ArrayQueue::new(
                (format.sample_rate.0 as usize * channels * QUEUE_SECONDS).max(channels),
            ),
            frames_played: AtomicU64::new(0),
            paused: AtomicBool::new(true),
            session: Mutex::new(0),
            next_session: AtomicU64::new(1),
            format,
        });
        let callback_shared = Arc::clone(&shared);
        let stream = device
            .build_output_stream(
                config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    fill(&callback_shared, data, channels);
                },
                // Logging is off-limits in the callback; device loss surfaces as a stalled
                // clock instead, which the player reports.
                |_error| {},
                None,
            )
            .map_err(AudioError::Device)?;
        stream.play().map_err(AudioError::Device)?;
        Ok(Self {
            _stream: stream,
            shared,
        })
    }

    pub fn format(&self) -> OutputFormat {
        self.shared.format
    }

    /// Hands the output to a new producer: earlier writers stop being able to push, the
    /// queue is emptied and playback is paused.
    pub fn writer(&self) -> AudioWriter {
        let writer = AudioWriter {
            shared: Arc::clone(&self.shared),
            session: self.shared.next_session.fetch_add(1, Ordering::Relaxed),
        };
        writer.claim();
        writer
    }
}

fn fill(shared: &Shared, data: &mut [f32], channels: usize) {
    if shared.paused.load(Ordering::Acquire) {
        data.fill(0.0);
        return;
    }
    let mut played = 0;
    for frame in data.chunks_mut(channels) {
        let mut complete = true;
        for sample in frame.iter_mut() {
            match shared.queue.pop() {
                Some(value) => *sample = value,
                None => {
                    *sample = 0.0;
                    complete = false;
                }
            }
        }
        if complete {
            played += 1;
        }
    }
    shared.frames_played.fetch_add(played, Ordering::Release);
}

fn lock(session: &Mutex<u64>) -> std::sync::MutexGuard<'_, u64> {
    // The guarded value is a plain counter, so a panic while holding it can't corrupt it.
    session.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A producer's handle to the output queue and clock.
#[derive(Debug, Clone)]
pub struct AudioWriter {
    shared: Arc<Shared>,
    session: u64,
}

impl AudioWriter {
    pub fn format(&self) -> OutputFormat {
        self.shared.format
    }

    /// Queues whole interleaved frames from the start of `samples` and returns how many
    /// samples were taken. Returns 0 once another writer has taken over the output.
    pub fn push(&self, samples: &[f32]) -> usize {
        let session = lock(&self.shared.session);
        if *session != self.session {
            return 0;
        }
        let queue = &self.shared.queue;
        let channels = usize::from(self.shared.format.channels);
        let room = queue.capacity() - queue.len();
        let take = samples.len().min(room) / channels * channels;
        for &sample in &samples[..take] {
            // Cannot fail: this writer is the only producer and the callback only pops.
            let _ = queue.push(sample);
        }
        take
    }

    /// Whether this writer (or a clone of it) currently owns the output.
    pub fn is_active(&self) -> bool {
        *lock(&self.shared.session) == self.session
    }

    /// Takes the output back from whichever writer has it: the queue is emptied and
    /// playback is paused, as for a new writer.
    pub fn claim(&self) {
        let mut session = lock(&self.shared.session);
        *session = self.session;
        self.shared.paused.store(true, Ordering::Release);
        while self.shared.queue.pop().is_some() {}
    }

    pub fn clear(&self) {
        let session = lock(&self.shared.session);
        if *session == self.session {
            while self.shared.queue.pop().is_some() {}
        }
    }

    pub fn is_empty(&self) -> bool {
        self.shared.queue.is_empty()
    }

    /// Frames the device has taken from the queue since the output was opened; this is the
    /// master clock.
    pub fn frames_played(&self) -> u64 {
        self.shared.frames_played.load(Ordering::Acquire)
    }

    pub fn set_paused(&self, paused: bool) {
        let session = lock(&self.shared.session);
        if *session == self.session {
            self.shared.paused.store(paused, Ordering::Release);
        }
    }
}
