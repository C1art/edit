//! Finding where a picture's subject is, for Auto Reframe.

use std::ops::{ControlFlow, Range};
use std::path::Path;

use timeline::{Ticks, Timebase};

use crate::motion::analysis_size;
use crate::{DecodedFrame, MediaError, MediaReader, Streams};

/// Small enough to analyze quickly; the subject's place needs no detail.
const ANALYSIS_WIDTH: u32 = 160;
/// Luma steps below which edges and motion count as noise, so a flat or still picture
/// doesn't make much of its grain.
const NOISE_FLOOR: f32 = 4.0;
/// How widely, as shares of the picture, the subject is expected around its middle.
const CENTER_SPREAD: [f32; 2] = [0.35, 0.45];

/// Where the subject of each frame in `range` of `path` is, as fractions of the
/// picture from its top left. `progress` hears the fraction done and can stop the
/// work. Blocks on decoding, so run it on a worker.
pub fn track_subject(
    path: &Path,
    timebase: Timebase,
    range: Range<Ticks>,
    mut progress: impl FnMut(f32) -> ControlFlow<()>,
) -> Result<Vec<(Ticks, [f32; 2])>, MediaError> {
    let mut reader = MediaReader::open_streams(path, timebase, Streams::Video)?;
    let video = reader
        .info()
        .video
        .as_ref()
        .ok_or(MediaError::NoPlayableStream)?;
    reader.set_video_size(Some(analysis_size(
        video.width,
        video.height,
        ANALYSIS_WIDTH,
    )))?;
    reader.seek(range.start)?;
    let span = (range.end - range.start).0.max(1) as f32;
    let mut previous: Option<Vec<f32>> = None;
    let mut centers = Vec::new();
    while let Some(frame) = reader.next_frame()? {
        let DecodedFrame::Video(frame) = frame else {
            continue;
        };
        if frame.pts >= range.end {
            break;
        }
        let luma = luma(&frame.pixels);
        let picture = Picture {
            rgba: &frame.pixels,
            luma: &luma,
            width: frame.width as usize,
            height: frame.height as usize,
        };
        centers.push((frame.pts, picture.subject(previous.as_deref())));
        previous = Some(luma);
        let done = (frame.pts - range.start).0 as f32 / span;
        if progress(done.clamp(0.0, 1.0)).is_break() {
            return Err(MediaError::Cancelled);
        }
    }
    Ok(centers)
}

fn luma(rgba: &[u8]) -> Vec<f32> {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .map(|&[r, g, b, _]| 0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b))
        .collect()
}

struct Picture<'a> {
    rgba: &'a [u8],
    luma: &'a [f32],
    width: usize,
    height: usize,
}

impl Picture<'_> {
    /// The center of what stands out: edges, what moved since `previous` and skin,
    /// favoring the middle of the picture as a camera operator would.
    fn subject(&self, previous: Option<&[f32]>) -> [f32; 2] {
        let (width, height) = (self.width, self.height);
        if width < 3 || height < 3 || self.luma.len() < width * height {
            return [0.5; 2];
        }
        let index = |x: usize, y: usize| y * width + x;
        let edges: Vec<f32> = (0..width * height)
            .map(|at| {
                let (x, y) = (at % width, at / width);
                let (left, right) = (x.saturating_sub(1), (x + 1).min(width - 1));
                let (up, down) = (y.saturating_sub(1), (y + 1).min(height - 1));
                (self.luma[index(right, y)] - self.luma[index(left, y)]).abs()
                    + (self.luma[index(x, down)] - self.luma[index(x, up)]).abs()
            })
            .collect();
        let motion: Vec<f32> = match previous.filter(|previous| previous.len() == self.luma.len()) {
            Some(previous) => self
                .luma
                .iter()
                .zip(previous)
                .map(|(now, before)| (now - before).abs())
                .collect(),
            None => vec![0.0; self.luma.len()],
        };
        let relative = |values: &[f32]| {
            let mean = values.iter().sum::<f32>() / values.len() as f32;
            mean.max(NOISE_FLOOR)
        };
        let (edge_scale, motion_scale) = (relative(&edges), relative(&motion));
        let mut sum = [0.0f64; 2];
        let mut total = 0.0f64;
        for (at, (&edge, &moved)) in edges.iter().zip(&motion).enumerate() {
            let (x, y) = (at % width, at / width);
            let fraction = [
                (x as f32 + 0.5) / width as f32,
                (y as f32 + 0.5) / height as f32,
            ];
            let prior = (0..2)
                .map(|axis| ((fraction[axis] - 0.5) / CENTER_SPREAD[axis]).powi(2))
                .sum::<f32>();
            let skin = if is_skin(&self.rgba[at * 4..at * 4 + 3]) {
                1.5
            } else {
                0.0
            };
            let salience =
                (0.6 * edge / edge_scale + moved / motion_scale + skin) * (-0.5 * prior).exp();
            // Squared, so the strongest part outweighs a wide spread of weak texture.
            let weight = f64::from(salience * salience);
            sum[0] += weight * f64::from(fraction[0]);
            sum[1] += weight * f64::from(fraction[1]);
            total += weight;
        }
        if total <= f64::EPSILON {
            return [0.5; 2];
        }
        sum.map(|sum| (sum / total) as f32)
    }
}

/// Whether a color falls in the range of skin tones in YCbCr, whatever the light.
fn is_skin(rgb: &[u8]) -> bool {
    let [r, g, b] = [0, 1, 2].map(|channel| f32::from(rgb[channel]));
    let cb = 128.0 - 0.168_736 * r - 0.331_264 * g + 0.5 * b;
    let cr = 128.0 + 0.5 * r - 0.418_688 * g - 0.081_312 * b;
    (77.0..=127.0).contains(&cb) && (133.0..=173.0).contains(&cr)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grey picture 40 by 20 with a patch of `color` around `(x, y)`.
    fn picture_with_patch(color: [u8; 3], (x, y): (usize, usize)) -> Vec<u8> {
        (0..40 * 20_usize)
            .flat_map(|at| {
                let (px, py) = (at % 40, at / 40);
                let near = px.abs_diff(x) <= 2 && py.abs_diff(y) <= 2;
                let [r, g, b] = if near { color } else { [100; 3] };
                [r, g, b, 255]
            })
            .collect()
    }

    fn subject(rgba: &[u8], previous: Option<&[u8]>) -> [f32; 2] {
        let luma_now = luma(rgba);
        let luma_before = previous.map(luma);
        let picture = Picture {
            rgba,
            luma: &luma_now,
            width: 40,
            height: 20,
        };
        picture.subject(luma_before.as_deref())
    }

    #[test]
    fn the_subject_is_found_where_the_picture_stands_out() {
        let face = picture_with_patch([220, 160, 130], (30, 10));
        let [x, y] = subject(&face, None);
        assert!(x > 0.65 && (y - 0.5).abs() < 0.1, "{x}, {y}");
        assert_eq!(
            subject(&picture_with_patch([100; 3], (5, 5)), None),
            [0.5, 0.5]
        );
    }

    #[test]
    fn what_moved_draws_the_subject_toward_it() {
        let still = picture_with_patch([40, 40, 40], (30, 10));
        let mover_before = picture_with_patch([40, 40, 40], (8, 10));
        let mut moved = still.clone();
        // The patch on the right stays; one on the left appears.
        for (pixel, before) in moved.chunks_mut(4).zip(mover_before.chunks(4)) {
            if before[0] == 40 {
                pixel.copy_from_slice(before);
            }
        }
        let [without, _] = subject(&moved, Some(&moved));
        let [with, _] = subject(&moved, Some(&still));
        assert!(with < without - 0.1, "{with} vs {without}");
    }
}
