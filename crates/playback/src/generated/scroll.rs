use std::sync::Arc;

use media::VideoFrame;
use timeline::{Clip, Generator, MediaItem, Sequence, Ticks, Title, TitleMotion};

use super::coverage::Coverage;
use super::{draw_title, Anchor, Canvas};

/// The whole of a rolling or crawling title, on a strip as wide as the canvas for a roll
/// or as tall for a crawl, with room around the text for its stroke, shadow and box.
pub(super) fn title_strip(title: &Title, width: u32, height: u32) -> Canvas {
    let Some(extent) = Coverage::title_extent(title, width) else {
        return Canvas::new(width, height);
    };
    let margin = 2 * (title.size + title.stroke.map_or(0, |stroke| stroke.width));
    let mut strip = if title.motion.is_crawl() {
        Canvas::new(extent.x.ceil() as u32 + margin, height)
    } else {
        Canvas::new(width, extent.y.ceil() as u32 + margin)
    };
    draw_title(&mut strip, title, Anchor::Center);
    strip
}

/// What of a scrolling title's `strip` shows on the canvas at source `time` of `clip`;
/// other pictures pass through.
pub(crate) fn scrolled(
    picture: Arc<VideoFrame>,
    sequence: &Sequence,
    clip: &Clip,
    media: &MediaItem,
    time: Ticks,
) -> Arc<VideoFrame> {
    let Some(Generator::Title(title)) = &media.generator else {
        return picture;
    };
    if title.motion == TitleMotion::Still {
        return picture;
    }
    let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
    let frame = rate.frame_at(clip.position_of(time) - clip.start, timebase);
    let frames = rate.frame_at(clip.duration, timebase);
    let canvas = [sequence.format.width, sequence.format.height];
    let along = |strip: u32, view: u32| {
        let position = title
            .motion
            .position(strip as f32, view as f32, frame, frames);
        position.round() as i64
    };
    let offset = if title.motion.is_crawl() {
        [along(picture.width, canvas[0]), 0]
    } else {
        [0, along(picture.height, canvas[1])]
    };
    Arc::new(window(&picture, canvas, offset))
}

/// A `canvas` sized picture of `strip` with its top left at `offset`, transparent
/// where the strip doesn't reach.
fn window(strip: &VideoFrame, [width, height]: [u32; 2], [left, top]: [i64; 2]) -> VideoFrame {
    let mut pixels = vec![0; width as usize * height as usize * 4];
    let columns = left.max(0)..(left + i64::from(strip.width)).min(i64::from(width));
    let rows = top.max(0)..(top + i64::from(strip.height)).min(i64::from(height));
    if !columns.is_empty() {
        let span = (columns.end - columns.start) as usize * 4;
        for y in rows {
            let to = (y as usize * width as usize + columns.start as usize) * 4;
            let from_row = (y - top) as usize * strip.width as usize;
            let from = (from_row + (columns.start - left) as usize) * 4;
            pixels[to..to + span].copy_from_slice(&strip.pixels[from..from + span]);
        }
    }
    VideoFrame {
        pts: Ticks::ZERO,
        width,
        height,
        pixels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbered(width: u32, height: u32) -> VideoFrame {
        let pixels = (0..width * height)
            .flat_map(|index| [index as u8, 0, 0, 255])
            .collect();
        VideoFrame {
            pts: Ticks::ZERO,
            width,
            height,
            pixels,
        }
    }

    #[test]
    fn a_window_shows_the_strip_where_it_overlaps_the_canvas() {
        // A 2 by 3 strip whose top left lands one pixel right of and two above the
        // canvas's leaves its bottom row in the canvas's top right.
        let strip = numbered(2, 3);
        let shown = window(&strip, [3, 2], [1, -2]);
        let reds: Vec<u8> = shown.pixels.chunks(4).map(|pixel| pixel[0]).collect();
        let alphas: Vec<u8> = shown.pixels.chunks(4).map(|pixel| pixel[3]).collect();
        assert_eq!(reds, [0, 4, 5, 0, 0, 0]);
        assert_eq!(alphas, [0, 255, 255, 0, 0, 0]);
    }
}
