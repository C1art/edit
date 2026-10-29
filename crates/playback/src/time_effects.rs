use std::sync::Arc;

use media::VideoFrame;
use timeline::{
    Clip, ClipNameSource, FrameRate, MediaItem, Param, Sequence, SimpleText, Ticks, Timebase,
    Timecode, TimecodeSource, VideoEffect,
};

use crate::generated::{text_box, timecode_box};

/// The source time a clip's picture is taken from, held on the frame grid of its last
/// Posterize Time, if it has one.
pub(crate) fn posterized(effects: &[VideoEffect], time: Ticks, timebase: Timebase) -> Ticks {
    let rate = effects.iter().rev().find_map(|effect| match effect {
        VideoEffect::PosterizeTime { frame_rate } => Some(frame_rate.value_at(time)),
        _ => None,
    });
    let Some(rate) = rate.filter(|rate| *rate > 0.0) else {
        return time;
    };
    let second = f64::from(timebase.den()) / f64::from(timebase.num());
    let step = (second / f64::from(rate)).round().max(1.0) as i64;
    Ticks(time.0.div_euclid(step) * step)
}

/// `frame` with the text of the clip's Timecode, Clip Name and Simple Text effects burnt
/// in, in list order.
pub(crate) fn with_burn_ins(
    frame: Arc<VideoFrame>,
    sequence: &Sequence,
    clip: &Clip,
    media: &MediaItem,
    time: Ticks,
) -> Arc<VideoFrame> {
    let burn_ins: Vec<_> = clip
        .look
        .effects
        .iter()
        .filter_map(|effect| match effect {
            VideoEffect::Timecode {
                position,
                size,
                opacity,
                source,
            } => {
                let text = match source {
                    TimecodeSource::Media => {
                        let rate = media
                            .video
                            .map_or(sequence.format.frame_rate, |video| video.frame_rate);
                        timecode_text(time, sequence.timebase, rate)
                    }
                    TimecodeSource::Clip => timecode_text(
                        time - clip.source_in,
                        sequence.timebase,
                        sequence.format.frame_rate,
                    ),
                };
                Some((text, position, size, opacity, None))
            }
            VideoEffect::ClipName {
                position,
                size,
                opacity,
                source,
            } => {
                let text = match source {
                    ClipNameSource::ProjectItem => media.name.clone(),
                    ClipNameSource::SequenceClip => {
                        clip.name.clone().unwrap_or_else(|| media.name.clone())
                    }
                    ClipNameSource::File => media
                        .path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                };
                Some((text, position, size, opacity, None))
            }
            VideoEffect::SimpleText(SimpleText {
                text,
                position,
                alignment,
                size,
                opacity,
            }) => Some((text.clone(), position, size, opacity, Some(*alignment))),
            _ => None,
        })
        .collect();
    if burn_ins.is_empty() {
        return frame;
    }
    let mut stamped = (*frame).clone();
    for (text, position, size, opacity, align) in burn_ins {
        let percent = |param: &Param| param.value_at(time) / 100.0;
        let height = (percent(size) * frame.height as f32).round().max(4.0) as u32;
        let opacity = percent(opacity).clamp(0.0, 1.0);
        // Timecode and names keep the monospace box; typed text sets in the sans.
        let label = match align {
            Some(align) => text_box(&text, height, opacity, align),
            None => timecode_box(&text, height, opacity),
        };
        let center = [
            percent(&position[0]) * frame.width as f32,
            percent(&position[1]) * frame.height as f32,
        ];
        let left = (center[0] - label.width as f32 / 2.0).round() as i64;
        let top = (center[1] - label.height as f32 / 2.0).round() as i64;
        blend_over(&mut stamped, &label, left, top);
    }
    Arc::new(stamped)
}

fn timecode_text(time: Ticks, timebase: Timebase, rate: FrameRate) -> String {
    Timecode::new(time.max(Ticks::ZERO), timebase, rate).to_string()
}

/// Composites straight-alpha `top` over `below` with its corner at `left`, `top_edge`.
fn blend_over(below: &mut VideoFrame, top: &VideoFrame, left: i64, top_edge: i64) {
    let (width, height) = (i64::from(below.width), i64::from(below.height));
    for row in 0..i64::from(top.height) {
        let y = top_edge + row;
        if !(0..height).contains(&y) {
            continue;
        }
        for column in 0..i64::from(top.width) {
            let x = left + column;
            if !(0..width).contains(&x) {
                continue;
            }
            let from = ((row * i64::from(top.width) + column) * 4) as usize;
            let to = ((y * width + x) * 4) as usize;
            let source = &top.pixels[from..from + 4];
            let alpha = f32::from(source[3]) / 255.0;
            if alpha == 0.0 {
                continue;
            }
            let pixel = &mut below.pixels[to..to + 4];
            let under = f32::from(pixel[3]) / 255.0 * (1.0 - alpha);
            let out = alpha + under;
            for channel in 0..3 {
                let mixed =
                    (f32::from(source[channel]) * alpha + f32::from(pixel[channel]) * under) / out;
                pixel[channel] = mixed.round() as u8;
            }
            pixel[3] = (out * 255.0).round() as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posterize_time_holds_each_picture_for_its_frame() {
        let timebase = Timebase::HZ_90K;
        let effects = [VideoEffect::PosterizeTime {
            frame_rate: Param::from_value(10.0),
        }];
        // A tenth of a second is 9,000 ticks.
        assert_eq!(posterized(&effects, Ticks(8_999), timebase), Ticks(0));
        assert_eq!(posterized(&effects, Ticks(9_000), timebase), Ticks(9_000));
        assert_eq!(posterized(&effects, Ticks(26_000), timebase), Ticks(18_000));
        assert_eq!(posterized(&[], Ticks(26_000), timebase), Ticks(26_000));
    }
}
