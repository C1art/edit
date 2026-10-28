use audio::SampleRate;
use timeline::{audio_fade, audio_span, ClipId, Sequence, SourceId, Ticks, Timebase};

use crate::clip_audio::ClipAudio;
use crate::clip_decoders::ClipDecoders;
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
    tracks: Vec<ClipDecoders<ClipAudio>>,
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
        self.tracks
            .iter_mut()
            .flat_map(ClipDecoders::iter_mut)
            .for_each(ClipAudio::reset);
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
            ClipDecoders::new(|| ClipAudio::new(TrackAudio::new(timebase, sample_rate)))
        });
        self.mix.fill(0.0);
        let mut track_levels = Vec::with_capacity(sequence.audio.len());
        for (track, decoders) in sequence.audio.iter().zip(&mut self.tracks) {
            self.track_mix.fill(0.0);
            if track.muted {
                track_levels.push([0.0; 2]);
                continue;
            }
            let heard: Vec<(usize, i64, i64)> = (0..track.clips.len())
                .filter_map(|index| {
                    let (span_start, span_end) = audio_span(track, index);
                    let (from, to) = (
                        start.max(to_frames(span_start)),
                        end.min(to_frames(span_end)),
                    );
                    (from < to).then_some((index, from, to))
                })
                .collect();
            let keep: Vec<ClipId> = heard
                .iter()
                .map(|&(index, ..)| track.clips[index].id)
                .collect();
            for (index, from, to) in heard {
                let clip = &track.clips[index];
                let Some(media) = sequence.media_item(clip.source) else {
                    continue;
                };
                let offset = usize::try_from(from - start).unwrap_or(0) * 2;
                let length = usize::try_from(to - from).unwrap_or(0) * 2;
                let clip_mix = &mut self.clip_mix[..length];
                clip_mix.fill(0.0);
                let decoder = decoders.for_clip(clip.id, &keep);
                let (source_in, into_clip) =
                    (to_frames(clip.source_in), from - to_frames(clip.start));
                if let Err(error) =
                    decoder.mix_into(media, source_in, into_clip, clip.speed, clip_mix)
                {
                    if !self.warned.contains(&media.id) {
                        self.warned.push(media.id);
                        warnings.push(error);
                    }
                }
                let (pairs, _) = clip_mix.as_chunks_mut::<2>();
                for (frame, pair) in (from..to).zip(pairs) {
                    let fade = audio_fade(track, index, Ticks(frame).rescale(self.rate, timebase));
                    pair.iter_mut().for_each(|sample| *sample *= fade);
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
