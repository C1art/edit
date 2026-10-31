use std::ops::{ControlFlow, Range};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use audio::SampleRate;
use crossbeam::channel::{Receiver, Sender};
use media::{
    Acceleration, AudioSettings, EncodeProfile, MediaWriter, RateControl, StillFormat, VideoCodec,
    VideoSettings,
};
use render::{Background, Compositor, LayerImage, Readback};
use timeline::{Sequence, Ticks};

use crate::export_sink::{Sink, Stills, Work};
use crate::export_timings::ExportTimings;
use crate::loudness::{measure_span, LoudnessTarget};
use crate::previews::{self, Preview, PreviewPictures};
use crate::sequence_mix::{SequenceMix, CHUNK_FRAMES};
use crate::{compose, Layer, PlaybackError, Resolution};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSettings {
    pub path: PathBuf,
    /// None for an audio-only file.
    pub video: Option<VideoExport>,
    /// None for a silent file.
    pub audio: Option<AudioSettings>,
    /// The part of the sequence to export; the whole of it when `None`.
    pub range: Option<Range<Ticks>>,
    /// Rendered previews whose pictures are taken where they cover, instead of composing
    /// the sequence again; ignored for an export that keeps alpha, which they lack.
    pub previews: Vec<Preview>,
    /// Where the sound is brought to, measured over the whole export first.
    pub loudness: Option<LoudnessTarget>,
}

/// Pictures at the sequence's frame rate, scaled from its frame size to `size`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoExport {
    pub encoding: VideoEncoding,
    /// Ignored by codecs no GPU encodes.
    pub acceleration: Acceleration,
    /// Both even.
    pub size: (u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoEncoding {
    Movie(VideoCodec, RateControl),
    /// Each frame as its own image, numbered after the export's path, without sound.
    Stills(StillFormat),
}

#[derive(Debug)]
pub enum ExportEvent {
    Progress {
        frames_done: i64,
        frames_total: i64,
    },
    /// A piece of media could not be read; the export carries on without it.
    Warning(PlaybackError),
    /// How much of the sound has been measured before anything is written, for an
    /// export brought to a loudness.
    Measuring(f32),
    /// What encodes the video, such as "NVIDIA NVENC (hardware)", sent once it opens.
    Encoder(String),
    /// Where the time went, sent just before `Finished`.
    Timings(ExportTimings),
    Finished,
    /// The partial file has been removed and anything already at the path left alone.
    Cancelled,
    /// The partial file has been removed and anything already at the path left alone.
    Failed(PlaybackError),
}

/// Renders a sequence to a file on a worker thread. Dropping the job cancels it.
pub struct ExportJob {
    events: Receiver<ExportEvent>,
    cancel: Arc<AtomicBool>,
}

impl ExportJob {
    pub fn start(
        sequence: Arc<Sequence>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        mut settings: ExportSettings,
    ) -> Result<Self, PlaybackError> {
        let (sender, events) = crossbeam::channel::unbounded();
        let cancel = Arc::new(AtomicBool::new(false));
        let mut compositor = Compositor::new(&device, &queue);
        let keeps_alpha = settings
            .video
            .as_ref()
            .is_some_and(|video| match video.encoding {
                VideoEncoding::Movie(codec, _) => codec.keeps_alpha(),
                VideoEncoding::Stills(_) => false,
            });
        if keeps_alpha {
            compositor.set_background(Background::Transparent);
            settings.previews.clear();
        }
        let partial = match settings.video.map(|video| video.encoding) {
            Some(VideoEncoding::Stills(_)) => None,
            _ => Some(partial_path(&settings.path)),
        };
        let worker = Worker {
            compositor,
            partial,
            sequence,
            settings,
            events: sender,
            cancel: Arc::clone(&cancel),
            stills_written: Vec::new(),
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
    /// Where a movie is written until it is finished, so that a failed or cancelled
    /// export never touches a file already at the path. None for still sequences.
    partial: Option<PathBuf>,
    /// Every image a still sequence export has written, removed if it fails.
    stills_written: Vec<PathBuf>,
}

enum Outcome {
    Finished,
    Cancelled,
}

impl Worker {
    fn run(mut self) {
        let event = match self.render().and_then(|outcome| self.keep(outcome)) {
            Ok(Outcome::Finished) => ExportEvent::Finished,
            Ok(Outcome::Cancelled) => {
                self.remove_partial();
                ExportEvent::Cancelled
            }
            Err(error) => {
                self.remove_partial();
                ExportEvent::Failed(error)
            }
        };
        let _ = self.events.send(event);
    }

    fn keep(&self, outcome: Outcome) -> Result<Outcome, PlaybackError> {
        if let (Outcome::Finished, Some(partial)) = (&outcome, &self.partial) {
            project::rename_into_place(partial, &self.settings.path).map_err(|error| {
                PlaybackError::Replace {
                    path: self.settings.path.clone(),
                    error,
                }
            })?;
        }
        Ok(outcome)
    }

    fn remove_partial(&self) {
        if let Some(partial) = &self.partial {
            let _ = std::fs::remove_file(partial);
        }
        for still in &self.stills_written {
            let _ = std::fs::remove_file(still);
        }
    }

    /// How loud the exported span is; `None` if cancelled while measuring.
    fn measure(
        &self,
        sequence: &Sequence,
        exported: Range<Ticks>,
        rate: SampleRate,
    ) -> Option<crate::Loudness> {
        let mut mix = SequenceMix::new(rate);
        let total = (exported.end - exported.start)
            .rescale(sequence.timebase, mix.rate())
            .0;
        let mut done = 0;
        measure_span(&mut mix, sequence, exported, |length, warnings| {
            if self.cancel.load(Ordering::Relaxed) {
                return ControlFlow::Break(());
            }
            // Writing reads the same media again and reports what fails then.
            warnings.clear();
            done += length;
            let _ = self
                .events
                .send(ExportEvent::Measuring(done as f32 / total.max(1) as f32));
            ControlFlow::Continue(())
        })
    }

    fn render(&mut self) -> Result<Outcome, PlaybackError> {
        let sequence = Arc::clone(&self.sequence);
        let format = sequence.format;
        let (rate, timebase) = (format.frame_rate, sequence.timebase);
        let input_size = (format.width, format.height);
        let (video, stills) = match self.settings.video {
            Some(VideoExport {
                encoding: VideoEncoding::Movie(codec, rate_control),
                size,
                acceleration,
            }) => {
                let settings = VideoSettings {
                    input_size,
                    width: size.0,
                    height: size.1,
                    frame_rate: rate,
                    codec,
                    rate_control,
                    profile: EncodeProfile::Delivery,
                    acceleration,
                };
                (Some(settings), None)
            }
            Some(VideoExport {
                encoding: VideoEncoding::Stills(still),
                size,
                ..
            }) => (None, Some((still, size))),
            None => (None, None),
        };
        let audio = self.settings.audio.filter(|_| stills.is_none());
        let frames = frame_span(&sequence, self.settings.range.clone());
        let exported =
            rate.frame_start(frames.start, timebase)..rate.frame_start(frames.end, timebase);
        let gain = match (self.settings.loudness, audio) {
            (Some(target), Some(audio)) if stills.is_none() => {
                let rate = SampleRate(audio.sample_rate);
                match self.measure(&sequence, exported.clone(), rate) {
                    Some(measured) => target.gain(measured),
                    None => return Ok(Outcome::Cancelled),
                }
            }
            _ => 1.0,
        };
        let chapters = timeline::chapters(&sequence.markers, exported);
        let path = self.settings.path.clone();
        let writer = match stills {
            Some(_) => None,
            None => Some(MediaWriter::with_chapters(
                self.partial.as_deref().unwrap_or(&path),
                video,
                audio,
                &chapters,
                timebase,
            )?),
        };
        if let Some(encoder) = writer.as_ref().and_then(MediaWriter::video_encoder) {
            let _ = self.events.send(ExportEvent::Encoder(encoder));
        }
        let writes_audio = writer.is_some() && audio.is_some();
        let stills_to = stills.map(|(format, size)| Stills {
            format,
            input_size,
            size,
        });
        let mut sink = Sink::start(writer, stills_to)?;

        let frames_total = frames.end - frames.start;
        // A silent export never mixes, so its rate doesn't matter.
        let sample_rate = audio.map_or(48_000, |audio| audio.sample_rate);
        let mut mix = SequenceMix::new(SampleRate(sample_rate));
        let audio_rate = mix.rate();
        let to_audio = |time: Ticks| time.rescale(timebase, audio_rate).0;
        let audio_total = to_audio(rate.frame_start(frames.end, timebase));
        let mut audio_written = to_audio(rate.frame_start(frames.start, timebase));
        let mut tracks = Vec::new();
        let mut warned = Vec::new();
        let mut warnings = Vec::new();

        let pictures = video.is_some() || stills.is_some();
        let mut previews = PreviewPictures::new(&self.settings.previews, &sequence);
        let mut pending: Option<Readback> = None;
        let started = Instant::now();
        let mut timings = ExportTimings {
            frames: frames_total,
            ..ExportTimings::default()
        };
        // One step past the last frame hands over the last picture.
        for frame in frames.start..=frames.end {
            if self.cancel.load(Ordering::Relaxed) {
                return Ok(Outcome::Cancelled);
            }
            let mut since = Instant::now();
            // Decoding this frame overlaps the GPU finishing the one before.
            let layers = (pictures && frame < frames.end).then(|| {
                let pts = rate.frame_start(frame, timebase);
                previews::previewed(&mut previews, pts, Resolution::Full).unwrap_or_else(|| {
                    compose::layers_at(
                        &sequence,
                        pts,
                        &mut tracks,
                        &mut warned,
                        &mut warnings,
                        Resolution::Full,
                    )
                })
            });
            ExportTimings::lap(&mut timings.decode, &mut since);
            if frame > frames.start {
                let done = frame - 1;
                if let Some(readback) = pending.take() {
                    let picture = readback.finish()?;
                    ExportTimings::lap(&mut timings.gpu_wait, &mut since);
                    match stills {
                        Some((still, _)) => {
                            let file = still_path(&path, done - frames.start, frames_total, still);
                            self.stills_written.push(file.clone());
                            sink.send(Work::Still(file, picture))?;
                        }
                        None => sink.send(Work::Picture(picture))?,
                    }
                    ExportTimings::lap(&mut timings.writer_wait, &mut since);
                }

                // Audio keeps pace with the pictures so the muxer can interleave them.
                let audio_end = to_audio(rate.frame_start(frame, timebase)).min(audio_total);
                if writes_audio {
                    while audio_written < audio_end {
                        mix.mix_chunk(&sequence, audio_written, &mut warnings);
                        let frames = (audio_total - audio_written).min(CHUNK_FRAMES as i64);
                        let samples = usize::try_from(frames).unwrap_or(0) * 2;
                        let sound = mix.mix[..samples].iter().map(|sample| sample * gain);
                        let sound = sound.collect();
                        ExportTimings::lap(&mut timings.audio_mix, &mut since);
                        sink.send(Work::Sound(sound))?;
                        ExportTimings::lap(&mut timings.writer_wait, &mut since);
                        audio_written += frames;
                    }
                }

                for warning in warnings.drain(..) {
                    let _ = self.events.send(ExportEvent::Warning(warning));
                }
                let _ = self.events.send(ExportEvent::Progress {
                    frames_done: frame - frames.start,
                    frames_total,
                });
            }
            if let Some(layers) = layers {
                let images: Vec<LayerImage<'_>> = layers.iter().map(Layer::image).collect();
                self.compositor.composite(input_size, &images)?;
                pending = Some(self.compositor.start_readback()?);
                ExportTimings::lap(&mut timings.composite, &mut since);
            }
        }
        timings.writer = sink.finish()?;
        timings.total = started.elapsed();
        let _ = self.events.send(ExportEvent::Timings(timings));
        Ok(Outcome::Finished)
    }
}

/// The image file of the `index`th of `count` frames: the export's path with the
/// index, padded to the same width for every frame, before the extension, so the
/// images sort and import back as one sequence.
fn still_path(path: &Path, index: i64, count: i64, format: StillFormat) -> PathBuf {
    let digits = (count - 1).max(0).to_string().len().max(4);
    let stem = path
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
    path.with_file_name(format!("{stem}{index:0digits$}.{}", format.extension()))
}

/// Beside `path` with the same extension, which picks the container.
fn partial_path(path: &Path) -> PathBuf {
    let mut name = path.file_stem().unwrap_or_default().to_owned();
    name.push(".partial");
    if let Some(extension) = path.extension() {
        name.push(".");
        name.push(extension);
    }
    path.with_file_name(name)
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
            clips: vec![clip.into()],
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

    #[test]
    fn stills_are_numbered_to_one_width_after_the_export_path() {
        let path = Path::new("renders/shot.png");
        let still = |index, count| still_path(path, index, count, StillFormat::Tiff);
        assert_eq!(still(7, 240), Path::new("renders/shot0007.tif"));
        assert_eq!(still(7, 12_001), Path::new("renders/shot00007.tif"));
        assert_eq!(still(12_000, 12_001), Path::new("renders/shot12000.tif"));
    }
}
