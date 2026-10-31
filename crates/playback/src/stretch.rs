use std::collections::VecDeque;
use std::f32::consts::TAU;
use std::ops::{Range, RangeInclusive};

use crate::clip_audio::ClipRead;

/// Output frames per grain, about 32 ms at 48 kHz: long enough to hold a low voice's
/// period, short enough that transients don't smear audibly.
const GRAIN: i64 = 1536;
/// Grains overlap by half, where their Hann windows sum to one.
const HOP: i64 = GRAIN / 2;
/// How far a grain may move from where the speed puts it to line up with the one
/// before; about a period of 60 Hz.
const SEEK: i64 = 800;
/// Coarse search step, refined around the best coarse match.
const COARSE: i64 = 4;
const REMEMBERED: usize = 8;

/// Waveform-similarity overlap-add: changes speed without changing pitch by laying
/// short grains of the source end to end, each nudged to continue the last one's
/// waveform.
#[derive(Debug, Default)]
pub(crate) struct Wsola {
    /// Recent grains and how far each was nudged, so a grain heard across two chunks
    /// is placed the same in both.
    placed: VecDeque<(i64, i64)>,
    template: Vec<f32>,
}

impl Wsola {
    pub(crate) fn reset(&mut self) {
        self.placed.clear();
    }

    /// The source frames the grains heard in output `frames` of the clip may read.
    pub(crate) fn needed(read: &ClipRead, frames: Range<i64>) -> Range<i64> {
        let ends = [grains(&frames).start(), grains(&frames).end()]
            .map(|&grain| read.position(grain * HOP).0);
        let (low, high) = (ends[0].min(ends[1]), ends[0].max(ends[1]));
        (low - SEEK - GRAIN).max(0)..(high + SEEK + GRAIN + 1).max(0)
    }

    /// Adds output frames from `offset` on into `out`, reading the source through
    /// `sample(frame, channel)`, which must cover what [`Self::needed`] asked for.
    pub(crate) fn render(
        &mut self,
        read: &ClipRead,
        offset: i64,
        out: &mut [f32],
        sample: impl Fn(i64, usize) -> f32,
    ) {
        let frames = (out.len() / 2) as i64;
        let direction = read.direction();
        for grain in grains(&(offset..offset + frames)) {
            let start = self.grain_start(read, grain, &sample);
            let origin = grain * HOP;
            for frame in origin.max(offset)..(origin + GRAIN).min(offset + frames) {
                let into = frame - origin;
                let source = start + direction * into;
                if source < 0 {
                    continue;
                }
                let weight = 0.5 - 0.5 * (TAU * into as f32 / GRAIN as f32).cos();
                let at = usize::try_from(frame - offset).unwrap_or(0) * 2;
                for (channel, sample_out) in out[at..at + 2].iter_mut().enumerate() {
                    *sample_out += weight * sample(source, channel);
                }
            }
        }
    }

    /// The source frame grain `grain` starts reading at: where the speed puts it,
    /// nudged to best continue the waveform of the grain before.
    fn grain_start(
        &mut self,
        read: &ClipRead,
        grain: i64,
        sample: &impl Fn(i64, usize) -> f32,
    ) -> i64 {
        let natural = read.position(grain * HOP).0;
        let remembered = |wanted: i64| {
            self.placed
                .iter()
                .find(|(placed, _)| *placed == wanted)
                .map(|&(_, nudge)| nudge)
        };
        if let Some(nudge) = remembered(grain) {
            return natural + nudge;
        }
        let nudge = match remembered(grain - 1) {
            Some(previous_nudge) => {
                let direction = read.direction();
                let previous = read.position((grain - 1) * HOP).0 + previous_nudge;
                let mono = |frame: i64| sample(frame, 0) + sample(frame, 1);
                // What the previous grain would have played next, had it gone on.
                self.template.clear();
                self.template
                    .extend((0..HOP).map(|i| mono(previous + direction * (HOP + i))));
                let similarity = |nudge: i64, step: i64| {
                    let start = natural + nudge;
                    let (mut product, mut energy) = (0.0, 1e-9);
                    for i in (0..HOP).step_by(step as usize) {
                        let value = mono(start + direction * i);
                        product += self.template[i as usize] * value;
                        energy += value * value;
                    }
                    product / energy.sqrt()
                };
                let best = |candidates: RangeInclusive<i64>, step: i64, by: i64| {
                    candidates
                        .step_by(step as usize)
                        .map(|nudge| (nudge, similarity(nudge, by)))
                        .max_by(|a, b| a.1.total_cmp(&b.1))
                        .map_or(0, |(nudge, _)| nudge)
                };
                let coarse = best(-SEEK..=SEEK, COARSE, COARSE);
                best(coarse - COARSE + 1..=coarse + COARSE - 1, 1, 1)
            }
            // The first grain after a seek has nothing to line up with.
            None => 0,
        };
        if self.placed.len() == REMEMBERED {
            self.placed.pop_front();
        }
        self.placed.push_back((grain, nudge));
        natural + nudge
    }
}

/// The grains sounding anywhere in output `frames`.
fn grains(frames: &Range<i64>) -> RangeInclusive<i64> {
    (frames.start - GRAIN).div_euclid(HOP) + 1..=(frames.end - 1).div_euclid(HOP)
}

#[cfg(test)]
mod tests {
    use super::*;
    use timeline::Speed;

    #[test]
    fn stretching_keeps_a_tone_at_its_pitch() {
        let period = 96.0;
        let tone = |frame: i64| (TAU * frame as f32 / period).sin();
        let read = ClipRead {
            source_in: 0,
            length: 48_000,
            speed: Speed::new(1, 2).unwrap(),
            reverse: false,
            maintain_pitch: true,
            curve: &[],
        };
        let mut wsola = Wsola::default();
        let mut out = vec![0.0; 2 * 9600];
        for (index, chunk) in out.chunks_mut(2 * 1024).enumerate() {
            let offset = index as i64 * 1024;
            wsola.render(&read, offset, chunk, |frame, _| tone(frame));
        }
        // Past the first grain, count rising zero crossings: half speed must not halve
        // the frequency as plain resampling would.
        let left: Vec<f32> = out
            .iter()
            .step_by(2)
            .skip(GRAIN as usize)
            .copied()
            .collect();
        let crossings = left
            .windows(2)
            .filter(|pair| pair[0] < 0.0 && pair[1] >= 0.0)
            .count();
        let expected = left.len() as f32 / period;
        assert!(
            (crossings as f32 - expected).abs() <= 2.0,
            "{crossings} vs {expected}"
        );
    }
}
