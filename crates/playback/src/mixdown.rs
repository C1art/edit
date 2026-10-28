/// Adds interleaved stereo `from` into `into`, scaling each side, and returns the peak
/// of what was added per side.
pub(crate) fn add_scaled(from: &[f32], scale: [f32; 2], into: &mut [f32]) -> [f32; 2] {
    let mut peaks = [0.0_f32; 2];
    for (source, target) in from
        .as_chunks::<2>()
        .0
        .iter()
        .zip(into.as_chunks_mut::<2>().0)
    {
        for side in 0..2 {
            let sample = source[side] * scale[side];
            target[side] += sample;
            peaks[side] = peaks[side].max(sample.abs());
        }
    }
    peaks
}

/// Scales interleaved stereo in place and returns the peak per side.
pub(crate) fn scale_in_place(samples: &mut [f32], scale: f32) -> [f32; 2] {
    let mut peaks = [0.0_f32; 2];
    for frame in samples.as_chunks_mut::<2>().0 {
        for side in 0..2 {
            frame[side] *= scale;
            peaks[side] = peaks[side].max(frame[side].abs());
        }
    }
    peaks
}
