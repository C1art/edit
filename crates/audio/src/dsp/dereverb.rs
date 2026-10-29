use super::db_to_amplitude;
use super::stft::{Spectrum, Stft, BINS, HOP};
use crate::SampleRate;

/// Reflections arriving this long after the sound are taken for the room's.
const LATE_SECONDS: f32 = 0.05;
/// How fast a typical room's sound dies away: 60 dB over this long.
const DECAY_SECONDS: f32 = 0.5;
/// Smooths each bin's level over a few windows, as a room's sound builds and fades.
const LEVEL_SMOOTHING: f32 = 0.6;
/// At full amount, the estimated reverberation is subtracted this many times over.
const OVERSUBTRACTION: f32 = 2.0;
const GAIN_SMOOTHING: f32 = 0.5;
/// The deepest cut, at full amount.
const MAX_REDUCTION_DB: f32 = 25.0;

/// Takes a room's reverberation out of the sound: each frequency's late reflections are
/// estimated from its level a moment ago, faded as a room fades it, and turned down.
/// Adds a window of delay.
#[derive(Debug, Clone)]
pub struct DeReverb {
    stft: Stft,
    channels: [Channel; 2],
    /// How much a level fades over the late delay, in power.
    late_fade: f32,
    /// Windows back the late reflections are read from, and where in each channel's ring
    /// of past levels the next window goes.
    late_windows: usize,
    next: usize,
}

#[derive(Debug, Clone)]
struct Channel {
    level: Vec<f32>,
    /// The last `late_windows` windows' levels, bin by bin.
    past: Vec<f32>,
    gain: Vec<f32>,
}

impl DeReverb {
    pub fn new(rate: SampleRate) -> Self {
        let windows_per_second = rate.0 as f32 / HOP as f32;
        let late_windows = (LATE_SECONDS * windows_per_second).round().max(1.0) as usize;
        let late_seconds = late_windows as f32 / windows_per_second;
        let channel = || Channel {
            level: vec![0.0; BINS],
            past: vec![0.0; BINS * late_windows],
            gain: vec![1.0; BINS],
        };
        Self {
            stft: Stft::new(),
            channels: [channel(), channel()],
            late_fade: 10.0_f32.powf(-6.0 * late_seconds / DECAY_SECONDS),
            late_windows,
            next: 0,
        }
    }

    /// Forgets the sound in flight, as after a seek.
    pub fn reset(&mut self) {
        self.stft.reset();
        for channel in &mut self.channels {
            channel.level.fill(0.0);
            channel.past.fill(0.0);
            channel.gain.fill(1.0);
        }
    }

    /// Takes reverberation out of interleaved stereo `samples` by `amount`, from 0 to 1,
    /// then lifts them by `gain_db`.
    pub fn process(&mut self, amount: f32, gain_db: f32, samples: &mut [f32]) {
        let amount = amount.clamp(0.0, 1.0);
        let Self {
            stft,
            channels,
            late_fade,
            late_windows,
            next,
        } = self;
        stft.process(samples, |spectra| {
            let slot = *next * BINS;
            for (channel, spectrum) in channels.iter_mut().zip(spectra) {
                channel.shape(spectrum, amount, *late_fade, slot);
            }
            *next = (*next + 1) % *late_windows;
        });
        let gain = db_to_amplitude(gain_db);
        samples.iter_mut().for_each(|sample| *sample *= gain);
    }
}

impl Channel {
    /// `slot` is where in the ring the oldest level is, which this window's replaces.
    fn shape(&mut self, spectrum: &mut Spectrum, amount: f32, late_fade: f32, slot: usize) {
        let floor = 10.0_f32.powf(-MAX_REDUCTION_DB * amount / 20.0);
        let past = &mut self.past[slot..slot + BINS];
        for (bin, ((level, earlier), gain)) in self
            .level
            .iter_mut()
            .zip(past)
            .zip(&mut self.gain)
            .enumerate()
        {
            *level = LEVEL_SMOOTHING * *level + (1.0 - LEVEL_SMOOTHING) * spectrum.power(bin);
            let late = *earlier * late_fade;
            *earlier = *level;
            let wanted = if *level > 0.0 {
                (1.0 - OVERSUBTRACTION * amount * late / *level).max(floor)
            } else {
                1.0
            };
            *gain = GAIN_SMOOTHING * *gain + (1.0 - GAIN_SMOOTHING) * wanted;
            spectrum.scale(bin, *gain);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::stft::WINDOW;
    use super::*;

    fn energy(samples: &[f32]) -> f32 {
        samples.iter().map(|sample| sample * sample).sum()
    }

    #[test]
    fn a_tail_dying_as_a_room_does_is_cut_but_the_sound_before_it_is_kept() {
        let rate = 48_000;
        let mut seed = 7_u32;
        let mut noise = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / (1 << 24) as f32 - 0.5
        };
        // Silence, a 30 ms burst, then its tail fading 60 dB over half a second.
        let (onset, burst) = (rate / 2, rate * 3 / 100);
        let input: Vec<f32> = (0..2 * rate)
            .map(|n| match n {
                n if n < onset => 0.0,
                n if n < onset + burst => noise(),
                n => {
                    let after = (n - onset - burst) as f32 / rate as f32;
                    0.5 * noise() * 10.0_f32.powf(-3.0 * after / DECAY_SECONDS)
                }
            })
            .collect();
        let mut samples: Vec<f32> = input.iter().flat_map(|&value| [value, value]).collect();
        DeReverb::new(SampleRate(rate as u32)).process(1.0, 0.0, &mut samples);
        let left: Vec<f32> = samples.iter().step_by(2).copied().collect();
        let span = |from: usize, to: usize| (onset + from, onset + to);
        let compare = |(from, to): (usize, usize)| {
            energy(&left[from + WINDOW..to + WINDOW]) / energy(&input[from..to])
        };
        let direct = compare(span(0, burst));
        let tail = compare(span(burst + rate / 10, burst + rate * 4 / 10));
        assert!(direct > 0.7, "the burst kept {direct} of its energy");
        assert!(tail < 0.1, "the tail kept {tail} of its energy");
    }
}
