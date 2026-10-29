use std::sync::Arc;

use media::VideoFrame;
use timeline::{Clip, Param, Ticks, Timebase, VideoEffect, WriteOn, WriteOnStyle};

use crate::lightning::to_segment;

/// The most points one stroke is drawn through, however fine its spacing.
const MAX_POINTS: i64 = 1024;

/// `frame` with the clip's Write-on strokes painted in, in list order.
pub(crate) fn written_on(
    frame: Arc<VideoFrame>,
    clip: &Clip,
    timebase: Timebase,
    time: Ticks,
) -> Arc<VideoFrame> {
    let mut strokes = clip
        .look
        .effects
        .iter()
        .filter_map(|effect| match effect {
            VideoEffect::WriteOn(write_on) => Some(write_on),
            _ => None,
        })
        .peekable();
    if strokes.peek().is_none() {
        return frame;
    }
    let mut written = (*frame).clone();
    let ticks_per_second = f64::from(timebase.den()) / f64::from(timebase.num());
    for write_on in strokes {
        let seconds = |param: &Param| {
            Ticks((f64::from(param.value_at(time)) * ticks_per_second).round() as i64)
        };
        let length = seconds(&write_on.stroke_length);
        let start = if length.0 > 0 {
            Ticks((time.0 - length.0).max(clip.source_in.0))
        } else {
            clip.source_in
        };
        let spacing = seconds(&write_on.brush_spacing).0;
        write(&mut written, write_on, (start, time), spacing, time);
    }
    Arc::new(written)
}

/// Paints the brush's path from `span.0` to `span.1`, a point every `spacing` ticks,
/// with the brush as it is at `time`.
fn write(
    frame: &mut VideoFrame,
    write_on: &WriteOn,
    (start, end): (Ticks, Ticks),
    spacing: i64,
    time: Ticks,
) {
    if end < start {
        return;
    }
    let size = [frame.width as f32, frame.height as f32];
    let [x, y] = &write_on.brush_position;
    let point = |at: Ticks| {
        [
            x.value_at(at) / 100.0 * size[0],
            y.value_at(at) / 100.0 * size[1],
        ]
    };
    let step = spacing.max((end.0 - start.0) / MAX_POINTS).max(1);
    let mut times: Vec<Ticks> = (start.0..end.0).step_by(step as usize).map(Ticks).collect();
    times.push(end);
    let path: Vec<[f32; 2]> = times.into_iter().map(point).collect();

    let value = |param: &Param| param.value_at(time);
    let radius = (value(&write_on.brush_size) / 2.0).max(0.5);
    let solid = radius * (value(&write_on.brush_hardness) / 100.0).clamp(0.0, 1.0);
    let (width, height) = (frame.width as usize, frame.height as usize);
    let mut coverage = vec![0.0f32; width * height];
    let first = path.first().copied().into_iter();
    for (from, to) in first.chain(path.iter().copied()).zip(path.iter().copied()) {
        let reach = radius + 1.0;
        let span = |axis: usize, limit: usize| {
            let low = (from[axis].min(to[axis]) - reach).floor().max(0.0) as usize;
            let high = ((from[axis].max(to[axis]) + reach).ceil().max(0.0) as usize).min(limit);
            low..high
        };
        for row in span(1, height) {
            for column in span(0, width) {
                let center = [column as f32 + 0.5, row as f32 + 0.5];
                let away = to_segment(center, from, to);
                let cover = ((radius - away) / (radius - solid).max(0.5)).clamp(0.0, 1.0);
                let pixel = &mut coverage[row * width + column];
                *pixel = pixel.max(cover);
            }
        }
    }

    let opacity = (value(&write_on.brush_opacity) / 100.0).clamp(0.0, 1.0);
    let color = [write_on.color.r, write_on.color.g, write_on.color.b].map(f32::from);
    for (pixel, cover) in frame.pixels.as_chunks_mut::<4>().0.iter_mut().zip(coverage) {
        let paint = cover * opacity;
        match write_on.paint_style {
            WriteOnStyle::RevealOriginalImage => {
                pixel[3] = (f32::from(pixel[3]) * paint).round() as u8;
            }
            WriteOnStyle::OnTransparent => {
                let [r, g, b] = color.map(|channel| channel.round() as u8);
                *pixel = [r, g, b, (paint * 255.0).round() as u8];
            }
            WriteOnStyle::OnOriginalImage if paint > 0.0 => {
                let under = f32::from(pixel[3]) / 255.0 * (1.0 - paint);
                let alpha = paint + under;
                for (channel, painted) in pixel.iter_mut().zip(color) {
                    let mixed = (painted * paint + f32::from(*channel) * under) / alpha;
                    *channel = mixed.round() as u8;
                }
                pixel[3] = (alpha * 255.0).round() as u8;
            }
            WriteOnStyle::OnOriginalImage => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use timeline::Rgb;

    #[test]
    fn the_stroke_follows_the_brush_up_to_now() {
        let mut brush_x = Param::default();
        brush_x.toggle_animation(Ticks(0));
        brush_x.set_at(Ticks(0), 10.0);
        brush_x.set_at(Ticks(100), 90.0);
        let write_on = WriteOn {
            brush_position: [brush_x, Param::from_value(50.0)],
            color: Rgb::new(255, 0, 0),
            brush_size: Param::from_value(2.0),
            brush_hardness: Param::from_value(100.0),
            paint_style: WriteOnStyle::OnTransparent,
            ..WriteOn::default()
        };
        let mut frame = VideoFrame {
            pts: Ticks(0),
            width: 20,
            height: 3,
            pixels: vec![0; 20 * 3 * 4],
        };
        // Halfway, the brush has come from 2 to 10 pixels across.
        write(&mut frame, &write_on, (Ticks(0), Ticks(50)), 1, Ticks(50));
        let alpha = |x: usize, y: usize| frame.pixels[(y * 20 + x) * 4 + 3];
        assert_eq!(alpha(2, 1), 255);
        assert_eq!(alpha(9, 1), 255);
        assert_eq!(alpha(14, 1), 0);
        assert_eq!(alpha(5, 0), 0);
    }
}
