use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler as _};

use crate::{AudioError, SampleRate};

const CHUNK_FRAMES: usize = 1024;

/// Converts an interleaved stream between fixed sample rates, outside the audio callback.
/// The FFT resampler's startup delay is trimmed, so output frame `n` lines up with input
/// time `n / to`, and `flush` pads the tail so that the total output length is exactly
/// the input length scaled by the rate ratio.
pub struct Resampler {
    from: SampleRate,
    to: SampleRate,
    channels: usize,
    fft: Option<Fft<f32>>,
    pending: Vec<f32>,
    scratch: Vec<f32>,
    delay_left: usize,
    frames_in: u64,
    frames_out: u64,
}

impl Resampler {
    pub fn new(from: SampleRate, to: SampleRate, channels: u16) -> Result<Self, AudioError> {
        let channels = usize::from(channels);
        if channels == 0 || from.0 == 0 || to.0 == 0 {
            return Err(AudioError::InvalidFormat);
        }
        let fft = if from == to {
            None
        } else {
            Some(
                Fft::new(
                    from.0 as usize,
                    to.0 as usize,
                    CHUNK_FRAMES,
                    channels,
                    FixedSync::Input,
                )
                .map_err(AudioError::ResamplerSetup)?,
            )
        };
        let scratch_len = fft
            .as_ref()
            .map_or(0, |fft| fft.output_frames_max() * channels);
        let delay_left = fft.as_ref().map_or(0, |fft| fft.output_delay());
        Ok(Self {
            from,
            to,
            channels,
            fft,
            pending: Vec::new(),
            scratch: vec![0.0; scratch_len],
            delay_left,
            frames_in: 0,
            frames_out: 0,
        })
    }

    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) -> Result<(), AudioError> {
        let input = &input[..input.len() / self.channels * self.channels];
        self.frames_in += (input.len() / self.channels) as u64;
        let Some(fft) = &mut self.fft else {
            output.extend_from_slice(input);
            return Ok(());
        };
        self.pending.extend_from_slice(input);
        loop {
            let needed = fft.input_frames_next();
            if self.pending.len() / self.channels < needed {
                return Ok(());
            }
            let consumed = run_chunk(
                fft,
                &self.pending,
                &mut self.scratch,
                self.channels,
                None,
                &mut Emitter {
                    delay_left: &mut self.delay_left,
                    frames_out: &mut self.frames_out,
                    output,
                },
            )?;
            self.pending.drain(..consumed * self.channels);
        }
    }

    /// Emits the remaining buffered audio at end of stream, then resets for reuse.
    pub fn flush(&mut self, output: &mut Vec<f32>) -> Result<(), AudioError> {
        let expected = self.expected_output_frames();
        if let Some(fft) = &mut self.fft {
            let start_len = output.len();
            let mut emitter = Emitter {
                delay_left: &mut self.delay_left,
                frames_out: &mut self.frames_out,
                output,
            };
            while *emitter.frames_out < expected {
                let partial = self.pending.len() / self.channels;
                let indexing = Indexing::new().partial_len(partial);
                let consumed = run_chunk(
                    fft,
                    &self.pending,
                    &mut self.scratch,
                    self.channels,
                    Some(&indexing),
                    &mut emitter,
                )?;
                self.pending.drain(..consumed.min(partial) * self.channels);
            }
            let excess = (self.frames_out - expected) as usize * self.channels;
            output.truncate(output.len().max(start_len + excess) - excess);
        }
        self.reset();
        Ok(())
    }

    /// Drops buffered audio, as needed when the source seeks.
    pub fn reset(&mut self) {
        if let Some(fft) = &mut self.fft {
            fft.reset();
            self.delay_left = fft.output_delay();
        }
        self.pending.clear();
        self.frames_in = 0;
        self.frames_out = 0;
    }

    fn expected_output_frames(&self) -> u64 {
        let (from, to) = (u128::from(self.from.0), u128::from(self.to.0));
        let frames = (u128::from(self.frames_in) * to + from / 2) / from;
        u64::try_from(frames).unwrap_or(u64::MAX)
    }
}

struct Emitter<'a> {
    delay_left: &'a mut usize,
    frames_out: &'a mut u64,
    output: &'a mut Vec<f32>,
}

/// Resamples one chunk from the front of `pending` and returns the input frames consumed.
fn run_chunk(
    fft: &mut Fft<f32>,
    pending: &[f32],
    scratch: &mut [f32],
    channels: usize,
    indexing: Option<&Indexing>,
    emitter: &mut Emitter<'_>,
) -> Result<usize, AudioError> {
    let input_frames = pending.len() / channels;
    let output_frames = scratch.len() / channels;
    let input = InterleavedSlice::new(pending, channels, input_frames)
        .map_err(|_| AudioError::InvalidFormat)?;
    let mut out = InterleavedSlice::new_mut(scratch, channels, output_frames)
        .map_err(|_| AudioError::InvalidFormat)?;
    let (consumed, produced) = fft
        .process_into_buffer(&input, &mut out, indexing)
        .map_err(AudioError::Resample)?;
    let skip = produced.min(*emitter.delay_left);
    *emitter.delay_left -= skip;
    emitter
        .output
        .extend_from_slice(&scratch[skip * channels..produced * channels]);
    *emitter.frames_out += (produced - skip) as u64;
    Ok(consumed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resample_in_chunks(from: u32, to: u32, input: &[f32], chunk: usize) -> Vec<f32> {
        let mut resampler = Resampler::new(SampleRate(from), SampleRate(to), 2).unwrap();
        let mut output = Vec::new();
        for piece in input.chunks(chunk * 2) {
            resampler.process(piece, &mut output).unwrap();
        }
        resampler.flush(&mut output).unwrap();
        output
    }

    #[test]
    fn output_length_matches_rate_ratio_for_odd_chunk_sizes() {
        let input = vec![0.0f32; 44_100 * 2];
        let output = resample_in_chunks(44_100, 48_000, &input, 997);
        assert_eq!(output.len(), 48_000 * 2);
    }

    #[test]
    fn delay_is_trimmed_so_an_impulse_keeps_its_time_position() {
        let mut input = vec![0.0f32; 4800 * 2];
        let impulse_frame = 2400;
        input[impulse_frame * 2] = 1.0;
        let output = resample_in_chunks(48_000, 44_100, &input, 480);
        let peak = output
            .chunks(2)
            .enumerate()
            .max_by(|a, b| a.1[0].total_cmp(&b.1[0]))
            .map(|(frame, _)| frame)
            .unwrap();
        let expected = impulse_frame * 44_100 / 48_000;
        assert!(
            peak.abs_diff(expected) <= 1,
            "peak at {peak}, expected {expected}"
        );
    }

    #[test]
    fn matching_rates_pass_through_unchanged() {
        let input: Vec<f32> = (0..2000).map(|i| i as f32).collect();
        assert_eq!(resample_in_chunks(48_000, 48_000, &input, 333), input);
    }
}
