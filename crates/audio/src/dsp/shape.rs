/// Soft-clips interleaved samples after `drive_db` of gain, scaled so full scale stays
/// full scale, and mixes the result in at `mix`.
pub fn distort(drive_db: f32, mix: f32, samples: &mut [f32]) {
    let drive = super::db_to_amplitude(drive_db.max(0.0));
    let full_scale = drive.tanh();
    let mix = mix.clamp(0.0, 1.0);
    for sample in samples {
        let shaped = (*sample * drive).tanh() / full_scale;
        *sample += (shaped - *sample) * mix;
    }
}

/// Scales the difference between the sides of interleaved stereo by `width`: none
/// makes mono, one leaves the sound as it is and more spreads it wider.
pub fn widen(width: f32, samples: &mut [f32]) {
    let width = width.max(0.0);
    for frame in samples.as_chunks_mut::<2>().0 {
        let [left, right] = *frame;
        let (middle, side) = ((left + right) / 2.0, (left - right) / 2.0 * width);
        *frame = [middle + side, middle - side];
    }
}
