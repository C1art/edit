use std::ops::{ControlFlow, Range};
use std::path::Path;

use timeline::{MaskTracking, Similarity, Ticks, Timebase};

use crate::block_match::{Gray, Pyramid};
use crate::fit::{robust_fit, MIN_OUTLIER_PIXELS};
use crate::{DecodedFrame, MediaError, MediaReader, Streams};

/// Wide enough to find features in, small enough to analyze quickly.
const ANALYSIS_WIDTH: u32 = 320;

/// How a picture moves from frame to frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Motion {
    /// The picture's size as analyzed, for its shape.
    pub size: [u32; 2],
    /// Each frame's time and where its points went from the frame before, in square
    /// units (see `Similarity`); the first frame's is the identity.
    pub steps: Vec<(Ticks, Similarity)>,
}

/// How the camera moved through `range` of `path`'s picture. `progress` hears the
/// fraction done and can stop the work. Blocks on decoding, so run it on a worker.
pub fn track_motion(
    path: &Path,
    timebase: Timebase,
    range: Range<Ticks>,
    mut progress: impl FnMut(f32) -> ControlFlow<()>,
) -> Result<Motion, MediaError> {
    let mut reader = MediaReader::open_streams(path, timebase, Streams::Video)?;
    let video = reader
        .info()
        .video
        .as_ref()
        .ok_or(MediaError::NoPlayableStream)?;
    let size = analysis_size(video.width, video.height, ANALYSIS_WIDTH);
    reader.set_video_size(Some(size))?;
    reader.seek(range.start)?;
    let span = (range.end - range.start).0.max(1) as f32;
    let mut previous: Option<Pyramid> = None;
    let mut steps = Vec::new();
    while let Some(frame) = reader.next_frame()? {
        let DecodedFrame::Video(frame) = frame else {
            continue;
        };
        if frame.pts >= range.end {
            break;
        }
        let pyramid = Pyramid::new(Gray::from_rgba(&frame.pixels, frame.width, frame.height));
        let step = previous
            .as_ref()
            .map_or(Similarity::IDENTITY, |previous| step(previous, &pyramid));
        steps.push((frame.pts, step));
        previous = Some(pyramid);
        let done = (frame.pts - range.start).0 as f32 / span;
        if progress(done.clamp(0.0, 1.0)).is_break() {
            return Err(MediaError::Cancelled);
        }
    }
    Ok(Motion {
        size: [size.0, size.1],
        steps,
    })
}

/// The picture shrunk to `target` wide, unless it's narrower already.
pub(crate) fn analysis_size(width: u32, height: u32, target: u32) -> (u32, u32) {
    if width <= target || width == 0 {
        return (width, height);
    }
    let scaled = u64::from(height) * u64::from(target) / u64::from(width);
    // Even, as some pixel formats need.
    let height = u32::try_from(scaled / 2 * 2).unwrap_or(2).max(2);
    (target, height)
}

/// Where the points of `previous` went in `next`.
fn step(previous: &Pyramid, next: &Pyramid) -> Similarity {
    let [width, height] = previous.size().map(f64::from);
    let unit = width.max(height);
    let square = |[x, y]: [f64; 2]| {
        [
            (x + 0.5 - width / 2.0) / unit,
            (y + 0.5 - height / 2.0) / unit,
        ]
    };
    let pairs: Vec<_> = previous
        .matches(next)
        .into_iter()
        .map(|(from, to)| (square(from), square(to)))
        .collect();
    robust_fit(
        &pairs,
        MIN_OUTLIER_PIXELS / unit,
        MaskTracking::PositionScaleRotation,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_picture::{draw, texture};

    /// The texture as seen after `motion` carried its points.
    fn picture(motion: &Similarity) -> Pyramid {
        let back = motion.inverse();
        draw(|point| {
            let [x, y] = back.apply(point);
            texture(x * 320.0, y * 320.0)
        })
    }

    #[test]
    fn a_shift_turn_and_zoom_between_frames_is_recovered() {
        let moved = Similarity {
            shift: [0.03, -0.015],
            angle: 0.02,
            scale: 1.015,
        };
        let found = step(&picture(&Similarity::IDENTITY), &picture(&moved));
        let off = (found.shift[0] - moved.shift[0]).abs() + (found.shift[1] - moved.shift[1]).abs();
        assert!(off < 1e-3, "{found:?}");
        assert!((found.angle - moved.angle).abs() < 2e-3, "{found:?}");
        assert!((found.scale - moved.scale).abs() < 2e-3, "{found:?}");
    }
}
