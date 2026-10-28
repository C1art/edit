use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use audio::AudioWriter;
use crossbeam::channel::{self, Receiver, Sender};
use timeline::{FrameRate, Sequence, Ticks, Timebase};

use crate::clock::PlaybackClock;
use crate::command::{Command, Event};
use crate::frame_picker::{FramePicker, TaggedFrame};
use crate::{audio_worker, video_worker, ComposedFrame, Levels, PlaybackError};

/// Composed frames buffered ahead of the clock, enough to absorb a slow decode.
const FRAME_QUEUE: usize = 8;

/// Meter readings kept waiting for the clock; more than the device queue ever holds.
const LEVELS_QUEUE: usize = 256;

const NANOSECONDS: Timebase = match Timebase::new(1, 1_000_000_000) {
    Some(timebase) => timebase,
    None => unreachable!(),
};

/// Plays a sequence. Lives on the UI thread and never blocks: call
/// [`update`](Self::update) every frame to collect what the workers produced.
///
/// With an audio writer, audio is the master clock; without one, the wall clock is.
pub struct Player {
    video_commands: Sender<Command>,
    audio_commands: Option<Sender<Command>>,
    events: Receiver<Event>,
    frames: Receiver<TaggedFrame>,
    writer: Option<AudioWriter>,
    epoch: Instant,
    sequence: Arc<Sequence>,
    clock: PlaybackClock,
    picker: FramePicker,
    generation: u64,
    awaiting_seek: bool,
    playing: bool,
    video_ended: bool,
    warnings: Vec<PlaybackError>,
    /// Meter readings for audio not yet heard, oldest first.
    pending_levels: VecDeque<Levels>,
    levels: Option<Levels>,
}

impl Player {
    pub fn new(
        sequence: Arc<Sequence>,
        writer: Option<AudioWriter>,
    ) -> Result<Self, PlaybackError> {
        let (video_tx, video_rx) = channel::unbounded();
        let (event_tx, event_rx) = channel::unbounded();
        let (frame_tx, frame_rx) = channel::bounded(FRAME_QUEUE);
        let audio_commands = match &writer {
            Some(writer) => {
                let (audio_tx, audio_rx) = channel::unbounded();
                audio_worker::spawn(
                    Arc::clone(&sequence),
                    writer.clone(),
                    audio_rx,
                    event_tx.clone(),
                )
                .map_err(PlaybackError::WorkerSpawn)?;
                Some(audio_tx)
            }
            None => None,
        };
        video_worker::spawn(
            Arc::clone(&sequence),
            video_worker::VideoChannels {
                commands: video_rx,
                events: event_tx,
                frames: frame_tx,
                stale_frames: frame_rx.clone(),
                reports_seeks: writer.is_none(),
            },
        )
        .map_err(PlaybackError::WorkerSpawn)?;
        let count_timebase = writer
            .as_ref()
            .and_then(|writer| Timebase::new(1, writer.format().sample_rate.0))
            .unwrap_or(NANOSECONDS);
        let mut player = Self {
            video_commands: video_tx,
            audio_commands,
            events: event_rx,
            frames: frame_rx,
            writer,
            epoch: Instant::now(),
            clock: PlaybackClock::new(sequence.timebase, count_timebase),
            sequence,
            picker: FramePicker::default(),
            generation: 0,
            awaiting_seek: false,
            playing: false,
            video_ended: false,
            warnings: Vec::new(),
            pending_levels: VecDeque::new(),
            levels: None,
        };
        player.seek(Ticks::ZERO);
        Ok(player)
    }

    pub fn sequence(&self) -> &Arc<Sequence> {
        &self.sequence
    }

    /// Plays `sequence` from the current position from now on, e.g. after an edit.
    pub fn set_sequence(&mut self, sequence: Arc<Sequence>) {
        self.sequence = Arc::clone(&sequence);
        self.send(Command::Sequence(sequence));
        self.seek(self.position());
    }

    /// Plays `sequence` without seeking, for edits that change only levels, so faders
    /// can be heard while they move. Audio already queued keeps its old levels.
    pub fn set_mix(&mut self, sequence: Arc<Sequence>) {
        self.sequence = Arc::clone(&sequence);
        self.send(Command::Sequence(sequence));
    }

    /// The meter reading for the audio being heard, while playing.
    pub fn levels(&self) -> Option<&Levels> {
        self.levels.as_ref().filter(|_| self.playing)
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn current_frame(&self) -> Option<&ComposedFrame> {
        self.picker.current()
    }

    /// Problems with individual media since the last call, for the UI to report.
    pub fn take_warnings(&mut self) -> Vec<PlaybackError> {
        std::mem::take(&mut self.warnings)
    }

    /// Whether the UI must keep repainting to pick up output that is still arriving.
    pub fn is_busy(&self) -> bool {
        self.playing || self.awaiting_seek || self.picker.has_pending() || !self.frames.is_empty()
    }

    pub fn position(&self) -> Ticks {
        self.clock.position(self.count()).min(self.duration())
    }

    pub fn duration(&self) -> Ticks {
        self.sequence.duration()
    }

    pub fn frame_rate(&self) -> FrameRate {
        self.sequence.format.frame_rate
    }

    /// Collects worker output and advances the displayed frame to the clock. Returns
    /// `true` when the frame to display changed.
    pub fn update(&mut self) -> bool {
        while let Ok(event) = self.events.try_recv() {
            self.handle(event);
        }
        let position = self.clock.position(self.count());
        while let Some(levels) = self
            .pending_levels
            .pop_front_if(|levels| levels.pts <= position)
        {
            self.levels = Some(levels);
        }
        let frames = &self.frames;
        let changed = self.picker.advance(position, || frames.try_recv().ok());
        if self.playing && self.reached_end(position) {
            self.pause();
        }
        changed
    }

    pub fn play(&mut self) {
        if self.playing || self.duration() == Ticks::ZERO {
            return;
        }
        let reclaimed = self.writer.as_ref().is_some_and(|writer| {
            let lost = !writer.is_active();
            if lost {
                writer.claim();
            }
            lost
        });
        if self.reached_end(self.position()) {
            self.seek(Ticks::ZERO);
        } else if reclaimed {
            // Another player used the device meanwhile, so this one's queued audio is gone.
            self.seek(self.position());
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
        let position = position.clamp(Ticks::ZERO, self.duration());
        self.generation += 1;
        self.awaiting_seek = true;
        self.video_ended = false;
        let was_playing = self.playing;
        self.clock.stop(self.count());
        self.clock.jump(position);
        self.playing = was_playing;
        self.picker.reset(self.generation);
        self.pending_levels.clear();
        self.levels = None;
        self.send(Command::Seek {
            position,
            generation: self.generation,
        });
    }

    /// Moves by whole frames from the frame on screen, pausing first.
    pub fn step(&mut self, frames: i64) {
        self.pause();
        let rate = self.frame_rate();
        let from = self
            .current_frame()
            .map_or(self.position(), |frame| frame.pts);
        let frame = rate
            .frame_at(from, self.sequence.timebase)
            .saturating_add(frames);
        self.seek(rate.frame_start(frame, self.sequence.timebase));
    }

    fn send(&self, command: Command) {
        if let Some(audio) = &self.audio_commands {
            let _ = audio.send(command.clone());
        }
        let _ = self.video_commands.send(command);
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Seeked {
                generation,
                frames_played,
            } if generation == self.generation => {
                self.awaiting_seek = false;
                if self.playing {
                    let count = match self.writer {
                        Some(_) => frames_played,
                        None => self.count(),
                    };
                    self.clock.start(count);
                }
            }
            Event::Ended { generation } if generation == self.generation => {
                self.video_ended = true;
            }
            Event::Warning(error) => self.warnings.push(error),
            Event::Levels(levels) => {
                if self.pending_levels.len() == LEVELS_QUEUE {
                    self.pending_levels.pop_front();
                }
                self.pending_levels.push_back(levels);
            }
            Event::Seeked { .. } | Event::Ended { .. } => {}
        }
    }

    fn count(&self) -> u64 {
        match &self.writer {
            Some(writer) => writer.frames_played(),
            None => u64::try_from(self.epoch.elapsed().as_nanos()).unwrap_or(u64::MAX),
        }
    }

    fn reached_end(&self, position: Ticks) -> bool {
        self.video_ended
            && !self.picker.has_pending()
            && self.frames.is_empty()
            && position >= self.duration()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        if let Some(writer) = &self.writer {
            writer.set_paused(true);
        }
        // Workers notice the shutdown within one backoff interval and exit on their own;
        // joining here would stall the UI thread.
        self.send(Command::Shutdown);
    }
}
