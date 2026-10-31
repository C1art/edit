use std::ops::Range;

use super::fft::Fft;
use crate::SampleRate;

/// Samples between the frames music is described by: about 43 ms at 48 kHz.
pub const RETIME_HOP: usize = 2048;
const WINDOW: usize = RETIME_HOP * 2;
const BANDS: usize = 16;
const LOWEST_HZ: f32 = 60.0;
const HIGHEST_HZ: f32 = 12_000.0;
/// Frames either side of a seam that must sound alike for it to pass unheard, about
/// 0.7 s.
const CONTEXT: usize = 16;
/// The intro and the ending play as they are.
const KEEP_SECONDS: f32 = 2.0;
/// How far from the wanted length a seam may land, for a better-sounding one.
const SLACK_SECONDS: f32 = 3.0;
/// What a frame's distance from the wanted length weighs against how alike a seam's
/// sides sound, per [`SLACK_SECONDS`].
const LENGTH_WEIGHT: f32 = 0.5;

/// Describes mono music frame by frame as it is pushed, to find where it can jump
/// forward or back without the jump being heard.
#[derive(Debug, Clone)]
pub struct MusicFeatures {
    rate: SampleRate,
    fft: Fft,
    window: Vec<f32>,
    /// Bin edges of the bands, log-spaced.
    edges: [usize; BANDS + 1],
    pending: Vec<f32>,
    pushed: u64,
    /// Log energy of each band, per frame.
    frames: Vec<[f32; BANDS]>,
}

impl MusicFeatures {
    pub fn new(rate: SampleRate) -> Self {
        let hz_per_bin = rate.0 as f32 / WINDOW as f32;
        let edges = std::array::from_fn(|band| {
            let hz = LOWEST_HZ * (HIGHEST_HZ / LOWEST_HZ).powf(band as f32 / BANDS as f32);
            ((hz / hz_per_bin).round() as usize).clamp(1, WINDOW / 2)
        });
        let window = (0..WINDOW)
            .map(|index| {
                let phase = std::f32::consts::TAU * index as f32 / WINDOW as f32;
                0.5 - 0.5 * phase.cos()
            })
            .collect();
        Self {
            rate,
            fft: Fft::new(WINDOW),
            window,
            edges,
            pending: Vec::with_capacity(WINDOW * 2),
            pushed: 0,
            frames: Vec::new(),
        }
    }

    pub fn push(&mut self, mono: &[f32]) {
        self.pushed += mono.len() as u64;
        self.pending.extend_from_slice(mono);
        let mut re = vec![0.0; WINDOW];
        let mut im = vec![0.0; WINDOW];
        while self.pending.len() >= WINDOW {
            for ((re, im), (sample, weight)) in re
                .iter_mut()
                .zip(&mut im)
                .zip(self.pending.iter().zip(&self.window))
            {
                *re = sample * weight;
                *im = 0.0;
            }
            self.fft.run(&mut re, &mut im, false);
            let power = |bin: usize| re[bin] * re[bin] + im[bin] * im[bin];
            let bands = std::array::from_fn(|band| {
                let bins = self.edges[band]..self.edges[band + 1].max(self.edges[band] + 1);
                let energy: f32 = bins.clone().map(power).sum::<f32>() / bins.len() as f32;
                (energy + 1e-9).ln()
            });
            self.frames.push(bands);
            self.pending.drain(..RETIME_HOP);
        }
    }

    /// The pieces of the music, as sample ranges played one after another, that come
    /// closest to `target` samples long while keeping its start and its end. The
    /// whole music when it is already within a frame of the target, or too short to
    /// cut.
    #[expect(
        clippy::single_range_in_vec_init,
        reason = "a list of one piece is meant"
    )]
    pub fn plan(&self, target: u64) -> Vec<Range<u64>> {
        let total = self.pushed;
        let hop = RETIME_HOP as u64;
        let frames = |seconds: f32| (seconds * self.rate.0 as f32 / RETIME_HOP as f32) as usize;
        let keep = frames(KEEP_SECONDS).max(CONTEXT);
        let usable = keep..self.frames.len().saturating_sub(keep);
        let widest = usable.end.saturating_sub(usable.start);
        let change = target.abs_diff(total) / hop;
        if change == 0 || widest < 2 {
            return vec![0..total];
        }
        let change = change as usize;
        let slack = frames(SLACK_SECONDS).max(1);
        let at = |frame: usize| frame as u64 * hop;
        if target < total {
            let lags = change.saturating_sub(slack).max(1)..(change + slack).min(widest);
            match self.best_seam(usable, lags, change, slack) {
                Some((from, to)) => vec![0..at(from), at(to)..total],
                None => vec![0..total],
            }
        } else {
            // Longer than the music allows in one jump back, the same loop repeats.
            let loops = change.div_ceil(widest - 1);
            let each = change / loops;
            let lags = each.saturating_sub(slack).max(1)..(each + slack).min(widest);
            match self.best_seam(usable, lags, each, slack) {
                Some((back_to, from)) => {
                    let mut pieces = vec![0..at(from)];
                    pieces.extend(std::iter::repeat_n(at(back_to)..at(from), loops - 1));
                    pieces.push(at(back_to)..total);
                    pieces
                }
                None => vec![0..total],
            }
        }
    }

    /// The frames `a < b`, both within `usable` and `lags` apart, around which the music
    /// sounds most alike, preferring those `wanted` apart.
    fn best_seam(
        &self,
        usable: Range<usize>,
        lags: Range<usize>,
        wanted: usize,
        slack: usize,
    ) -> Option<(usize, usize)> {
        let mut best: Option<(f32, usize, usize)> = None;
        for lag in lags {
            let penalty = LENGTH_WEIGHT * lag.abs_diff(wanted) as f32 / slack as f32;
            for a in usable.start..usable.end.saturating_sub(lag) {
                let score = self.unlikeness(a, a + lag) + penalty;
                if best.is_none_or(|(lowest, ..)| score < lowest) {
                    best = Some((score, a, a + lag));
                }
            }
        }
        best.map(|(_, a, b)| (a, b))
    }

    /// The mean difference in band levels around frames `a` and `b`.
    fn unlikeness(&self, a: usize, b: usize) -> f32 {
        let offsets = (0..CONTEXT * 2).step_by(2);
        let count = offsets.len() * BANDS;
        let sum: f32 = offsets
            .map(|offset| {
                let (left, right) = (
                    &self.frames[a + offset - CONTEXT],
                    &self.frames[b + offset - CONTEXT],
                );
                left.iter()
                    .zip(right)
                    .map(|(x, y)| (x - y).abs())
                    .sum::<f32>()
            })
            .sum();
        sum / count as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: SampleRate = SampleRate(48_000);
    /// About four seconds, a whole number of frames.
    const SECTION: u64 = 94 * RETIME_HOP as u64;

    /// Four-second tones: A B A B C, so a jump of eight seconds within the first four
    /// sections is seamless.
    fn song() -> MusicFeatures {
        let mut features = MusicFeatures::new(RATE);
        for hz in [440.0, 660.0, 440.0, 660.0, 990.0] {
            let tone: Vec<f32> = (0..SECTION)
                .map(|index| (std::f32::consts::TAU * hz * index as f32 / RATE.0 as f32).sin())
                .collect();
            features.push(&tone);
        }
        features
    }

    fn length(pieces: &[Range<u64>]) -> u64 {
        pieces.iter().map(|piece| piece.end - piece.start).sum()
    }

    #[test]
    fn music_is_cut_or_looped_where_the_seam_matches() {
        let features = song();
        let hop = RETIME_HOP as u64;
        let section_pair = 2 * SECTION;

        let shorter = features.plan(3 * SECTION);
        assert_eq!(shorter.len(), 2);
        assert!((shorter[1].start - shorter[0].end).abs_diff(section_pair) <= hop);
        assert_eq!(shorter[1].end, 5 * SECTION);

        let longer = features.plan(7 * SECTION);
        assert_eq!(longer.len(), 2);
        assert!((longer[0].end - longer[1].start).abs_diff(section_pair) <= hop);
        assert!(length(&longer).abs_diff(7 * SECTION) <= hop);

        assert_eq!(features.plan(5 * SECTION + 10), vec![0..5 * SECTION]);
    }
}
