// Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

//! What the export tests share: a GPU, running an export to its end, and reading the
//! file back.

use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use media::{DecodedFrame, MediaReader};
use playback::{ExportEvent, ExportJob, ExportSettings};
use timeline::{Sequence, Ticks, Timebase};

pub const TB: Timebase = Timebase::HZ_90K;

pub type Gpu = (wgpu::Device, wgpu::Queue);

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::yield_now();
    }
}

pub fn gpu() -> Gpu {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("a GPU adapter");
    block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("a GPU device")
}

pub fn export(sequence: &Arc<Sequence>, gpu: &Gpu, settings: ExportSettings) -> Result<(), String> {
    let job = ExportJob::start(Arc::clone(sequence), gpu.0.clone(), gpu.1.clone(), settings)
        .map_err(|error| error.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(120);
    while Instant::now() < deadline {
        for event in job.poll() {
            match event {
                ExportEvent::Finished => return Ok(()),
                ExportEvent::Failed(error) => return Err(error.to_string()),
                ExportEvent::Warning(error) => return Err(format!("warning: {error}")),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Err("timed out".into())
}

#[derive(Debug, Default)]
pub struct Decoded {
    /// Start and mean brightness of each picture.
    pub pictures: Vec<(Ticks, f64)>,
    /// Mono, at the file's sample rate, from the first audio timestamp on.
    pub sound: Vec<f32>,
    pub sound_start: Option<Ticks>,
    pub sample_rate: u32,
    /// What the container declares.
    pub duration: Option<Ticks>,
}

pub fn decode(path: &Path) -> Result<Decoded, String> {
    let mut reader = MediaReader::open(path, TB).map_err(|error| error.to_string())?;
    let mut decoded = Decoded {
        duration: reader.info().duration,
        ..Decoded::default()
    };
    while let Some(frame) = reader.next_frame().map_err(|error| error.to_string())? {
        match frame {
            DecodedFrame::Video(picture) => {
                let total: u64 = picture
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|pixel| &pixel[..3])
                    .map(|&value| u64::from(value))
                    .sum();
                let count = picture.pixels.len() / 4 * 3;
                decoded
                    .pictures
                    .push((picture.pts, total as f64 / count.max(1) as f64));
            }
            DecodedFrame::Audio(buffer) => {
                decoded.sound_start.get_or_insert(buffer.pts);
                decoded.sample_rate = buffer.sample_rate;
                let channels = usize::from(buffer.channels.max(1));
                decoded.sound.extend(
                    buffer
                        .samples
                        .chunks_exact(channels)
                        .map(|frame| frame.iter().sum::<f32>() / channels as f32),
                );
            }
            DecodedFrame::Planar(_) => return Err("planar frame from a packed reader".into()),
        }
    }
    Ok(decoded)
}

impl Decoded {
    /// The pictures brighter than mid-grey.
    pub fn flashes(&self) -> Vec<usize> {
        self.pictures
            .iter()
            .enumerate()
            .filter(|(_, &(_, level))| level > 128.0)
            .map(|(index, _)| index)
            .collect()
    }

    /// Seconds from the file's start to the first audible sample.
    pub fn first_sound(&self) -> Option<f64> {
        let start = self.sound_start.map_or(0.0, |pts| pts.0 as f64 / 90_000.0);
        let rate = f64::from(self.sample_rate.max(1));
        self.sound
            .iter()
            .position(|sample| sample.abs() > 0.01)
            .map(|index| start + index as f64 / rate)
    }
}
