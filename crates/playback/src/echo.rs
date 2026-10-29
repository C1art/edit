//! Echo: the decoded picture combined with pictures from other moments of its clip.

use std::sync::Arc;

use media::{MediaError, VideoFrame};
use timeline::{Echo, EchoOperator, MediaItem, Sequence, Ticks, VideoEffect};

use crate::track_video::TrackVideo;
use crate::Resolution;

/// Echoes further than this many sequence frames from the picture are left out, as
/// every frame between has to stay decoded.
const MAX_ECHO_SPAN: u64 = 30;

/// The clip's last Echo, which is the one that shows.
pub(crate) fn echo_of(effects: &[VideoEffect]) -> Option<&Echo> {
    effects.iter().rev().find_map(|effect| match effect {
        VideoEffect::Echo(echo) => Some(echo),
        _ => None,
    })
}

/// The pictures Echo combines, the clip's own first: how many sequence frames along the
/// clip each lies and how strongly it shows.
fn taps(echo: &Echo, time: Ticks) -> Vec<(i64, f32)> {
    let step = echo.echo_time.value_at(time).round() as i64;
    let echoes = echo.echoes.value_at(time).round().max(0.0) as i64;
    let start = echo.starting_intensity.value_at(time).clamp(0.0, 1.0);
    let decay = echo.decay.value_at(time).clamp(0.0, 1.0);
    (0..=echoes)
        .map(|index| (index * step, start * decay.powi(index as i32)))
        .take_while(|(frames, _)| frames.unsigned_abs() <= MAX_ECHO_SPAN)
        .collect()
}

/// The picture at `time` echoed as `echo` says. The pictures are decoded earliest first,
/// keeping those passed on the way, so that the next frame finds them all again
/// without seeking.
pub(crate) fn echoed_frame_at(
    decoder: &mut TrackVideo,
    sequence: &Sequence,
    media: &MediaItem,
    (time, echo): (Ticks, &Echo),
    canvas: [u32; 2],
    resolution: Resolution,
) -> Result<Option<Arc<VideoFrame>>, MediaError> {
    let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
    let taps = taps(echo, time);
    let times: Vec<Ticks> = taps
        .iter()
        .map(|&(frames, _)| (time + rate.frame_start(frames, timebase)).max(Ticks::ZERO))
        .collect();
    let span = match (times.iter().min(), times.iter().max()) {
        (Some(&earliest), Some(&latest)) => latest - earliest,
        _ => Ticks::ZERO,
    };
    let media_rate = media.video.map_or(rate, |video| video.frame_rate);
    let held = usize::try_from(media_rate.frame_at(span, timebase)).unwrap_or(0) + 2;
    decoder.keep_history(held);
    let mut order: Vec<usize> = (0..times.len()).collect();
    order.sort_by_key(|&index| times[index]);
    let mut pictures = vec![None; times.len()];
    for index in order {
        pictures[index] = decoder.frame_at(media, times[index], canvas, resolution)?;
    }
    let weighted: Vec<(&VideoFrame, f32)> = pictures
        .iter()
        .zip(&taps)
        .filter_map(|(picture, &(_, intensity))| Some((picture.as_deref()?, intensity)))
        .collect();
    Ok(combine(&weighted, echo.operator).map(Arc::new))
}

/// `pictures` combined by `operator`, each scaled by its intensity. Pictures of another
/// size than the first are left out.
fn combine(pictures: &[(&VideoFrame, f32)], operator: EchoOperator) -> Option<VideoFrame> {
    let (first, _) = pictures.first()?;
    let pictures: Vec<_> = pictures
        .iter()
        .filter(|(picture, _)| picture.pixels.len() == first.pixels.len())
        .collect();
    let count = pictures.len() as f32;
    let pixels = (0..first.pixels.len() / 4)
        .flat_map(|pixel| {
            let colors = pictures.iter().map(|(picture, intensity)| {
                let bytes = &picture.pixels[pixel * 4..pixel * 4 + 4];
                (
                    std::array::from_fn(|channel| f32::from(bytes[channel]) / 255.0),
                    *intensity,
                )
            });
            to_bytes(combine_pixel(colors, operator, count))
        })
        .collect();
    Some(VideoFrame { pixels, ..**first })
}

/// One pixel of the echoed picture from the same pixel of each picture, straight alpha
/// in and out.
fn combine_pixel(
    colors: impl DoubleEndedIterator<Item = ([f32; 4], f32)>,
    operator: EchoOperator,
    count: f32,
) -> [f32; 4] {
    let scaled = |(color, intensity): ([f32; 4], f32)| color.map(|value| value * intensity);
    match operator {
        EchoOperator::Add => colors.map(scaled).fold([0.0; 4], |sum, color| {
            std::array::from_fn(|channel| sum[channel] + color[channel])
        }),
        EchoOperator::Blend => colors
            .map(scaled)
            .fold([0.0; 4], |sum, color| {
                std::array::from_fn(|channel| sum[channel] + color[channel])
            })
            .map(|sum| sum / count),
        EchoOperator::Maximum => colors.map(scaled).fold([0.0; 4], |most, color| {
            std::array::from_fn(|channel| most[channel].max(color[channel]))
        }),
        EchoOperator::Minimum => colors.map(scaled).fold([1.0; 4], |least, color| {
            std::array::from_fn(|channel| least[channel].min(color[channel]))
        }),
        EchoOperator::Screen => colors
            .map(scaled)
            .fold([1.0; 4], |left, color| {
                std::array::from_fn(|channel| left[channel] * (1.0 - color[channel]))
            })
            .map(|left| 1.0 - left),
        // Drawn from the back: the clip's own picture last when it is in front.
        EchoOperator::CompositeInBack => composite(colors.rev()),
        EchoOperator::CompositeInFront => composite(colors),
    }
}

/// Each color drawn over those before it, by its alpha times its intensity.
fn composite(colors: impl Iterator<Item = ([f32; 4], f32)>) -> [f32; 4] {
    let [r, g, b, a] = colors.fold([0.0; 4], |below, (color, intensity)| {
        let alpha = color[3] * intensity;
        let over = |channel: usize| color[channel] * alpha + below[channel] * (1.0 - alpha);
        [over(0), over(1), over(2), alpha + below[3] * (1.0 - alpha)]
    });
    if a <= 0.0 {
        return [0.0; 4];
    }
    [r / a, g / a, b / a, a]
}

fn to_bytes(color: [f32; 4]) -> [u8; 4] {
    color.map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
}

#[cfg(test)]
mod tests {
    use timeline::Param;

    use super::*;

    fn frame(pixel: [u8; 4]) -> VideoFrame {
        VideoFrame {
            pts: Ticks::ZERO,
            width: 1,
            height: 1,
            pixels: pixel.to_vec(),
        }
    }

    #[test]
    fn echoes_fade_by_the_decay_and_stop_at_the_span() {
        let echo = Echo {
            echo_time: Param::from_value(-2.0),
            echoes: Param::from_value(3.0),
            starting_intensity: Param::from_value(0.8),
            decay: Param::from_value(0.5),
            operator: EchoOperator::Add,
        };
        assert_eq!(
            taps(&echo, Ticks::ZERO),
            [(0, 0.8), (-2, 0.4), (-4, 0.2), (-6, 0.1)]
        );
        let far = Echo {
            echo_time: Param::from_value(20.0),
            ..echo
        };
        assert_eq!(taps(&far, Ticks::ZERO), [(0, 0.8), (20, 0.4)]);
    }

    #[test]
    fn operators_combine_the_scaled_pictures() {
        let (dark, light) = (frame([60, 0, 200, 255]), frame([120, 252, 100, 254]));
        let pictures = [(&dark, 1.0), (&light, 0.5)];
        let combined = |operator| combine(&pictures, operator).unwrap().pixels;
        assert_eq!(combined(EchoOperator::Add), [120, 126, 250, 255]);
        assert_eq!(combined(EchoOperator::Maximum), [60, 126, 200, 255]);
        assert_eq!(combined(EchoOperator::Minimum), [60, 0, 50, 127]);
        assert_eq!(combined(EchoOperator::Blend), [60, 63, 125, 191]);
        assert_eq!(combined(EchoOperator::Screen), [106, 126, 211, 255]);
    }

    #[test]
    fn composites_put_the_clips_own_picture_in_front_or_behind() {
        let own = frame([255, 0, 0, 255]);
        let echo = frame([0, 0, 255, 128]);
        let pictures = [(&own, 1.0), (&echo, 1.0)];
        let back = combine(&pictures, EchoOperator::CompositeInBack).unwrap();
        assert_eq!(back.pixels, [255, 0, 0, 255]);
        let front = combine(&pictures, EchoOperator::CompositeInFront).unwrap();
        assert_eq!(front.pixels, [127, 0, 128, 255]);
    }
}
