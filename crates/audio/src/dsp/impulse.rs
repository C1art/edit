use std::f32::consts::TAU;

use crate::SampleRate;

/// Decibels a reverb's tail falls by over its decay time, by the RT60 convention.
const DECAY_DB: f32 = 60.0;
const LONGEST_SECONDS: f32 = 4.0;
/// Damping at full narrows the tail's brightness from about this to a dull thud.
const BRIGHTEST_HZ: f32 = 18_000.0;
/// How many times over its decay the fullest damping halves the tail's brightness, in
/// powers of e.
const DAMPING_FALL: f32 = 4.0;
const EARLY_REFLECTIONS: usize = 8;

/// The shape of a space's response to a click, for [`synthesize_impulse`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImpulseShape {
    /// Seconds for the tail to fall by sixty decibels.
    pub decay_seconds: f32,
    /// Milliseconds over which the first distinct echoes off near walls arrive; none
    /// leaves a plate's instant density.
    pub early_ms: f32,
    /// 0 to 1: how fast the tail loses its high end.
    pub damping: f32,
    pub pre_delay_ms: f32,
}

/// A stereo impulse response for `shape`, from noise that decays and darkens as a
/// room's diffuse tail does, with the sides seeded apart so the space sounds wide.
/// Each side carries unit energy, so the reverb keeps a sound's loudness.
pub fn synthesize_impulse(shape: &ImpulseShape, rate: SampleRate) -> [Vec<f32>; 2] {
    let rate_hz = rate.0 as f32;
    let seconds = |ms: f32| ms.max(0.0) / 1000.0;
    let decay = shape.decay_seconds.clamp(0.05, LONGEST_SECONDS);
    let pre_delay = (seconds(shape.pre_delay_ms) * rate_hz) as usize;
    let tail = (decay * rate_hz) as usize;
    let damping = shape.damping.clamp(0.0, 1.0);
    [0x9E37_79B9_u32, 0x85EB_CA6B].map(|seed| {
        let mut noise = Noise(seed);
        let mut response = vec![0.0; pre_delay + tail];
        let mut darkened = 0.0;
        for (index, sample) in response[pre_delay..].iter_mut().enumerate() {
            let time = index as f32 / rate_hz;
            let envelope = 10.0_f32.powf(-DECAY_DB / 20.0 * time / decay);
            let cutoff = BRIGHTEST_HZ * (-DAMPING_FALL * damping * time / decay).exp();
            let follow = 1.0 - (-TAU * cutoff / rate_hz).exp();
            darkened += (noise.next() - darkened) * follow;
            *sample = darkened * envelope;
        }
        let early = seconds(shape.early_ms) * rate_hz;
        for reflection in 0..EARLY_REFLECTIONS {
            if early < 1.0 {
                break;
            }
            let offset = ((noise.next() * 0.5 + 0.5) * early) as usize;
            let strength = 1.0 - reflection as f32 / EARLY_REFLECTIONS as f32;
            let sign = noise.next().signum();
            if let Some(sample) = response.get_mut(pre_delay + offset) {
                *sample += sign * strength * 4.0;
            }
        }
        let energy = response.iter().map(|sample| sample * sample).sum::<f32>();
        let scale = energy.sqrt().recip();
        if scale.is_finite() {
            response.iter_mut().for_each(|sample| *sample *= scale);
        }
        response
    })
}

/// Repeatable white noise from -1 to 1, so a space sounds the same every time it's
/// built.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        let Self(state) = self;
        *state ^= *state << 13;
        *state ^= *state >> 17;
        *state ^= *state << 5;
        *state as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}
