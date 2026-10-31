use super::fft::Fft;

/// Samples taken in before each pass: about 5 ms of delay on the wet sound at 48 kHz.
const BLOCK: usize = 256;
/// Each pass transforms a block and the one before it, so the circular wrap-around of
/// the product lands only in the half thrown away.
const SPAN: usize = BLOCK * 2;

/// A spectrum of [`SPAN`] bins.
#[derive(Debug, Clone)]
struct Spectrum {
    re: Vec<f32>,
    im: Vec<f32>,
}

impl Spectrum {
    fn silent() -> Self {
        Self {
            re: vec![0.0; SPAN],
            im: vec![0.0; SPAN],
        }
    }
}

/// Runs the middle of stereo sound through a stereo impulse response, one response per
/// side, by uniformly partitioned overlap-save convolution. The result lags the input
/// by [`BLOCK`] samples.
#[derive(Debug, Clone)]
pub struct Convolver {
    fft: Fft,
    /// Each side's response cut into block-long pieces, as spectra.
    pieces: [Vec<Spectrum>; 2],
    /// The latest transformed inputs, newest at `newest`, reused as a ring.
    history: Vec<Spectrum>,
    newest: usize,
    /// The previous block of input and the one filling.
    input: [f32; SPAN],
    filled: usize,
    /// The wet frames handed out while the next block fills.
    ready: [[f32; 2]; BLOCK],
    scratch: Spectrum,
}

impl Convolver {
    pub fn new(response: &[Vec<f32>; 2]) -> Self {
        let fft = Fft::new(SPAN);
        let count = response
            .iter()
            .map(|side| side.len().div_ceil(BLOCK))
            .max()
            .unwrap_or(0)
            .max(1);
        let pieces = response.each_ref().map(|side| {
            (0..count)
                .map(|piece| {
                    let mut spectrum = Spectrum::silent();
                    let start = (piece * BLOCK).min(side.len());
                    let taps = &side[start..(start + BLOCK).min(side.len())];
                    spectrum.re[..taps.len()].copy_from_slice(taps);
                    fft.run(&mut spectrum.re, &mut spectrum.im, false);
                    spectrum
                })
                .collect()
        });
        Self {
            fft,
            pieces,
            history: vec![Spectrum::silent(); count],
            newest: 0,
            input: [0.0; SPAN],
            filled: 0,
            ready: [[0.0; 2]; BLOCK],
            scratch: Spectrum::silent(),
        }
    }

    pub fn reset(&mut self) {
        for spectrum in &mut self.history {
            spectrum.re.fill(0.0);
            spectrum.im.fill(0.0);
        }
        self.input = [0.0; SPAN];
        self.filled = 0;
        self.ready = [[0.0; 2]; BLOCK];
    }

    /// Replaces each interleaved frame with the response to the frames before it.
    pub fn process(&mut self, samples: &mut [f32]) {
        for frame in samples.as_chunks_mut::<2>().0 {
            self.input[BLOCK + self.filled] = (frame[0] + frame[1]) / 2.0;
            *frame = self.ready[self.filled];
            self.filled += 1;
            if self.filled == BLOCK {
                self.pass();
                self.filled = 0;
            }
        }
    }

    fn pass(&mut self) {
        self.newest = (self.newest + 1) % self.history.len();
        let latest = &mut self.history[self.newest];
        latest.re.copy_from_slice(&self.input);
        latest.im.fill(0.0);
        self.fft.run(&mut latest.re, &mut latest.im, false);
        self.input.copy_within(BLOCK.., 0);
        // Both sides' outputs are real, so one inverse carries the left in its real part
        // and the right in its imaginary part.
        let Spectrum { re, im } = &mut self.scratch;
        re.fill(0.0);
        im.fill(0.0);
        let count = self.history.len();
        for age in 0..count {
            let input = &self.history[(self.newest + count - age) % count];
            let [left, right] = [&self.pieces[0][age], &self.pieces[1][age]];
            for bin in 0..SPAN {
                let (x_re, x_im) = (input.re[bin], input.im[bin]);
                let left_re = x_re * left.re[bin] - x_im * left.im[bin];
                let left_im = x_re * left.im[bin] + x_im * left.re[bin];
                let right_re = x_re * right.re[bin] - x_im * right.im[bin];
                let right_im = x_re * right.im[bin] + x_im * right.re[bin];
                re[bin] += left_re - right_im;
                im[bin] += left_im + right_re;
            }
        }
        self.fft.run(re, im, true);
        let scale = (SPAN as f32).recip();
        for (frame, (left, right)) in self
            .ready
            .iter_mut()
            .zip(re[BLOCK..].iter().zip(&im[BLOCK..]))
        {
            *frame = [left * scale, right * scale];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_is_the_direct_convolution_a_block_late() {
        // Long enough to span several pieces, and different per side.
        let response = [
            (0..700)
                .map(|tap| ((tap * 37 % 101) as f32 - 50.0) / 50.0)
                .collect::<Vec<_>>(),
            (0..500)
                .map(|tap| ((tap * 53 % 89) as f32 - 44.0) / 44.0)
                .collect(),
        ];
        let middle: Vec<f32> = (0..2_000)
            .map(|frame| ((frame * 29 % 61) as f32 - 30.0) / 30.0)
            .collect();
        let mut samples: Vec<f32> = middle.iter().flat_map(|&sample| [sample; 2]).collect();
        let mut convolver = Convolver::new(&response);
        convolver.process(&mut samples[..1_234]);
        convolver.process(&mut samples[1_234..]);
        for frame in [BLOCK, BLOCK + 1, 900, 1_999] {
            for (side, taps) in response.iter().enumerate() {
                let time = frame - BLOCK;
                let direct: f32 = taps
                    .iter()
                    .enumerate()
                    .filter(|&(lag, _)| lag <= time)
                    .map(|(lag, tap)| tap * middle[time - lag])
                    .sum();
                assert!(
                    (samples[frame * 2 + side] - direct).abs() < 1e-3,
                    "{frame} {side}"
                );
            }
        }
        assert!(samples[..BLOCK * 2].iter().all(|&sample| sample == 0.0));
    }
}
