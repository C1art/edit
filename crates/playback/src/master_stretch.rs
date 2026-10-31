//! Playing the mixed sound faster or slower at its own pitch, by overlap-adding
//! windowed grains read from the mix at the playing speed (WSOLA), each nudged to carry
//! on the waveform of the grain before.

use std::f32::consts::TAU;

/// About 21 ms at 48 kHz: long enough to hold a voice's pitch, short enough not to echo.
const GRAIN: usize = 1024;
/// Grains overlap by half, where Hann windows sum to one.
const HOP: usize = GRAIN / 2;
/// How far a grain may move from where the speed puts it to line up with the last one.
const SEEK: usize = 256;
/// The coarse search tries every `COARSE`th nudge before refining around the best.
const COARSE: usize = 8;

pub(crate) struct MasterStretch {
    /// Mix frames read per output frame, as a percentage.
    percent: u32,
    /// Interleaved stereo mix frames not yet passed by every grain to come.
    input: Vec<f32>,
    /// The mix frame `input` starts at.
    origin: usize,
    /// The next grain to place.
    grain: usize,
    /// Where the last grain started reading, in mix frames.
    previous: Option<usize>,
    /// Output being overlap-added; its first `HOP` frames are complete once a grain lands.
    pending: Vec<f32>,
    window: Vec<f32>,
    template: Vec<f32>,
}

impl MasterStretch {
    pub(crate) fn new(percent: u32) -> Self {
        let window = (0..GRAIN)
            .map(|i| 0.5 - 0.5 * (TAU * i as f32 / GRAIN as f32).cos())
            .collect();
        Self {
            percent: percent.max(1),
            input: Vec::new(),
            origin: 0,
            grain: 0,
            previous: None,
            pending: vec![0.0; GRAIN * 2],
            window,
            template: Vec::with_capacity(HOP),
        }
    }

    /// Takes interleaved stereo mix frames and appends the stretched frames they complete.
    pub(crate) fn push(&mut self, mix: &[f32], out: &mut Vec<f32>) {
        self.input.extend_from_slice(mix);
        loop {
            let natural = self.natural(self.grain);
            if natural + SEEK + GRAIN > self.origin + self.input.len() / 2 {
                return;
            }
            let start = match self.previous {
                Some(previous) => self.best_start(previous + HOP, natural),
                None => natural,
            };
            let read = &self.input[(start - self.origin) * 2..][..GRAIN * 2];
            for ((pending, sample), weight) in self
                .pending
                .as_chunks_mut::<2>()
                .0
                .iter_mut()
                .zip(read.as_chunks::<2>().0)
                .zip(&self.window)
            {
                pending[0] += weight * sample[0];
                pending[1] += weight * sample[1];
            }
            out.extend_from_slice(&self.pending[..HOP * 2]);
            self.pending.copy_within(HOP * 2.., 0);
            self.pending[GRAIN..].fill(0.0);
            self.previous = Some(start);
            self.grain += 1;
            let keep_from = (start + HOP).min(self.natural(self.grain).saturating_sub(SEEK));
            let drop = keep_from.saturating_sub(self.origin);
            self.input.drain(..(drop * 2).min(self.input.len()));
            self.origin += drop;
        }
    }

    fn natural(&self, grain: usize) -> usize {
        grain * HOP * self.percent as usize / 100
    }

    /// The start near `natural` whose sound best continues from `continuing`, where the
    /// last grain would have gone on.
    fn best_start(&mut self, continuing: usize, natural: usize) -> usize {
        let mono = |input: &[f32], frame: usize| {
            let at = (frame - self.origin) * 2;
            input[at] + input[at + 1]
        };
        self.template.clear();
        self.template
            .extend((0..HOP).map(|i| mono(&self.input, continuing + i)));
        let lowest = natural.saturating_sub(SEEK).max(self.origin);
        let similarity = |start: usize, step: usize| {
            let (mut product, mut energy) = (0.0, 1e-9);
            for i in (0..HOP).step_by(step) {
                let value = mono(&self.input, start + i);
                product += self.template[i] * value;
                energy += value * value;
            }
            product / f32::sqrt(energy)
        };
        let best = |candidates: std::ops::RangeInclusive<usize>, step: usize, by: usize| {
            candidates
                .step_by(step)
                .map(|start| (start, similarity(start, by)))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map_or(natural, |(start, _)| start)
        };
        let highest = natural + SEEK;
        let coarse = best(lowest..=highest, COARSE, COARSE);
        best(
            coarse.saturating_sub(COARSE - 1).max(lowest)..=(coarse + COARSE - 1).min(highest),
            1,
            1,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stretched_length(percent: u32, frames: usize) -> usize {
        let tone: Vec<f32> = (0..frames)
            .flat_map(|i| {
                let value = (TAU * 220.0 * i as f32 / 48_000.0).sin();
                [value, value]
            })
            .collect();
        let mut stretch = MasterStretch::new(percent);
        let mut out = Vec::new();
        for chunk in tone.chunks(1024 * 2) {
            stretch.push(chunk, &mut out);
        }
        out.len() / 2
    }

    #[test]
    fn output_length_follows_the_speed() {
        let frames = 480_000;
        for percent in [25, 50, 150, 200] {
            let expected = frames * 100 / percent as usize;
            let made = stretched_length(percent, frames);
            // The last grains wait for mix beyond the end.
            let slack = (GRAIN + SEEK) * 100 / percent as usize + HOP;
            assert!(
                made + slack >= expected && made <= expected,
                "{percent}%: {made} of {expected}"
            );
        }
    }
}
