use std::ops::Range;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use audio::SampleRate;
use crossbeam::channel::{Receiver, Sender};
use media::{AudioSettings, EncodeProfile, MediaWriter, VideoSettings};
use render::{Compositor, LayerImage};
use timeline::{Sequence, Ticks};

use crate::sequence_mix::{SequenceMix, CHUNK_FRAMES};
use crate::{compose, Layer, PlaybackError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSettings {
    pub path: PathBuf,
    /// x264 constant rate factor; lower is better and larger.
    pub crf: u8,
    pub sample_rate: u32,
    pub audio_bits_per_second: u32,
    /// The part of the sequence to export; the whole of it when `None`.
    pub range: Option<Range<Ticks>>,
}

#[derive(Debug)]
pub enum ExportEvent {
    Progress {
        frames_done: i64,
        frames_total: i64,
    },
    /// A piece of media could not be read; the export carries on without it.
    Warning(PlaybackError),
    Finished,
    /// The partial file has been removed.
    Cancelled,
    /// The partial file has been removed.
    Failed(PlaybackError),
}

/// Renders a sequence to a file on a worker thread, at the sequence's own frame size
/// and rate. Dropping the job cancels it.
pub struct ExportJob {
    events: Receiver<ExportEvent>,
    cancel: Arc<AtomicBool>,
}

impl ExportJob {
    pub fn start(
        sequence: Arc<Sequence>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        settings: ExportSettings,
    ) -> Result<Self, PlaybackError> {
        let (sender, events) = crossbeam::channel::unbounded();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker = Worker {
            compositor: Compositor::new(&device, &queue),
            sequence,
            settings,
            events: sender,
            cancel: Arc::clone(&cancel),
        };
        thread::Builder::new()
            .name("export".into())
            .spawn(move || worker.run())
            .map_err(PlaybackError::WorkerSpawn)?;
        Ok(Self { events, cancel })
    }

    /// Everything that happened since the last poll; never blocks.
    pub fn poll(&self) -> Vec<ExportEvent> {
        self.events.try_iter().collect()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Drop for ExportJob {
    fn drop(&mut self) {
        self.cancel();
    }
}

struct Worker {
    sequence: Arc<Sequence>,
    settings: ExportSettings,
    compositor: Compositor,
    events: Sender<ExportEvent>,
    cancel: Arc<AtomicBool>,
}

enum Outcome {
    Finished,
    Cancelled,
}

impl Worker {
    fn run(mut self) {
        let event = match self.render() {
            Ok(Outcome::Finished) => ExportEvent::Finished,
            Ok(Outcome::Cancelled) => {
                let _ = std::fs::remove_file(&self.settings.path);
                ExportEvent::Cancelled
            }
            Err(error) => {
                let _ = std::fs::remove_file(&self.settings.path);
                ExportEvent::Failed(error)
            }
        };
        let _ = self.events.send(event);
    }

    fn render(&mut self) -> Result<Outcome, PlaybackError> {
        let sequence = Arc::clone(&self.sequence);
        let format = sequence.format;
        let (rate, timebase) = (format.frame_rate, sequence.timebase);
        let video = VideoSettings {
            width: format.width,
            height: format.height,
            frame_rate: rate,
            crf: self.settings.crf,
            profile: EncodeProfile::Delivery,
        };
        let audio = AudioSettings {
            sample_rate: self.settings.sample_rate,
            bits_per_second: self.settings.audio_bits_per_second,
        };
        let mut writer = MediaWriter::create(&self.settings.path, Some(video), Some(audio))?;

        let frames = frame_span(&sequence, self.settings.range.clone());
        let frames_total = frames.end - frames.start;
        let mut mix = SequenceMix::new(SampleRate(self.settings.sample_rate));
        let audio_rate = mix.rate();
        let to_audio = |time: Ticks| time.rescale(timebase, audio_rate).0;
        let audio_total = to_audio(rate.frame_start(frames.end, timebase));
        let mut audio_written = to_audio(rate.frame_start(frames.start, timebase));
        let mut tracks = Vec::new();
        let mut warned = Vec::new();
        let mut warnings = Vec::new();

        for frame in frames.clone() {
            if self.cancel.load(Ordering::Relaxed) {
                return Ok(Outcome::Cancelled);
            }
            let pts = rate.frame_start(frame, timebase);
            let layers =
                compose::layers_at(&sequence, pts, &mut tracks, &mut warned, &mut warnings);
            let images: Vec<LayerImage<'_>> = layers.iter().map(Layer::image).collect();
            self.compositor
                .composite((format.width, format.height), &images)?;
            writer.write_video(&self.compositor.read_output()?)?;

            // Audio keeps pace with the pictures so the muxer can interleave them.
            let audio_end = to_audio(rate.frame_start(frame + 1, timebase)).min(audio_total);
            while audio_written < audio_end {
                mix.mix_chunk(&sequence, audio_written, &mut warnings);
                let frames = (audio_total - audio_written).min(CHUNK_FRAMES as i64);
                let samples = usize::try_from(frames).unwrap_or(0) * 2;
                writer.write_audio(&mix.mix[..samples])?;
                audio_written += frames;
            }

            for warning in warnings.drain(..) {
                let _ = self.events.send(ExportEvent::Warning(warning));
            }
            let _ = self.events.send(ExportEvent::Progress {
                frames_done: frame + 1 - frames.start,
                frames_total,
            });
        }
        writer.finish()?;
        Ok(Outcome::Finished)
    }
}

/// Every frame that overlaps `range`, or the sequence up to the end of its last clip.
fn frame_span(sequence: &Sequence, range: Option<Range<Ticks>>) -> Range<i64> {
    let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
    let range = range.unwrap_or(Ticks::ZERO..sequence.duration());
    let first = rate.frame_at(range.start, timebase);
    let last = rate.frame_at(range.end, timebase);
    let end = if rate.frame_start(last, timebase) < range.end {
        last + 1
    } else {
        last
    };
    first..end.max(first)
}

#[cfg(test)]
mod tests {
    use timeline::{Clip, ClipId, FrameRate, SourceId, Timebase, Track, VideoFormat};

    use super::*;

    #[test]
    fn a_range_exports_every_frame_it_overlaps() {
        let format = VideoFormat {
            frame_rate: FrameRate::FPS_25,
            ..VideoFormat::default()
        };
        let mut sequence = Sequence::new(Timebase::HZ_90K, format);
        let clip = Clip::new(ClipId(1), SourceId(1), Ticks(0), Ticks(36_100), Ticks(0));
        sequence.video.push(Track {
            clips: vec![clip],
            ..Track::default()
        });
        // A frame lasts 3,600 ticks; the clip ends 100 ticks into frame 10.
        assert_eq!(frame_span(&sequence, None), 0..11);
        assert_eq!(
            frame_span(&sequence, Some(Ticks(7_200)..Ticks(18_000))),
            2..5
        );
        assert_eq!(
            frame_span(&sequence, Some(Ticks(7_300)..Ticks(10_900))),
            2..4
        );
    }
}
