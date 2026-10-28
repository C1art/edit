use audio::SampleRate;
use timeline::{Sequence, SourceId, Ticks, Timebase};

use crate::mixdown;
use crate::track_audio::TrackAudio;
use crate::{Levels, PlaybackError};

/// Stereo frames mixed per step: small enough to start quickly after a seek, large
/// enough to keep per-chunk overhead low.
pub(crate) const CHUNK_FRAMES: usize = 1024;

/// Mixes every audio track of a sequence, with clip gain, track volume and balance and
/// master gain, into interleaved stereo at one output rate.
pub(crate) struct SequenceMix {
    sample_rate: SampleRate,
    /// One tick per output frame.
    rate: Timebase,
    tracks: Vec<TrackAudio>,
    /// The last chunk mixed.
    pub(crate) mix: Vec<f32>,
    track_mix: Vec<f32>,
    clip_mix: Vec<f32>,
    /// Media already reported as unreadable.
    warned: Vec<SourceId>,
}

impl SequenceMix {
    pub(crate) fn new(sample_rate: SampleRate) -> Self {
        Self {
            sample_rate,
            rate: Timebase::new(1, sample_rate.0).unwrap_or(Timebase::HZ_90K),
            tracks: Vec::new(),
            mix: vec![0.0; CHUNK_FRAMES * 2],
            track_mix: vec![0.0; CHUNK_FRAMES * 2],
            clip_mix: vec![0.0; CHUNK_FRAMES * 2],
            warned: Vec::new(),
        }
    }

    pub(crate) fn rate(&self) -> Timebase {
        self.rate
    }

    /// Forgets decode positions, so the next chunk seeks.
    pub(crate) fn reset(&mut self) {
        self.tracks.iter_mut().for_each(TrackAudio::reset);
    }

    /// Mixes the chunk starting at output frame `start` into `self.mix` and returns its
    /// levels. Media that can't be read is reported once and left silent.
    pub(crate) fn mix_chunk(
        &mut self,
        sequence: &Sequence,
        start: i64,
        warnings: &mut Vec<PlaybackError>,
    ) -> Levels {
        let to_frames = |ticks: Ticks| ticks.rescale(sequence.timebase, self.rate).0;
        let end = start + CHUNK_FRAMES as i64;
        let (timebase, sample_rate) = (sequence.timebase, self.sample_rate);
        self.tracks.resize_with(sequence.audio.len(), || {
            TrackAudio::new(timebase, sample_rate)
        });
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
                let (from, to) = (start.max(clip_start), end.min(clip_end));
                if from >= to {
                    continue;
                }
                let Some(media) = sequence.media_item(clip.source) else {
                    continue;
                };
                let first = from - clip_start + to_frames(clip.source_in);
                let offset = usize::try_from(from - start).unwrap_or(0) * 2;
                let length = usize::try_from(to - from).unwrap_or(0) * 2;
                let clip_mix = &mut self.clip_mix[..length];
                clip_mix.fill(0.0);
                if let Err(error) = decoder.mix_into(media, first, clip_mix) {
                    if !self.warned.contains(&media.id) {
                        self.warned.push(media.id);
                        warnings.push(error);
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
        Levels {
            pts: Ticks(start).rescale(self.rate, sequence.timebase),
            tracks: track_levels,
            master,
        }
    }
}
