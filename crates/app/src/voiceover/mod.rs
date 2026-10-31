//! Records a voice from a microphone straight onto an audio track while the sequence
//! plays, as Premiere's voice-over record does.

mod dialog;
mod recording;

use std::ops::Range;
use std::path::Path;
use std::time::{Duration, Instant};

use audio::{AudioOutput, InputDevice, InputLevel};
use crossbeam::channel::{self, Receiver, TryRecvError};
use timeline::{AddTrack, ImportMedia, PlaceClips, PlaceMode, Ticks, TrackKind};

pub(crate) use dialog::dialog;
use recording::{Command, Destination, Event, Recording};

use crate::editor::Editor;

const COUNTDOWN: Duration = Duration::from_secs(3);
/// How often the list of microphones is asked for again while the setup is open, so
/// one plugged in meanwhile shows up.
const RELIST: Duration = Duration::from_secs(5);

/// The audio track a take goes on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrackChoice {
    Existing(usize),
    New,
}

pub(crate) struct Voiceover {
    /// The microphone to record from; the system's default when `None`.
    pub(crate) input: Option<InputDevice>,
    pub(crate) countdown: bool,
    /// Whether the sequence is heard while recording; off by default, as speakers
    /// would be recorded too.
    pub(crate) hear_sequence: bool,
    setup: Option<TrackChoice>,
    take: Option<Take>,
    listing: Option<Receiver<Vec<InputDevice>>>,
    listed_at: Option<Instant>,
    devices: Vec<InputDevice>,
}

impl Default for Voiceover {
    fn default() -> Self {
        Self {
            input: None,
            countdown: true,
            hear_sequence: false,
            setup: None,
            take: None,
            listing: None,
            listed_at: None,
            devices: Vec::new(),
        }
    }
}

struct Take {
    recording: Recording,
    track: TrackChoice,
    phase: Phase,
    level: Option<InputLevel>,
    /// The meter's reading, falling back slowly after a peak so it can be read.
    meter: f32,
    /// Whether the program was silenced for the take and needs its sound back.
    muted: bool,
    /// The loop turned off for the take, to turn back on after it.
    looped: Option<Range<Ticks>>,
}

enum Phase {
    Opening,
    Counting { until: Instant },
    Recording { at: Ticks, since: Instant },
    Saving { at: Ticks },
}

impl Voiceover {
    pub(crate) fn is_busy(&self) -> bool {
        self.setup.is_some() || self.take.is_some()
    }

    /// The microphones last found, asking the system again on a worker when the list
    /// has gone stale.
    fn devices(&mut self) -> &[InputDevice] {
        if let Some(listing) = &self.listing {
            match listing.try_recv() {
                Ok(devices) => self.devices = devices,
                Err(TryRecvError::Empty) => return &self.devices,
                Err(TryRecvError::Disconnected) => {}
            }
            self.listing = None;
            self.listed_at = Some(Instant::now());
        }
        if self.listed_at.is_none_or(|at| at.elapsed() >= RELIST) {
            let (sender, listing) = channel::bounded(1);
            let spawned = std::thread::Builder::new()
                .name("audio inputs".to_owned())
                .spawn(move || {
                    let _ = sender.send(audio::input_devices().unwrap_or_default());
                });
            match spawned {
                Ok(_) => self.listing = Some(listing),
                Err(_) => self.listed_at = Some(Instant::now()),
            }
        }
        &self.devices
    }
}

impl Editor {
    pub(crate) fn open_voiceover(&mut self) {
        if self.voiceover.is_busy() {
            return;
        }
        let sequence = self.timeline.sequence();
        let track = self
            .view
            .targets
            .audio
            .filter(|&index| index < sequence.audio.len())
            .or_else(|| (!sequence.audio.is_empty()).then_some(0))
            .map_or(TrackChoice::New, TrackChoice::Existing);
        self.voiceover.setup = Some(track);
    }

    fn start_voiceover(&mut self, track: TrackChoice, ctx: &egui::Context) {
        let sequence = self.timeline.sequence();
        let destination = Destination {
            folder: self
                .project_path
                .as_deref()
                .and_then(Path::parent)
                .map_or_else(crate::project_store::app_folder, Path::to_owned)
                .join("Voiceover"),
            timebase: sequence.timebase,
            frame_rate: sequence.format.frame_rate,
        };
        match Recording::open(self.voiceover.input.clone(), destination, ctx) {
            Ok(recording) => {
                self.voiceover.take = Some(Take {
                    recording,
                    track,
                    phase: Phase::Opening,
                    level: None,
                    meter: 0.0,
                    muted: false,
                    looped: None,
                });
            }
            Err(error) => self.notice(format!("Cannot record: {error}")),
        }
    }

    /// Moves a take along as its worker reports and its countdown runs out.
    fn tend_voiceover(&mut self) {
        let Some(take) = &mut self.voiceover.take else {
            return;
        };
        if let Some(level) = &take.level {
            take.meter = level.take_peak().max(take.meter * 0.85);
        }
        if let Phase::Counting { until } = take.phase {
            if Instant::now() >= until {
                self.begin_take();
            }
        }
        let Some(take) = &mut self.voiceover.take else {
            return;
        };
        match take.recording.events.try_recv() {
            Ok(Event::Opened(level)) => {
                take.level = Some(level);
                if self.voiceover.countdown {
                    take.phase = Phase::Counting {
                        until: Instant::now() + COUNTDOWN,
                    };
                } else {
                    self.begin_take();
                }
            }
            Ok(Event::Finished { media, warning }) => {
                let at = match take.phase {
                    Phase::Saving { at } | Phase::Recording { at, .. } => Some(at),
                    _ => None,
                };
                let track = take.track;
                // Stopping paused the program already, and it may have been played since;
                // a take that ended by itself, as when the microphone is unplugged, pauses it.
                if matches!(take.phase, Phase::Saving { .. }) {
                    take.phase = Phase::Opening;
                }
                self.end_take();
                if let Some(warning) = warning {
                    self.notice(warning);
                }
                if let Some(at) = at {
                    self.place_take(media, at, track);
                }
            }
            Ok(Event::Failed(message)) => {
                self.end_take();
                self.notice(message);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => self.end_take(),
        }
    }

    /// Starts the sequence and the microphone together at the playhead.
    fn begin_take(&mut self) {
        let hear = self.voiceover.hear_sequence;
        let Some(take) = &mut self.voiceover.take else {
            return;
        };
        let _ = take.recording.commands.send(Command::Start);
        let at = match &mut self.program {
            Some(program) => {
                // The take is placed as one run from where it starts, so the program
                // mustn't jump back to a loop's start under it.
                take.looped = program.player.looping();
                program.player.set_looping(None);
                if !hear {
                    take.muted = program.player.set_output(None).is_ok();
                }
                program.player.play();
                // Playing from the end starts over, so the take starts wherever play did.
                program.player.position()
            }
            None => Ticks::ZERO,
        };
        take.phase = Phase::Recording {
            at,
            since: Instant::now(),
        };
    }

    fn stop_take(&mut self) {
        let Some(take) = &mut self.voiceover.take else {
            return;
        };
        if let Phase::Recording { at, .. } = take.phase {
            let _ = take.recording.commands.send(Command::Stop);
            take.phase = Phase::Saving { at };
        }
        self.quiet_program();
    }

    /// Throws the take away; the worker deletes its file once it sees it's gone.
    fn discard_take(&mut self) {
        self.end_take();
    }

    fn end_take(&mut self) {
        self.quiet_program();
        self.voiceover.take = None;
    }

    /// Pauses the program and gives it back its sound if the take took it away.
    fn quiet_program(&mut self) {
        let Some(take) = &mut self.voiceover.take else {
            return;
        };
        let muted = std::mem::take(&mut take.muted);
        let looped = take.looped.take();
        let recording = matches!(take.phase, Phase::Recording { .. } | Phase::Saving { .. });
        let Some(program) = &mut self.program else {
            return;
        };
        if recording {
            program.player.pause();
        }
        if looped.is_some() {
            program.player.set_looping(looped);
        }
        if muted {
            let writer = self.audio.as_ref().map(AudioOutput::writer);
            if let Err(error) = program.player.set_output(writer) {
                self.notices.push(format!("No sound: {error}"));
            }
        }
    }

    fn place_take(&mut self, media: ImportMedia, at: Ticks, track: TrackChoice) {
        if !self.apply(media) {
            return;
        }
        let sequence = self.timeline.sequence();
        let Some(item) = sequence.media.last() else {
            return;
        };
        let source = item.id;
        let range = item.default_range(sequence.timebase);
        let track = match track {
            TrackChoice::Existing(index) if index < sequence.audio.len() => index,
            _ => {
                let index = sequence.audio.len();
                if !self.apply(AddTrack {
                    kind: TrackKind::Audio,
                }) {
                    return;
                }
                index
            }
        };
        let placed = self.apply(PlaceClips {
            pieces: vec![(source, range.start, range.end - range.start)],
            at,
            video_track: None,
            audio_track: Some(track),
            mode: PlaceMode::Overwrite,
        });
        if placed {
            self.notice(format!("Recorded the voiceover onto A{}", track + 1));
        }
    }
}
