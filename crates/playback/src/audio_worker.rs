use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use audio::{remix_stereo, AudioWriter};
use crossbeam::channel::{Receiver, RecvTimeoutError, Sender};
use timeline::{Sequence, Ticks};

use crate::command::{coalesce, Coalesced, Command, Event};
use crate::sequence_mix::{SequenceMix, CHUNK_FRAMES};

/// How long the worker waits for room in the device queue before trying again.
const BACKOFF: Duration = Duration::from_millis(4);

pub(crate) fn spawn(
    sequence: Arc<Sequence>,
    writer: AudioWriter,
    commands: Receiver<Command>,
    events: Sender<Event>,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("audio-mix".into())
        .spawn(move || {
            AudioWorker {
                mix: SequenceMix::new(writer.format().sample_rate),
                sequence,
                writer,
                commands,
                events,
                position: 0,
                out: Vec::new(),
                sent: 0,
                running: false,
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
    mix: SequenceMix,
    /// The next output frame to mix, counted from the start of the sequence.
    position: i64,
    out: Vec<f32>,
    sent: usize,
    /// Nothing is produced until the first seek says where to start.
    running: bool,
}

impl AudioWorker {
    fn run(mut self) {
        loop {
            let command = if !self.running {
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
            if let Some((position, generation)) = seek {
                self.seek(position, generation);
            }
        }
    }

    fn seek(&mut self, position: Ticks, generation: u64) {
        self.writer.clear();
        let frames_played = self.writer.frames_played();
        self.position = position.rescale(self.sequence.timebase, self.mix.rate()).0;
        self.out.clear();
        self.sent = 0;
        self.running = true;
        self.mix.reset();
        let _ = self.events.send(Event::Seeked {
            generation,
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
        remix_stereo(&self.mix.mix, self.writer.format().channels, &mut self.out);
        self.sent = 0;
        self.position += CHUNK_FRAMES as i64;
    }
}
