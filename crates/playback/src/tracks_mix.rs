use audio::SampleRate;
use timeline::{
    audio_fade, audio_span, clip_balance, clip_level, track_automation, AudioEffect, ClipId, Gain,
    MediaItem, SendTap, Sequence, SourceId, Submix, Ticks,
};
use timeline::{Timebase, Track};

use crate::audio_effects::EffectChain;
use crate::clip_audio::{ClipAudio, ClipRead, FrameSource};
use crate::clip_decoders::ClipDecoders;
use crate::mixdown;
use crate::track_audio::TrackAudio;
use crate::PlaybackError;

/// Mixes a set of audio tracks, the sequence's own or a nest's, with clip gain, track
/// automation, volume and balance and a master gain, into interleaved stereo at one
/// output rate.
pub(crate) struct TracksMix {
    sample_rate: SampleRate,
    /// One tick per output frame.
    rate: Timebase,
    tracks: Vec<ClipDecoders<Voice>>,
    /// Each track's inserts, in step with `tracks`.
    inserts: Vec<EffectChain>,
    /// What the tracks sent to each submix add up to, and its inserts.
    submix_mix: Vec<Vec<f32>>,
    submix_inserts: Vec<EffectChain>,
    master_inserts: EffectChain,
    track_mix: Vec<f32>,
    clip_mix: Vec<f32>,
    /// The time-remapped clip being mixed's speed keyframes, in frames.
    curve: Vec<[i64; 2]>,
    /// Media already reported as unreadable.
    warned: Vec<SourceId>,
}

/// One clip's decoding and the state of its effects.
struct Voice {
    audio: ClipAudio<MediaAudio>,
    effects: EffectChain,
}

impl Voice {
    fn reset(&mut self) {
        self.audio.reset();
        self.effects.reset();
    }
}

/// What a mix adds up: tracks, the submixes they may be sent to, and the master gain.
pub(crate) struct Bus<'a> {
    pub(crate) tracks: &'a [Track],
    pub(crate) submixes: &'a [Submix],
    pub(crate) master_effects: &'a [AudioEffect],
    pub(crate) master: Gain,
}

impl<'a> Bus<'a> {
    pub(crate) fn of(sequence: &'a Sequence) -> Self {
        Self {
            tracks: &sequence.audio,
            submixes: &sequence.submixes,
            master_effects: &sequence.master_effects,
            master: sequence.master,
        }
    }
}

/// Peak levels after the faders.
pub(crate) struct MixLevels {
    pub(crate) tracks: Vec<[f32; 2]>,
    pub(crate) submixes: Vec<[f32; 2]>,
    pub(crate) master: [f32; 2],
}

impl TracksMix {
    pub(crate) fn new(sample_rate: SampleRate) -> Self {
        Self {
            sample_rate,
            rate: Timebase::new(1, sample_rate.0).unwrap_or(Timebase::HZ_90K),
            tracks: Vec::new(),
            inserts: Vec::new(),
            submix_mix: Vec::new(),
            submix_inserts: Vec::new(),
            master_inserts: EffectChain::new(sample_rate),
            track_mix: Vec::new(),
            clip_mix: Vec::new(),
            curve: Vec::new(),
            warned: Vec::new(),
        }
    }

    pub(crate) fn rate(&self) -> Timebase {
        self.rate
    }

    /// Forgets decode positions, so the next mix seeks.
    pub(crate) fn reset(&mut self) {
        self.tracks
            .iter_mut()
            .flat_map(ClipDecoders::iter_mut)
            .for_each(Voice::reset);
        self.inserts
            .iter_mut()
            .chain(&mut self.submix_inserts)
            .chain(std::iter::once(&mut self.master_inserts))
            .for_each(EffectChain::reset);
    }

    /// Writes the mix of `bus` from output frame `start` over `out`. Clips find their
    /// media in `pool`; media that can't be read is reported once and left silent.
    pub(crate) fn mix(
        &mut self,
        pool: &Sequence,
        bus: Bus<'_>,
        start: i64,
        out: &mut [f32],
        warnings: &mut Vec<PlaybackError>,
    ) -> MixLevels {
        let Bus {
            tracks,
            submixes,
            master_effects,
            master,
        } = bus;
        let timebase = pool.timebase;
        let to_frames = |ticks: Ticks| ticks.rescale(timebase, self.rate).0;
        let end = start + (out.len() / 2) as i64;
        let sample_rate = self.sample_rate;
        self.tracks.resize_with(tracks.len(), || {
            ClipDecoders::new(|| Voice {
                audio: ClipAudio::new(MediaAudio::new(timebase, sample_rate)),
                effects: EffectChain::new(sample_rate),
            })
        });
        self.inserts
            .resize_with(tracks.len(), || EffectChain::new(sample_rate));
        self.submix_inserts
            .resize_with(submixes.len(), || EffectChain::new(sample_rate));
        self.submix_mix.resize_with(submixes.len(), Vec::new);
        for mix in &mut self.submix_mix {
            mix.clear();
            mix.resize(out.len(), 0.0);
        }
        self.track_mix.resize(out.len(), 0.0);
        self.clip_mix.resize(out.len(), 0.0);
        out.fill(0.0);
        let soloing = tracks.iter().any(|track| track.solo);
        let mut track_levels = Vec::with_capacity(tracks.len());
        let chains = self.tracks.iter_mut().zip(&mut self.inserts);
        for (track, (decoders, inserts)) in tracks.iter().zip(chains) {
            self.track_mix.fill(0.0);
            if track.muted || (soloing && !track.solo) {
                track_levels.push([0.0; 2]);
                continue;
            }
            let heard: Vec<(usize, i64, i64)> = (0..track.clips.len())
                .filter(|&index| !track.clips[index].disabled)
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
                let Some(media) = pool.media_item(clip.source) else {
                    continue;
                };
                let offset = usize::try_from(from - start).unwrap_or(0) * 2;
                let length = usize::try_from(to - from).unwrap_or(0) * 2;
                let clip_mix = &mut self.clip_mix[..length];
                clip_mix.fill(0.0);
                let (voice, changed) = decoders.for_clip_noting_change(clip.id, &keep);
                if changed {
                    voice.audio.forget_window();
                    voice.effects.reset();
                }
                self.curve.clear();
                if let Some(remap) = &clip.remap {
                    let clip_start = to_frames(clip.start);
                    self.curve.extend(remap.anchors().iter().map(|anchor| {
                        [
                            to_frames(clip.start + anchor.offset) - clip_start,
                            to_frames(anchor.source),
                        ]
                    }));
                }
                let read = ClipRead {
                    source_in: to_frames(clip.source_in),
                    length: to_frames(clip.end()) - to_frames(clip.start),
                    speed: clip.rate(),
                    reverse: clip.reverse,
                    maintain_pitch: clip.maintain_pitch,
                    curve: &self.curve,
                };
                let into_clip = from - to_frames(clip.start);
                if let Err(error) = voice
                    .audio
                    .mix_into(pool, media, &read, into_clip, clip_mix)
                {
                    if !self.warned.contains(&media.id) {
                        self.warned.push(media.id);
                        warnings.push(error);
                    }
                }
                if !clip.audio_effects.is_empty() {
                    let rate = self.rate;
                    let at = |frame| clip.source_time(Ticks(frame).rescale(rate, timebase));
                    voice
                        .effects
                        .process(Some(clip.id), &clip.audio_effects, from, at, clip_mix);
                }
                let (pairs, _) = clip_mix.as_chunks_mut::<2>();
                apply_gain(pairs, from, |frame| {
                    let position = Ticks(frame).rescale(self.rate, timebase);
                    let level = audio_fade(track, index, position) * clip_level(clip, position);
                    clip_balance(clip, position).map(|side| side * level)
                });
                mixdown::add_scaled(
                    clip_mix,
                    [1.0, 1.0],
                    &mut self.track_mix[offset..offset + length],
                );
            }
            if !track.effects.is_empty() {
                let rate = self.rate;
                let at = |frame| Ticks(frame).rescale(rate, timebase);
                inserts.process(None, &track.effects, start, at, &mut self.track_mix);
            }
            let sends = (track, submixes);
            send(
                sends,
                SendTap::PreFader,
                1.0,
                &self.track_mix,
                &mut self.submix_mix,
            );
            if track.automation.is_animated() || track.automation.value() != 0.0 {
                let (pairs, _) = self.track_mix.as_chunks_mut::<2>();
                apply_gain(pairs, start, |frame| {
                    [track_automation(track, Ticks(frame).rescale(self.rate, timebase)); 2]
                });
            }
            let volume = track.volume.amplitude();
            send(
                sends,
                SendTap::PostFader,
                volume,
                &self.track_mix,
                &mut self.submix_mix,
            );
            let [left, right] = track.pan.balance();
            let sent = track
                .output
                .and_then(|id| submixes.iter().position(|submix| submix.id == id));
            let target = match sent {
                Some(index) => &mut self.submix_mix[index][..],
                None => &mut *out,
            };
            track_levels.push(mixdown::add_scaled(
                &self.track_mix,
                [left * volume, right * volume],
                target,
            ));
        }
        let rate = self.rate;
        let at = |frame| Ticks(frame).rescale(rate, timebase);
        let buses = self.submix_mix.iter_mut().zip(&mut self.submix_inserts);
        let submix_levels = submixes
            .iter()
            .zip(buses)
            .map(|(submix, (mix, inserts))| {
                if submix.muted {
                    return [0.0; 2];
                }
                if !submix.effects.is_empty() {
                    inserts.process(None, &submix.effects, start, at, mix);
                }
                let volume = submix.volume.amplitude();
                let [left, right] = submix.pan.balance();
                mixdown::add_scaled(mix, [left * volume, right * volume], out)
            })
            .collect();
        if !master_effects.is_empty() {
            self.master_inserts
                .process(None, master_effects, start, at, out);
        }
        MixLevels {
            tracks: track_levels,
            submixes: submix_levels,
            master: mixdown::scale_in_place(out, master.amplitude()),
        }
    }
}

/// Frames between the points a gain is worked out at; ramped between them, as the
/// keyframes and fades behind a gain rarely bend over so short a time.
const GAIN_STEP: i64 = 64;

/// Scales each stereo frame of `pairs`, the first being frame `first`, by
/// `gain_at`. The points sit on a grid of whole frames, so a mix comes out the same
/// however it is split into calls.
fn apply_gain(pairs: &mut [[f32; 2]], first: i64, gain_at: impl Fn(i64) -> [f32; 2]) {
    let mut grid = first.div_euclid(GAIN_STEP) * GAIN_STEP;
    let mut from = gain_at(grid);
    let mut frame = first;
    let mut rest = pairs;
    while !rest.is_empty() {
        let to = gain_at(grid + GAIN_STEP);
        let count = usize::try_from(grid + GAIN_STEP - frame)
            .unwrap_or(0)
            .min(rest.len());
        let (block, after) = rest.split_at_mut(count);
        for (offset, pair) in (frame - grid..).zip(block) {
            let along = offset as f32 / GAIN_STEP as f32;
            for ((sample, from), to) in pair.iter_mut().zip(from).zip(to) {
                *sample *= from + (to - from) * along;
            }
        }
        rest = after;
        frame += count as i64;
        grid += GAIN_STEP;
        from = to;
    }
}

/// Adds `track_mix` to the submixes `track` sends to from `tap`, through the sends'
/// levels and pans and `fader`.
fn send(
    (track, submixes): (&Track, &[Submix]),
    tap: SendTap,
    fader: f32,
    track_mix: &[f32],
    submix_mix: &mut [Vec<f32>],
) {
    let heard = track
        .sends
        .iter()
        .filter(|send| !send.muted && send.tap == tap);
    for send in heard {
        let Some(index) = submixes.iter().position(|submix| submix.id == send.to) else {
            continue;
        };
        let level = fader * send.level.amplitude();
        let [left, right] = send.pan.balance();
        mixdown::add_scaled(
            track_mix,
            [left * level, right * level],
            &mut submix_mix[index],
        );
    }
}

/// A file's audio, or the mix of a nested sequence's tracks.
pub(crate) struct MediaAudio {
    file: TrackAudio,
    sample_rate: SampleRate,
    nest: Option<Box<NestAudio>>,
}

struct NestAudio {
    mix: TracksMix,
    out: Vec<f32>,
    warnings: Vec<PlaybackError>,
}

impl MediaAudio {
    fn new(timebase: Timebase, sample_rate: SampleRate) -> Self {
        Self {
            file: TrackAudio::new(timebase, sample_rate),
            sample_rate,
            nest: None,
        }
    }
}

impl FrameSource for MediaAudio {
    fn read(
        &mut self,
        pool: &Sequence,
        media: &MediaItem,
        first: i64,
        out: &mut [f32],
    ) -> Result<(), PlaybackError> {
        let Some(nest) = &media.nest else {
            return self.file.read(pool, media, first, out);
        };
        let sample_rate = self.sample_rate;
        let nested = self.nest.get_or_insert_with(|| {
            Box::new(NestAudio {
                mix: TracksMix::new(sample_rate),
                out: Vec::new(),
                warnings: Vec::new(),
            })
        });
        nested.out.resize(out.len(), 0.0);
        let bus = Bus {
            tracks: &nest.audio,
            submixes: &nest.submixes,
            master_effects: &nest.master_effects,
            master: nest.master,
        };
        let warnings = &mut nested.warnings;
        nested.mix.mix(pool, bus, first, &mut nested.out, warnings);
        mixdown::add_scaled(&nested.out, [1.0, 1.0], out);
        // The clip reports a nest's first unreadable media, as it would a file's.
        match nested.warnings.drain(..).next() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn reset(&mut self) {
        FrameSource::reset(&mut self.file);
        if let Some(nested) = &mut self.nest {
            nested.mix.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ramped_gain_is_exact_at_its_points_and_the_same_however_it_is_split() {
        let gain_at = |frame: i64| [(frame as f32 / 1000.0).sin().abs(), 0.5];
        let ones = |count| vec![[1.0f32; 2]; count];
        let mut whole = ones(300);
        apply_gain(&mut whole, -70, gain_at);
        let mut pieces = ones(300);
        let (head, tail) = pieces.split_at_mut(101);
        apply_gain(head, -70, gain_at);
        apply_gain(tail, 31, gain_at);
        assert_eq!(whole, pieces);
        assert_eq!(whole[70 + 128], gain_at(128));
        assert_eq!(whole[70 - 64], gain_at(-64));
        let between = whole[70 + 32][0];
        assert!((between - (gain_at(0)[0] + gain_at(64)[0]) / 2.0).abs() < 1e-6);
    }
}
