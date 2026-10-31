use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use audio::{remix_stereo, AudioWriter};
use crossbeam::channel::{Receiver, RecvTimeoutError, Sender};
use timeline::Sequence;

use crate::command::{coalesce, Coalesced, Command, Event, Seek};
use crate::master_stretch::MasterStretch;
use crate::sequence_mix::{SequenceMix, CHUNK_FRAMES};
use crate::shuttle::PlayRate;

/// How long the worker waits for room in the device queue before trying again.
const BACKOFF: Duration = Duration::from_millis(4);

/// A scrub sounds this many chunks, long enough to hear a word's sound, short enough
/// that a drag isn't left behind.
const SNIPPET_CHUNKS: usize = 2;

/// Frames over which a scrub snippet fades in and out, so it doesn't click.
const FADE_FRAMES: usize = 256;

pub(crate) struct AudioChannels {
    pub(crate) commands: Receiver<Command>,
    pub(crate) events: Sender<Event>,
    /// The player's newest seek, so a scrub only unpauses the device while still current.
    pub(crate) latest_seek: Arc<AtomicU64>,
}

pub(crate) fn spawn(
    sequence: Arc<Sequence>,
    writer: AudioWriter,
    channels: AudioChannels,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("audio-mix".into())
        .spawn(move || {
            AudioWorker {
                mix: SequenceMix::new(writer.format().sample_rate),
                sequence,
                writer,
                commands: channels.commands,
                events: channels.events,
                latest_seek: channels.latest_seek,
                position: 0,
                out: Vec::new(),
                sent: 0,
                running: false,
                snippet: None,
                stretch: None,
                stretched: Vec::new(),
            }
            .run();
        })
}

/// Mixes every audio track at the device rate and keeps the device queue full. Past the
/// end of the sequence it keeps sending silence so the master clock keeps running.
struct AudioWorker {
    sequence: Arc<Sequence>,
    writer: AudioWriter,
    commands: Receiver<Command>,
    events: Sender<Event>,
    latest_seek: Arc<AtomicU64>,
    mix: SequenceMix,
    /// The next output frame to mix, counted from the start of the sequence.
    position: i64,
    out: Vec<f32>,
    sent: usize,
    /// Nothing is produced until the first seek says where to start.
    running: bool,
    /// While scrubbing, the chunks of the snippet still to mix; otherwise audio is made
    /// to play on.
    snippet: Option<usize>,
    /// Off normal speed, what keeps the faster or slower sound at its pitch.
    stretch: Option<MasterStretch>,
    stretched: Vec<f32>,
}

impl AudioWorker {
    fn run(mut self) {
        loop {
            let idle = self.sent == self.out.len() && self.snippet == Some(0);
            let command = if !self.running || idle {
                match self.commands.recv() {
                    Ok(command) => Some(command),
                    Err(_) => return,
                }
            } else if self.sent < self.out.len() {
                let pushed = self.writer.push(&self.out[self.sent..]);
                self.sent += pushed;
                if pushed > 0 {
                    continue;
                }
                match self.commands.recv_timeout(BACKOFF) {
                    Ok(command) => Some(command),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            } else {
                match self.commands.try_recv() {
                    Ok(command) => Some(command),
                    Err(crossbeam::channel::TryRecvError::Empty) => {
                        self.mix_next();
                        None
                    }
                    Err(crossbeam::channel::TryRecvError::Disconnected) => return,
                }
            };
            let Some(command) = command else {
                continue;
            };
            let Some(Coalesced { sequence, seek, .. }) = coalesce(command, &self.commands) else {
                return;
            };
            if let Some(sequence) = sequence {
                self.sequence = sequence;
            }
            if let Some(seek) = seek {
                self.seek(seek);
            }
        }
    }

    fn seek(&mut self, seek: Seek) {
        // Snippets of a continuing scrub queue up behind one another rather than cutting
        // the last one off with a click; anything else queued is stale.
        let scrubbing_on = seek.scrub && self.snippet.is_some();
        if !scrubbing_on {
            self.writer.clear();
        }
        let frames_played = self.writer.frames_played();
        self.position = seek
            .position
            .rescale(self.sequence.timebase, self.mix.rate())
            .0;
        self.out.clear();
        self.sent = 0;
        self.running = true;
        self.mix.reset();
        self.stretch = (!seek.scrub && seek.speed != PlayRate::NORMAL)
            .then(|| MasterStretch::new(seek.speed.0.unsigned_abs()));
        self.snippet = seek.scrub.then(|| {
            // A drag outrunning playback skips positions instead of falling behind.
            let behind = self.writer.queued_frames() >= SNIPPET_CHUNKS * CHUNK_FRAMES;
            if behind {
                0
            } else {
                SNIPPET_CHUNKS
            }
        });
        if seek.scrub {
            let latest = &self.latest_seek;
            self.writer
                .resume_if(|| latest.load(Ordering::Acquire) == seek.generation);
        }
        let _ = self.events.send(Event::Seeked {
            generation: seek.generation,
            frames_played,
        });
    }

    fn mix_next(&mut self) {
        let sequence = Arc::clone(&self.sequence);
        let mut warnings = Vec::new();
        let levels = self.mix.mix_chunk(&sequence, self.position, &mut warnings);
        for warning in warnings {
            let _ = self.events.send(Event::Warning(warning));
        }
        let _ = self.events.send(Event::Levels(levels));
        self.out.clear();
        let channels = self.writer.format().channels;
        match &mut self.stretch {
            Some(stretch) => {
                self.stretched.clear();
                stretch.push(&self.mix.mix, &mut self.stretched);
                remix_stereo(&self.stretched, channels, &mut self.out);
            }
            None => remix_stereo(&self.mix.mix, channels, &mut self.out),
        }
        if let Some(left) = &mut self.snippet {
            fade(&mut self.out, channels, *left == SNIPPET_CHUNKS, *left == 1);
            *left = left.saturating_sub(1);
        }
        self.sent = 0;
        self.position += CHUNK_FRAMES as i64;
    }
}

/// Ramps the start and/or end of an interleaved chunk from and to silence.
fn fade(out: &mut [f32], channels: u16, start: bool, end: bool) {
    let frames = out.len() / usize::from(channels).max(1);
    let ramp = FADE_FRAMES.min(frames).max(1) as f32;
    for (index, frame) in out.chunks_mut(usize::from(channels).max(1)).enumerate() {
        let rising = if start { index as f32 / ramp } else { 1.0 };
        let falling = if end {
            (frames - 1 - index) as f32 / ramp
        } else {
            1.0
        };
        let gain = rising.min(falling).min(1.0);
        frame.iter_mut().for_each(|sample| *sample *= gain);
    }
}
