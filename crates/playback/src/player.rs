use std::path::PathBuf;
use std::time::Instant;

use audio::AudioWriter;
use crossbeam::channel::{self, Receiver, Sender};
use media::{SourceInfo, VideoFrame};
use timeline::{Ticks, Timebase};

use crate::clock::PlaybackClock;
use crate::frame_picker::{FramePicker, TaggedFrame};
use crate::worker::{self, Channels, Command, Event};
use crate::PlaybackError;

/// Decoded frames buffered ahead of the clock; enough to absorb a slow decode or a
/// demuxer that interleaves audio well ahead of video.
const FRAME_QUEUE: usize = 12;

const NANOSECONDS: Timebase = match Timebase::new(1, 1_000_000_000) {
    Some(timebase) => timebase,
    None => unreachable!(),
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClockSource {
    Audio,
    Wall,
}

/// Plays one media file. Lives on the UI thread and never blocks: call
/// [`update`](Self::update) every frame to collect decoded output.
pub struct Player {
    commands: Sender<Command>,
    events: Receiver<Event>,
    frames: Receiver<TaggedFrame>,
    writer: Option<AudioWriter>,
    epoch: Instant,
    timebase: Timebase,
    clock: PlaybackClock,
    source: ClockSource,
    picker: FramePicker,
    generation: u64,
    awaiting_seek: bool,
    playing: bool,
    stream_ended: bool,
    info: Option<SourceInfo>,
    error: Option<PlaybackError>,
}

impl Player {
    /// Starts opening `path` on a worker thread. Without a `writer` the file plays silently
    /// against the wall clock.
    pub fn open(
        path: PathBuf,
        timebase: Timebase,
        writer: Option<AudioWriter>,
    ) -> Result<Self, PlaybackError> {
        let (command_tx, command_rx) = channel::unbounded();
        let (event_tx, event_rx) = channel::unbounded();
        let (frame_tx, frame_rx) = channel::bounded(FRAME_QUEUE);
        worker::spawn(
            path,
            timebase,
            writer.clone(),
            Channels {
                commands: command_rx,
                events: event_tx,
                frames: frame_tx,
                stale_frames: frame_rx.clone(),
            },
        )
        .map_err(PlaybackError::WorkerSpawn)?;
        Ok(Self {
            commands: command_tx,
            events: event_rx,
            frames: frame_rx,
            writer,
            epoch: Instant::now(),
            timebase,
            clock: PlaybackClock::new(timebase, NANOSECONDS),
            source: ClockSource::Wall,
            picker: FramePicker::default(),
            generation: 0,
            awaiting_seek: false,
            playing: false,
            stream_ended: false,
            info: None,
            error: None,
        })
    }

    pub fn info(&self) -> Option<&SourceInfo> {
        self.info.as_ref()
    }

    pub fn error(&self) -> Option<&PlaybackError> {
        self.error.as_ref()
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn current_frame(&self) -> Option<&VideoFrame> {
        self.picker.current()
    }

    /// Whether the UI must keep repainting to pick up output that is still arriving.
    pub fn is_busy(&self) -> bool {
        self.error.is_none() && (self.playing || self.awaiting_seek || self.info.is_none())
    }

    pub fn position(&self) -> Ticks {
        let position = self.clock.position(self.count());
        self.end().map_or(position, |end| position.min(end))
    }

    pub fn duration(&self) -> Option<Ticks> {
        self.info.as_ref().and_then(|info| info.duration)
    }

    /// Collects worker output and advances the displayed frame to the clock. Returns
    /// `true` when the frame to display changed.
    pub fn update(&mut self) -> bool {
        while let Ok(event) = self.events.try_recv() {
            self.handle(event);
        }
        if self.playing && self.stream_ended && self.source == ClockSource::Audio {
            let drained = self.writer.as_ref().is_none_or(AudioWriter::is_empty);
            if drained {
                // Audio ran out first; let video finish against the wall clock.
                let old = self.count();
                self.source = ClockSource::Wall;
                self.clock.switch_counter(old, NANOSECONDS, self.count());
            }
        }
        let position = self.clock.position(self.count());
        let frames = &self.frames;
        let changed = self.picker.advance(position, || frames.try_recv().ok());
        if self.playing && self.reached_end(position) {
            self.pause();
        }
        changed
    }

    pub fn play(&mut self) {
        if self.playing || self.info.is_none() || self.error.is_some() {
            return;
        }
        if self.reached_end(self.position()) {
            self.seek(Ticks::ZERO);
        }
        self.playing = true;
        if let Some(writer) = &self.writer {
            writer.set_paused(false);
        }
        if !self.awaiting_seek {
            self.clock.start(self.count());
        }
    }

    pub fn pause(&mut self) {
        if !self.playing {
            return;
        }
        self.playing = false;
        if let Some(writer) = &self.writer {
            writer.set_paused(true);
        }
        self.clock.stop(self.count());
    }

    pub fn toggle(&mut self) {
        if self.playing {
            self.pause();
        } else {
            self.play();
        }
    }

    pub fn seek(&mut self, position: Ticks) {
        if self.info.is_none() {
            return;
        }
        let position = match self.duration() {
            Some(duration) => position.clamp(Ticks::ZERO, duration),
            None => position.max(Ticks::ZERO),
        };
        self.generation += 1;
        self.awaiting_seek = true;
        self.stream_ended = false;
        if self.source == ClockSource::Wall && self.has_audio_output() {
            self.source = ClockSource::Audio;
        }
        self.clock = PlaybackClock::new(self.timebase, self.count_timebase());
        self.clock.jump(position);
        self.picker.reset(self.generation);
        let _ = self.commands.send(Command::Seek {
            position,
            generation: self.generation,
        });
    }

    /// Moves by whole frames from the frame on screen, pausing first.
    pub fn step(&mut self, frames: i64) {
        let Some(rate) = self
            .info
            .as_ref()
            .and_then(|info| info.video.as_ref()?.frame_rate)
        else {
            return;
        };
        self.pause();
        let from = self
            .current_frame()
            .map_or(self.position(), |frame| frame.pts);
        let frame = rate.frame_at(from, self.timebase).saturating_add(frames);
        self.seek(rate.frame_start(frame, self.timebase));
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Opened(info) => {
                self.source = if info.has_audio && self.writer.is_some() {
                    ClockSource::Audio
                } else {
                    ClockSource::Wall
                };
                self.info = Some(info);
                self.clock = PlaybackClock::new(self.timebase, self.count_timebase());
            }
            Event::Seeked {
                generation,
                frames_played,
            } if generation == self.generation => {
                self.awaiting_seek = false;
                if self.playing {
                    let count = match self.source {
                        ClockSource::Audio => frames_played,
                        ClockSource::Wall => self.count(),
                    };
                    self.clock.start(count);
                }
            }
            Event::Ended { generation } if generation == self.generation => {
                self.stream_ended = true;
            }
            Event::Failed(error) => {
                self.pause();
                self.error = Some(error);
            }
            Event::Seeked { .. } | Event::Ended { .. } => {}
        }
    }

    fn has_audio_output(&self) -> bool {
        self.writer.is_some() && self.info.as_ref().is_some_and(|info| info.has_audio)
    }

    fn count_timebase(&self) -> Timebase {
        match (self.source, &self.writer) {
            (ClockSource::Audio, Some(writer)) => {
                Timebase::new(1, writer.format().sample_rate.0).unwrap_or(NANOSECONDS)
            }
            _ => NANOSECONDS,
        }
    }

    fn count(&self) -> u64 {
        match (self.source, &self.writer) {
            (ClockSource::Audio, Some(writer)) => writer.frames_played(),
            _ => u64::try_from(self.epoch.elapsed().as_nanos()).unwrap_or(u64::MAX),
        }
    }

    /// The last instant of media: the declared duration, or the last frame when the
    /// container doesn't declare one and decoding has finished.
    fn end(&self) -> Option<Ticks> {
        self.duration().or_else(|| {
            let frame = self.current_frame().filter(|_| self.stream_ended)?;
            Some(frame.pts)
        })
    }

    fn reached_end(&self, position: Ticks) -> bool {
        self.stream_ended
            && !self.picker.has_pending()
            && self.frames.is_empty()
            && self.end().is_some_and(|end| position >= end)
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        if let Some(writer) = &self.writer {
            writer.set_paused(true);
        }
        // The worker notices the shutdown within one backoff interval and exits on its own;
        // joining here would stall the UI thread.
        let _ = self.commands.send(Command::Shutdown);
    }
}
