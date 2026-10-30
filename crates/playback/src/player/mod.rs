use std::collections::VecDeque;
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use audio::AudioWriter;
use crossbeam::channel::{self, Receiver, Sender};
use timeline::{FrameRate, Sequence, Ticks, Timebase};

mod moves;
mod output;

use crate::clock::{PlaybackClock, SharedPosition};
use crate::command::{Command, Event, Seek};
use crate::frame_picker::{FramePicker, TaggedFrame};
use crate::shuttle::{PlayRate, Shuttle};
use crate::{
    audio_worker, video_worker, ComposedFrame, Levels, PlaybackError, Preview, Resolution,
};

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
    /// `generation`, shared with the audio worker.
    latest_seek: Arc<AtomicU64>,
    awaiting_seek: bool,
    playing: bool,
    video_ended: bool,
    warnings: Vec<PlaybackError>,
    /// Meter readings for audio not yet heard, oldest first.
    pending_levels: VecDeque<Levels>,
    levels: Option<Levels>,
    /// Whether the video worker decodes media items' proxies instead of their files.
    proxies: bool,
    resolution: Resolution,
    previews: Arc<Vec<Preview>>,
    shuttle: Option<Shuttle>,
    /// While set, playing wraps from this range's end back to its start.
    looping: Option<Range<Ticks>>,
    stop: Option<Stop>,
    scrub_audio: bool,
    /// Whether the last move was a scrub, which leaves no audio queued to play on with.
    scrubbed: bool,
    playing_at: Arc<SharedPosition>,
    /// The frame last shown while playing, to count the frames skipped after it.
    last_shown: Option<Ticks>,
    dropped: u64,
}

/// Where playback started by [`Player::play_range`] ends.
#[derive(Debug, Clone, Copy)]
struct Stop {
    at: Ticks,
    /// Where the playhead goes back to once stopped, as Play Around leaves it.
    back_to: Option<Ticks>,
}

impl Player {
    pub fn new(
        sequence: Arc<Sequence>,
        writer: Option<AudioWriter>,
    ) -> Result<Self, PlaybackError> {
        let (video_tx, video_rx) = channel::unbounded();
        let (event_tx, event_rx) = channel::unbounded();
        let (frame_tx, frame_rx) = channel::bounded(FRAME_QUEUE);
        let latest_seek = Arc::new(AtomicU64::new(0));
        let playing_at = Arc::new(SharedPosition::new());
        let audio_commands = match &writer {
            Some(writer) => {
                let (audio_tx, audio_rx) = channel::unbounded();
                audio_worker::spawn(
                    Arc::clone(&sequence),
                    writer.clone(),
                    audio_worker::AudioChannels {
                        commands: audio_rx,
                        events: event_tx.clone(),
                        latest_seek: Arc::clone(&latest_seek),
                    },
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
                playing_at: Arc::clone(&playing_at),
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
            latest_seek,
            awaiting_seek: false,
            playing: false,
            video_ended: false,
            warnings: Vec::new(),
            pending_levels: VecDeque::new(),
            levels: None,
            proxies: false,
            resolution: Resolution::Full,
            previews: Arc::new(Vec::new()),
            shuttle: None,
            looping: None,
            stop: None,
            scrub_audio: true,
            playing_at,
            last_shown: None,
            dropped: 0,
            scrubbed: false,
        };
        player.seek(Ticks::ZERO);
        Ok(player)
    }

    pub fn sequence(&self) -> &Arc<Sequence> {
        &self.sequence
    }

    /// Plays `sequence` from the current position from now on, e.g. after an edit.
    pub fn set_sequence(&mut self, sequence: Arc<Sequence>) {
        self.send_sequence(sequence);
        self.seek(self.position());
    }

    /// Shows proxies, where media items have them, in place of their full-size pictures.
    pub fn set_proxies(&mut self, enabled: bool) {
        if self.proxies != enabled {
            self.proxies = enabled;
            self.set_sequence(Arc::clone(&self.sequence));
        }
    }

    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    pub fn set_resolution(&mut self, resolution: Resolution) {
        if self.resolution != resolution {
            self.resolution = resolution;
            let _ = self.video_commands.send(Command::Resolution(resolution));
            self.seek(self.position());
        }
    }

    /// Plays these rendered files in place of the pictures they cover, from the next
    /// frame composed.
    pub fn set_previews(&mut self, previews: Vec<Preview>) {
        self.previews = Arc::new(previews);
        let _ = self
            .video_commands
            .send(Command::Previews(Arc::clone(&self.previews)));
    }

    /// Plays `sequence` without seeking, for edits that change only levels, so faders
    /// can be heard while they move. Audio already queued keeps its old levels.
    pub fn set_mix(&mut self, sequence: Arc<Sequence>) {
        self.send_sequence(sequence);
    }

    fn send_sequence(&mut self, sequence: Arc<Sequence>) {
        self.sequence = Arc::clone(&sequence);
        if let Some(audio) = &self.audio_commands {
            let _ = audio.send(Command::Sequence(Arc::clone(&sequence)));
        }
        let pictures = if self.proxies {
            Arc::new(sequence.with_proxies())
        } else {
            sequence
        };
        let _ = self.video_commands.send(Command::Sequence(pictures));
    }

    /// The meter reading for the audio being heard, while playing.
    pub fn levels(&self) -> Option<&Levels> {
        self.levels.as_ref().filter(|_| self.playing)
    }

    /// Whether playing at any speed, including shuttling.
    pub fn is_playing(&self) -> bool {
        self.playing || self.shuttle.is_some()
    }

    pub fn rate(&self) -> PlayRate {
        match self.shuttle {
            Some(shuttle) => shuttle.rate(),
            None if self.playing => PlayRate::NORMAL,
            None => PlayRate::STOPPED,
        }
    }

    pub fn current_frame(&self) -> Option<&ComposedFrame> {
        self.picker.current()
    }

    /// Frames skipped to keep up since playing last started.
    pub fn dropped_frames(&self) -> u64 {
        self.dropped
    }

    fn count_dropped(&mut self) {
        let Some(shown) = self.picker.current().map(|frame| frame.pts) else {
            return;
        };
        let frame = |time| self.frame_rate().frame_at(time, self.sequence.timebase);
        if let Some(last) = self.last_shown.filter(|&last| last < shown) {
            let skipped = frame(shown) - frame(last) - 1;
            self.dropped += u64::try_from(skipped).unwrap_or(0);
        }
        self.last_shown = Some(shown);
    }

    /// Problems with individual media since the last call, for the UI to report.
    pub fn take_warnings(&mut self) -> Vec<PlaybackError> {
        std::mem::take(&mut self.warnings)
    }

    /// Whether the UI must keep repainting to pick up output that is still arriving.
    /// Frames queued for later times wait on a clock that only runs while playing.
    pub fn is_busy(&self) -> bool {
        self.is_playing() || self.awaiting_seek || (!self.picker.is_settled() && !self.video_ended)
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
        self.playing_at.set(self.playing.then_some(position));
        if changed && self.playing {
            self.count_dropped();
        }
        self.advance_shuttle();
        if self.playing {
            self.keep_in_range(position);
        }
        if self.playing && self.reached_end(position) {
            self.pause();
        }
        changed
    }

    pub fn play(&mut self) {
        self.shuttle = None;
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
        if let Some(range) = self
            .looping
            .clone()
            .filter(|range| !range.contains(&self.position()))
        {
            self.go_to(range.start, false);
        } else if self.reached_end(self.position()) {
            self.seek(Ticks::ZERO);
        } else if reclaimed || self.scrubbed {
            // Another player used the device meanwhile, or scrubbing played out what was
            // queued, so the audio must be made again from here.
            self.seek(self.position());
        }
        self.playing = true;
        self.dropped = 0;
        self.last_shown = None;
        if let Some(writer) = &self.writer {
            writer.set_paused(false);
        }
        if !self.awaiting_seek {
            self.clock.start(self.count());
        }
    }

    pub fn pause(&mut self) {
        self.shuttle = None;
        self.stop = None;
        if !self.playing {
            return;
        }
        self.playing = false;
        self.playing_at.set(None);
        if let Some(writer) = &self.writer {
            writer.set_paused(true);
        }
        self.clock.stop(self.count());
    }

    pub fn toggle(&mut self) {
        if self.is_playing() {
            self.pause();
        } else {
            self.play();
        }
    }

    pub fn seek(&mut self, position: Ticks) {
        self.shuttle = None;
        self.stop = None;
        self.go_to(position, false);
    }

    fn go_to(&mut self, position: Ticks, scrub: bool) {
        let position = position.clamp(Ticks::ZERO, self.duration());
        // Until the clock has moved there, the video worker mustn't skip toward the old position.
        self.playing_at.set(None);
        self.last_shown = None;
        self.generation += 1;
        self.latest_seek.store(self.generation, Ordering::Release);
        self.scrubbed = scrub;
        self.awaiting_seek = true;
        self.video_ended = false;
        let was_playing = self.playing;
        self.clock.stop(self.count());
        self.clock.jump(position);
        self.playing = was_playing;
        self.picker.reset(self.generation);
        self.pending_levels.clear();
        self.levels = None;
        self.send(Command::Seek(Seek {
            position,
            generation: self.generation,
            scrub,
        }));
        // A scrub may have left the device running; audio readied while paused waits.
        if !self.playing && !scrub {
            if let Some(writer) = &self.writer {
                writer.set_paused(true);
            }
        }
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
