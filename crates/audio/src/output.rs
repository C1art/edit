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
    /// Set once the device goes away or the stream can no longer play.
    lost: AtomicBool,
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
    device_name: String,
}

/// The names of the output devices connected now, each told apart from any others of
/// the same name. Asking the system can take a while, so call this off the UI thread.
pub fn output_devices() -> Result<Vec<String>, AudioError> {
    let devices = cpal::default_host()
        .output_devices()
        .map_err(AudioError::Device)?;
    Ok(distinct_names(devices.map(|device| device.to_string())))
}

/// The names in order, a later one already taken numbered " (2)", " (3)" and on, as
/// Windows names devices of the same model alike.
fn distinct_names(names: impl Iterator<Item = String>) -> Vec<String> {
    let mut distinct: Vec<String> = Vec::new();
    for name in names {
        let numbered = (2..).map(|number| format!("{name} ({number})"));
        let free = std::iter::once(name.clone())
            .chain(numbered)
            .find(|candidate| !distinct.contains(candidate))
            .unwrap_or(name);
        distinct.push(free);
    }
    distinct
}

impl AudioOutput {
    /// Opens the named device, or the system default, which playback then follows where the
    /// system allows. Opening can take a while, so call this off the UI thread.
    pub fn open(device_name: Option<&str>) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = match device_name {
            None => host.default_output_device(),
            Some(name) => {
                let devices: Vec<_> = host.output_devices().map_err(AudioError::Device)?.collect();
                let names = distinct_names(devices.iter().map(ToString::to_string));
                devices
                    .into_iter()
                    .zip(names)
                    .find_map(|(device, distinct)| (distinct == name).then_some(device))
            }
        };
        let device = device.ok_or_else(|| match device_name {
            Some(name) => AudioError::MissingDevice(name.to_owned()),
            None => AudioError::NoOutputDevice,
        })?;
        // Chosen by its numbered name, which the device itself doesn't know.
        let name = device_name.map_or_else(|| device.to_string(), str::to_owned);
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
            lost: AtomicBool::new(false),
            session: Mutex::new(0),
            next_session: AtomicU64::new(1),
            format,
        });
        let callback_shared = Arc::clone(&shared);
        let error_shared = Arc::clone(&shared);
        let stream = device
            .build_output_stream(
                config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    fill(&callback_shared, data, channels);
                },
                // This can run on the audio thread, so it only raises a flag for the owner
                // to poll.
                move |error: cpal::Error| {
                    if matches!(
                        error.kind(),
                        cpal::ErrorKind::DeviceNotAvailable
                            | cpal::ErrorKind::StreamInvalidated
                            | cpal::ErrorKind::HostUnavailable
                    ) {
                        error_shared.lost.store(true, Ordering::Release);
                    }
                },
                None,
            )
            .map_err(AudioError::Device)?;
        stream.play().map_err(AudioError::Device)?;
        Ok(Self {
            _stream: stream,
            shared,
            device_name: name,
        })
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Whether the device went away or stopped playing, so the output must be opened anew.
    pub fn is_lost(&self) -> bool {
        self.shared.lost.load(Ordering::Acquire)
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

    pub fn queued_frames(&self) -> usize {
        self.shared.queue.len() / usize::from(self.shared.format.channels)
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

    /// Unpauses only if `still_wanted` holds while no other producer can pause meanwhile,
    /// so a worker acting on a stale request can't undo a newer pause.
    pub fn resume_if(&self, still_wanted: impl FnOnce() -> bool) {
        let session = lock(&self.shared.session);
        if *session == self.session && still_wanted() {
            self.shared.paused.store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_of_one_name_are_numbered_after_the_first() {
        let names = [
            "Speakers",
            "Speakers (2)",
            "Headphones",
            "Speakers",
            "Speakers",
        ];
        assert_eq!(
            distinct_names(names.into_iter().map(str::to_owned)),
            [
                "Speakers",
                "Speakers (2)",
                "Headphones",
                "Speakers (3)",
                "Speakers (4)"
            ]
        );
    }
}
