use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use audio::{remix_stereo, AudioWriter};
use crossbeam::channel::{Receiver, RecvTimeoutError, Sender};
use timeline::{Sequence, SourceId, Ticks, Timebase};

use crate::command::{coalesce, Coalesced, Command, Event};
use crate::mixdown;
use crate::track_audio::TrackAudio;
use crate::Levels;

/// How long the worker waits for room in the device queue before trying again.
const BACKOFF: Duration = Duration::from_millis(4);

/// Frames mixed per step: small enough to start quickly after a seek, large enough to
/// keep per-chunk overhead low.
const CHUNK_FRAMES: usize = 1024;

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
                rate: Timebase::new(1, writer.format().sample_rate.0).unwrap_or(Timebase::HZ_90K),
                sequence,
                writer,
                commands,
                events,
                tracks: Vec::new(),
                position: 0,
                mix: vec![0.0; CHUNK_FRAMES * 2],
                track_mix: vec![0.0; CHUNK_FRAMES * 2],
                clip_mix: vec![0.0; CHUNK_FRAMES * 2],
                out: Vec::new(),
                sent: 0,
                running: false,
                warned: Vec::new(),
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
    /// One tick per output frame.
    rate: Timebase,
    tracks: Vec<TrackAudio>,
    /// The next output frame to mix, counted from the start of the sequence.
    position: i64,
    mix: Vec<f32>,
    track_mix: Vec<f32>,
    clip_mix: Vec<f32>,
    out: Vec<f32>,
    sent: usize,
    /// Nothing is produced until the first seek says where to start.
    running: bool,
    warned: Vec<SourceId>,
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
            let Some(Coalesced { sequence, seek }) = coalesce(command, &self.commands) else {
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
        self.position = position.rescale(self.sequence.timebase, self.rate).0;
        self.out.clear();
        self.sent = 0;
        self.running = true;
        let (timebase, rate) = (self.sequence.timebase, self.writer.format().sample_rate);
        self.tracks.resize_with(self.sequence.audio.len(), || {
            TrackAudio::new(timebase, rate)
        });
        self.tracks.iter_mut().for_each(TrackAudio::reset);
        let _ = self.events.send(Event::Seeked {
            generation,
            frames_played,
        });
    }

    fn mix_next(&mut self) {
        let sequence = Arc::clone(&self.sequence);
        let to_frames = |ticks: Ticks| ticks.rescale(sequence.timebase, self.rate).0;
        let chunk_start = self.position;
        let chunk_end = chunk_start + CHUNK_FRAMES as i64;
        let (timebase, rate) = (sequence.timebase, self.writer.format().sample_rate);
        self.tracks
            .resize_with(sequence.audio.len(), || TrackAudio::new(timebase, rate));
        self.mix.fill(0.0);
        let mut track_levels = Vec::with_capacity(sequence.audio.len());
        for (track, decoder) in sequence.audio.iter().zip(&mut self.tracks) {
            self.track_mix.fill(0.0);
            if track.muted {
                track_levels.push([0.0; 2]);
                continue;
            }
            for clip in &track.clips {
                let (clip_start, clip_end) = (to_frames(clip.start), to_frames(clip.end()));
                let (from, to) = (chunk_start.max(clip_start), chunk_end.min(clip_end));
                if from >= to {
                    continue;
                }
                let Some(media) = sequence.media_item(clip.source) else {
                    continue;
                };
                let first = from - clip_start + to_frames(clip.source_in);
                let offset = usize::try_from(from - chunk_start).unwrap_or(0) * 2;
                let length = usize::try_from(to - from).unwrap_or(0) * 2;
                let clip_mix = &mut self.clip_mix[..length];
                clip_mix.fill(0.0);
                if let Err(error) = decoder.mix_into(media, first, clip_mix) {
                    if !self.warned.contains(&media.id) {
                        self.warned.push(media.id);
                        let _ = self.events.send(Event::Warning(error));
                    }
                }
                let gain = clip.gain.amplitude();
                mixdown::add_scaled(
                    clip_mix,
                    [gain, gain],
                    &mut self.track_mix[offset..offset + length],
                );
            }
            let volume = track.volume.amplitude();
            let [left, right] = track.pan.balance();
            track_levels.push(mixdown::add_scaled(
                &self.track_mix,
                [left * volume, right * volume],
                &mut self.mix,
            ));
        }
        let master = mixdown::scale_in_place(&mut self.mix, sequence.master.amplitude());
        let _ = self.events.send(Event::Levels(Levels {
            pts: Ticks(chunk_start).rescale(self.rate, sequence.timebase),
            tracks: track_levels,
            master,
        }));
        self.out.clear();
        remix_stereo(&self.mix, self.writer.format().channels, &mut self.out);
        self.sent = 0;
        self.position = chunk_end;
    }
}
