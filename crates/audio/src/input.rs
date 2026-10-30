use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use crossbeam::queue::ArrayQueue;

use crate::output::distinct_names;
use crate::{AudioError, SampleRate};

/// How much sound waits for the recording worker; it drains far more often than this.
const QUEUE_SECONDS: usize = 4;

/// An input device as listed and remembered, as for output devices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDevice {
    pub id: Option<String>,
    pub name: String,
}

impl InputDevice {
    pub fn is(&self, other: &InputDevice) -> bool {
        match (&self.id, &other.id) {
            (Some(id), Some(other_id)) => id == other_id,
            _ => self.name == other.name,
        }
    }
}

/// The input devices connected now. Asking the system can take a while, so call this
/// off the UI thread.
pub fn input_devices() -> Result<Vec<InputDevice>, AudioError> {
    let devices: Vec<_> = cpal::default_host()
        .input_devices()
        .map_err(AudioError::Device)?
        .collect();
    Ok(described(&devices))
}

fn described(devices: &[cpal::Device]) -> Vec<InputDevice> {
    let names = distinct_names(devices.iter().map(ToString::to_string));
    devices
        .iter()
        .zip(names)
        .map(|(device, name)| InputDevice {
            id: device.id().ok().map(|id| id.to_string()),
            name,
        })
        .collect()
}

struct Shared {
    /// Mono samples kept since [`AudioInput::keep`].
    queue: ArrayQueue<f32>,
    keeping: AtomicBool,
    /// The loudest sample since the level was last read, as `f32` bits: for values that
    /// aren't negative the bits order as the numbers do, so `fetch_max` finds the loudest.
    peak: AtomicU32,
    /// Set when sound was lost because the queue was full.
    overflowed: AtomicBool,
    lost: AtomicBool,
}

/// An open input stream, mixed down to mono, as a voice is recorded.
pub struct AudioInput {
    _stream: cpal::Stream,
    shared: Arc<Shared>,
    device: InputDevice,
    sample_rate: SampleRate,
}

/// Reads how loud the input is, from any thread.
#[derive(Clone)]
pub struct InputLevel(Arc<Shared>);

impl InputLevel {
    /// The loudest sample since the last call, from 0 to 1 and over.
    pub fn take_peak(&self) -> f32 {
        f32::from_bits(self.0.peak.swap(0, Ordering::AcqRel))
    }
}

impl AudioInput {
    /// Opens the given device, or the system default, measuring its level at once but
    /// keeping nothing until [`keep`](Self::keep). Opening can take a while, so call this
    /// off the UI thread.
    pub fn open(wanted: Option<&InputDevice>) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let (device, opened) = match wanted {
            None => {
                let device = host
                    .default_input_device()
                    .ok_or(AudioError::NoInputDevice)?;
                let opened = InputDevice {
                    id: device.id().ok().map(|id| id.to_string()),
                    name: device.to_string(),
                };
                (device, opened)
            }
            Some(wanted) => {
                let devices: Vec<_> = host.input_devices().map_err(AudioError::Device)?.collect();
                let listed = described(&devices);
                devices
                    .into_iter()
                    .zip(listed)
                    .find(|(_, listed)| listed.is(wanted))
                    .ok_or_else(|| AudioError::MissingDevice(wanted.name.clone()))?
            }
        };
        let config = device.default_input_config().map_err(AudioError::Device)?;
        let sample_rate = SampleRate(config.sample_rate());
        let channels = usize::from(config.channels()).max(1);
        let shared = Arc::new(Shared {
            queue: ArrayQueue::new(sample_rate.0 as usize * QUEUE_SECONDS),
            keeping: AtomicBool::new(false),
            peak: AtomicU32::new(0),
            overflowed: AtomicBool::new(false),
            lost: AtomicBool::new(false),
        });
        let format = config.sample_format();
        let config: cpal::StreamConfig = config.into();
        let stream = match format {
            cpal::SampleFormat::F32 => stream::<f32>(&device, config, &shared, channels),
            cpal::SampleFormat::I16 => stream::<i16>(&device, config, &shared, channels),
            cpal::SampleFormat::I32 => stream::<i32>(&device, config, &shared, channels),
            cpal::SampleFormat::U16 => stream::<u16>(&device, config, &shared, channels),
            _ => Err(AudioError::UnsupportedInputFormat),
        }?;
        stream.play().map_err(AudioError::Device)?;
        Ok(Self {
            _stream: stream,
            shared,
            device: opened,
            sample_rate,
        })
    }

    pub fn device(&self) -> &InputDevice {
        &self.device
    }

    pub fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    pub fn level(&self) -> InputLevel {
        InputLevel(Arc::clone(&self.shared))
    }

    /// Starts keeping what comes in, for [`drain`](Self::drain) to collect.
    pub fn keep(&self) {
        self.shared.keeping.store(true, Ordering::Release);
    }

    /// Moves what was kept since the last call to the end of `into`.
    pub fn drain(&self, into: &mut Vec<f32>) {
        into.extend(std::iter::from_fn(|| self.shared.queue.pop()));
    }

    /// Whether some sound was lost because it wasn't drained in time.
    pub fn overflowed(&self) -> bool {
        self.shared.overflowed.load(Ordering::Acquire)
    }

    /// Whether the device went away or stopped.
    pub fn is_lost(&self) -> bool {
        self.shared.lost.load(Ordering::Acquire)
    }
}

fn stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    shared: &Arc<Shared>,
    channels: usize,
) -> Result<cpal::Stream, AudioError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let callback_shared = Arc::clone(shared);
    let error_shared = Arc::clone(shared);
    // Precomputed so the callback only multiplies.
    let mix = 1.0 / channels as f32;
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                take(&callback_shared, data, channels, mix);
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
        .map_err(AudioError::Device)
}

/// Runs on the audio thread: mixes each frame to mono into the preallocated queue,
/// without allocating, locking or waiting.
fn take<T>(shared: &Shared, data: &[T], channels: usize, mix: f32)
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let keeping = shared.keeping.load(Ordering::Acquire);
    let mut peak = 0.0f32;
    for frame in data.chunks(channels) {
        let mono = frame.iter().map(|&s| s.to_sample::<f32>()).sum::<f32>() * mix;
        peak = peak.max(mono.abs());
        if keeping && shared.queue.push(mono).is_err() {
            shared.overflowed.store(true, Ordering::Release);
        }
    }
    shared.peak.fetch_max(peak.to_bits(), Ordering::AcqRel);
}
