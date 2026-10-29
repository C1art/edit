use std::f32::consts::PI;

use super::fft::Fft;

/// About 21 ms at 48 kHz: fine enough in frequency to part a voice from hiss, and the
/// delay it adds stays well under what lip sync shows.
pub(super) const WINDOW: usize = 1024;
/// Four overlapping windows cover each sample.
pub(super) const HOP: usize = WINDOW / 4;
pub(super) const BINS: usize = WINDOW / 2 + 1;
/// Square-root Hann windows in and out overlap at a quarter window to twice the input.
const OVERLAP_GAIN: f32 = 0.5;

/// Cuts stereo sound into overlapping windows, hands their spectra out to be shaped and
/// adds the shaped windows back up. Adds a window of delay.
#[derive(Debug, Clone)]
pub(super) struct Stft {
    fft: Fft,
    window: Vec<f32>,
    framing: [Framing; 2],
    spectra: [Spectrum; 2],
    /// Samples taken since the last window.
    filled: usize,
}

#[derive(Debug, Clone)]
struct Framing {
    /// The latest window of input, oldest first.
    input: Vec<f32>,
    /// Overlapping windows of output being added up; the front hop is complete.
    sum: Vec<f32>,
    /// The hop of output being handed out while the next fills.
    ready: Vec<f32>,
}

/// One channel's window as frequencies; only the lower `BINS` need shaping.
#[derive(Debug, Clone)]
pub(super) struct Spectrum {
    re: Vec<f32>,
    im: Vec<f32>,
}

impl Spectrum {
    pub(super) fn power(&self, bin: usize) -> f32 {
        self.re[bin].powi(2) + self.im[bin].powi(2)
    }

    pub(super) fn scale(&mut self, bin: usize, gain: f32) {
        self.re[bin] *= gain;
        self.im[bin] *= gain;
        // The upper half mirrors the lower, as the input is real.
        if bin > 0 && bin < WINDOW / 2 {
            self.re[WINDOW - bin] *= gain;
            self.im[WINDOW - bin] *= gain;
        }
    }
}

impl Stft {
    pub(super) fn new() -> Self {
        let framing = || Framing {
            input: vec![0.0; WINDOW],
            sum: vec![0.0; WINDOW],
            ready: vec![0.0; HOP],
        };
        let spectrum = || Spectrum {
            re: vec![0.0; WINDOW],
            im: vec![0.0; WINDOW],
        };
        Self {
            fft: Fft::new(WINDOW),
            window: (0..WINDOW)
                .map(|n| (PI * (n as f32 + 0.5) / WINDOW as f32).sin())
                .collect(),
            framing: [framing(), framing()],
            spectra: [spectrum(), spectrum()],
            filled: 0,
        }
    }

    /// Forgets the sound in flight, as after a seek.
    pub(super) fn reset(&mut self) {
        for framing in &mut self.framing {
            framing.input.fill(0.0);
            framing.sum.fill(0.0);
            framing.ready.fill(0.0);
        }
        self.filled = 0;
    }

    /// Runs interleaved stereo `samples` through, calling `shape` with both channels'
    /// spectra each time a hop of new input completes a window.
    pub(super) fn process(
        &mut self,
        samples: &mut [f32],
        mut shape: impl FnMut(&mut [Spectrum; 2]),
    ) {
        for frame in samples.as_chunks_mut::<2>().0 {
            for (sample, framing) in frame.iter_mut().zip(&mut self.framing) {
                framing.input[WINDOW - HOP + self.filled] = *sample;
                *sample = framing.ready[self.filled];
            }
            self.filled += 1;
            if self.filled < HOP {
                continue;
            }
            self.filled = 0;
            for (framing, spectrum) in self.framing.iter().zip(&mut self.spectra) {
                for ((re, im), (&input, &weight)) in spectrum
                    .re
                    .iter_mut()
                    .zip(&mut spectrum.im)
                    .zip(framing.input.iter().zip(&self.window))
                {
                    *re = input * weight;
                    *im = 0.0;
                }
                self.fft.run(&mut spectrum.re, &mut spectrum.im, false);
            }
            shape(&mut self.spectra);
            let scale = OVERLAP_GAIN / WINDOW as f32;
            for (framing, spectrum) in self.framing.iter_mut().zip(&mut self.spectra) {
                self.fft.run(&mut spectrum.re, &mut spectrum.im, true);
                for ((sum, &re), &weight) in
                    framing.sum.iter_mut().zip(&spectrum.re).zip(&self.window)
                {
                    *sum += re * weight * scale;
                }
                framing.ready.copy_from_slice(&framing.sum[..HOP]);
                framing.sum.copy_within(HOP.., 0);
                framing.sum[WINDOW - HOP..].fill(0.0);
                framing.input.copy_within(HOP.., 0);
            }
        }
    }
}
