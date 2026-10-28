/// Maps interleaved stereo onto a device layout: front left/right first, any further
/// channels silent, and mono as the average of both sides.
pub fn remix_stereo(stereo: &[f32], channels: u16, output: &mut Vec<f32>) {
    let channels = usize::from(channels);
    output.reserve(stereo.len() / 2 * channels);
    for frame in stereo.as_chunks::<2>().0 {
        match channels {
            0 => {}
            1 => output.push((frame[0] + frame[1]) * 0.5),
            _ => {
                output.extend_from_slice(frame);
                output.extend(std::iter::repeat_n(0.0, channels - 2));
            }
        }
    }
}
