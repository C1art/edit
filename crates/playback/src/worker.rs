use std::path::PathBuf;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use audio::{remix_stereo, AudioWriter, Resampler, SampleRate};
use crossbeam::channel::{Receiver, RecvTimeoutError, Sender, TryRecvError, TrySendError};
use media::{AudioBuffer, DecodedFrame, MediaReader, SourceInfo, VideoFrame};
use timeline::{Ticks, Timebase};

use crate::frame_picker::TaggedFrame;
use crate::PlaybackError;

/// How long the worker sleeps when both outputs are full, before checking them again.
const BACKOFF: Duration = Duration::from_millis(4);

#[derive(Debug)]
pub(crate) enum Command {
    Seek { position: Ticks, generation: u64 },
    Shutdown,
}

#[derive(Debug)]
pub(crate) enum Event {
    Opened(SourceInfo),
    /// The audio queue was emptied for this seek when the device had played `frames_played`.
    Seeked {
        generation: u64,
        frames_played: u64,
    },
    Ended {
        generation: u64,
    },
    Failed(PlaybackError),
}

pub(crate) struct Channels {
    pub(crate) commands: Receiver<Command>,
    pub(crate) events: Sender<Event>,
    pub(crate) frames: Sender<TaggedFrame>,
    /// The worker's own handle on the frame queue, for discarding frames a seek made stale.
    pub(crate) stale_frames: Receiver<TaggedFrame>,
}

pub(crate) fn spawn(
    path: PathBuf,
    timebase: Timebase,
    writer: Option<AudioWriter>,
    channels: Channels,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new().name("decode".into()).spawn(move || {
        let reader = match MediaReader::open(&path, timebase) {
            Ok(reader) => reader,
            Err(error) => {
                let _ = channels.events.send(Event::Failed(error.into()));
                return;
            }
        };
        let _ = channels.events.send(Event::Opened(reader.info().clone()));
        let audio = writer
            .filter(|_| reader.info().has_audio)
            .map(|writer| AudioPath::new(writer, timebase));
        Worker {
            reader,
            channels,
            audio,
            generation: 0,
            pending_video: None,
            finished: false,
        }
        .run();
    })
}

struct Worker {
    reader: MediaReader,
    channels: Channels,
    audio: Option<AudioPath>,
    generation: u64,
    pending_video: Option<VideoFrame>,
    finished: bool,
}

enum Flow {
    Continue,
    Exit,
}

impl Worker {
    fn run(mut self) {
        loop {
            if let Flow::Exit = self.handle_commands() {
                return;
            }
            let mut progressed = false;
            if let Some(frame) = self.pending_video.take() {
                let tagged = TaggedFrame {
                    generation: self.generation,
                    frame,
                };
                match self.channels.frames.try_send(tagged) {
                    Ok(()) => progressed = true,
                    Err(TrySendError::Full(tagged)) => self.pending_video = Some(tagged.frame),
                    Err(TrySendError::Disconnected(_)) => return,
                }
            }
            if let Some(audio) = &mut self.audio {
                progressed |= audio.push_pending();
            }
            let blocked = self.pending_video.is_some()
                || self.audio.as_ref().is_some_and(AudioPath::has_pending);
            if !blocked && !self.finished {
                self.decode_next();
                continue;
            }
            if progressed {
                continue;
            }
            let command = if blocked {
                match self.channels.commands.recv_timeout(BACKOFF) {
                    Ok(command) => Some(command),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            } else {
                match self.channels.commands.recv() {
                    Ok(command) => Some(command),
                    Err(_) => return,
                }
            };
            if let Some(command) = command {
                if let Flow::Exit = self.handle(command) {
                    return;
                }
            }
        }
    }

    /// Applies queued commands; of several queued seeks, only the last one is performed.
    fn handle_commands(&mut self) -> Flow {
        let mut seek = None;
        loop {
            match self.channels.commands.try_recv() {
                Ok(Command::Seek {
                    position,
                    generation,
                }) => seek = Some((position, generation)),
                Ok(Command::Shutdown) | Err(TryRecvError::Disconnected) => return Flow::Exit,
                Err(TryRecvError::Empty) => break,
            }
        }
        match seek {
            Some((position, generation)) => self.handle(Command::Seek {
                position,
                generation,
            }),
            None => Flow::Continue,
        }
    }

    fn handle(&mut self, command: Command) -> Flow {
        match command {
            Command::Shutdown => Flow::Exit,
            Command::Seek {
                position,
                generation,
            } => {
                self.seek(position, generation);
                Flow::Continue
            }
        }
    }

    fn seek(&mut self, position: Ticks, generation: u64) {
        self.generation = generation;
        self.pending_video = None;
        self.finished = false;
        while self.channels.stale_frames.try_recv().is_ok() {}
        let frames_played = self.audio.as_mut().map_or(0, |audio| audio.reset(position));
        if let Err(error) = self.reader.seek(position) {
            self.fail(error.into());
            return;
        }
        let _ = self.channels.events.send(Event::Seeked {
            generation,
            frames_played,
        });
    }

    fn decode_next(&mut self) {
        match self.reader.next_frame() {
            Ok(Some(DecodedFrame::Video(frame))) => self.pending_video = Some(frame),
            Ok(Some(DecodedFrame::Audio(buffer))) => {
                if let Some(audio) = &mut self.audio {
                    if let Err(error) = audio.accept(&buffer) {
                        self.fail(error);
                    }
                }
            }
            Ok(None) => {
                self.finished = true;
                if let Some(audio) = &mut self.audio {
                    if let Err(error) = audio.flush() {
                        self.fail(error);
                        return;
                    }
                }
                let _ = self.channels.events.send(Event::Ended {
                    generation: self.generation,
                });
            }
            Err(error) => self.fail(error.into()),
        }
    }

    fn fail(&mut self, error: PlaybackError) {
        self.finished = true;
        let _ = self.channels.events.send(Event::Failed(error));
    }
}

/// Carries decoded audio to the device: resampling and channel mapping happen here,
/// on the worker, so the audio callback only copies samples.
struct AudioPath {
    writer: AudioWriter,
    timebase: Timebase,
    resampler: Option<(u32, Resampler)>,
    /// Where the next buffer should start; gaps before it are filled with silence so
    /// the master clock stays aligned with media time.
    cursor: Ticks,
    stereo: Vec<f32>,
    pending: Vec<f32>,
    sent: usize,
}

impl AudioPath {
    fn new(writer: AudioWriter, timebase: Timebase) -> Self {
        Self {
            writer,
            timebase,
            resampler: None,
            cursor: Ticks::ZERO,
            stereo: Vec::new(),
            pending: Vec::new(),
            sent: 0,
        }
    }

    /// Returns the device frame count at the moment the queue was emptied.
    fn reset(&mut self, position: Ticks) -> u64 {
        self.writer.clear();
        let frames_played = self.writer.frames_played();
        if let Some((_, resampler)) = &mut self.resampler {
            resampler.reset();
        }
        self.cursor = position;
        self.pending.clear();
        self.sent = 0;
        frames_played
    }

    fn accept(&mut self, buffer: &AudioBuffer) -> Result<(), PlaybackError> {
        let format = self.writer.format();
        if self.resampler.as_ref().map(|(rate, _)| *rate) != Some(buffer.sample_rate) {
            self.flush()?;
            let resampler = Resampler::new(
                SampleRate(buffer.sample_rate),
                format.sample_rate,
                buffer.channels,
            )?;
            self.resampler = Some((buffer.sample_rate, resampler));
        }
        let Some((_, resampler)) = &mut self.resampler else {
            unreachable!("resampler was created above");
        };
        let gap = buffer.pts - self.cursor;
        if gap > Ticks::ZERO {
            let rate =
                Timebase::new(1, buffer.sample_rate).ok_or(audio::AudioError::InvalidFormat)?;
            let frames = usize::try_from(gap.rescale(self.timebase, rate).0).unwrap_or(0);
            let silence = vec![0.0; frames * usize::from(buffer.channels)];
            resampler.process(&silence, &mut self.stereo)?;
        }
        resampler.process(&buffer.samples, &mut self.stereo)?;
        self.cursor = self.cursor.max(buffer.end(self.timebase));
        self.drain_stereo();
        Ok(())
    }

    fn flush(&mut self) -> Result<(), PlaybackError> {
        if let Some((_, resampler)) = &mut self.resampler {
            resampler.flush(&mut self.stereo)?;
            self.drain_stereo();
        }
        Ok(())
    }

    fn drain_stereo(&mut self) {
        remix_stereo(
            &self.stereo,
            self.writer.format().channels,
            &mut self.pending,
        );
        self.stereo.clear();
    }

    fn has_pending(&self) -> bool {
        self.sent < self.pending.len()
    }

    /// Returns `true` if any samples were queued.
    fn push_pending(&mut self) -> bool {
        if !self.has_pending() {
            return false;
        }
        let pushed = self.writer.push(&self.pending[self.sent..]);
        self.sent += pushed;
        if self.sent == self.pending.len() {
            self.pending.clear();
            self.sent = 0;
        }
        pushed > 0
    }
}
