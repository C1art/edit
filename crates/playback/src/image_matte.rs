use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use media::{DecodedFrame, MediaError, MediaReader, Streams, VideoFrame};
use timeline::{ImageMatteKey, ImageMatteSource, Timebase, VideoEffect};

use crate::PlaybackError;

/// About as many mattes as clips keyed by one on screen at once.
const KEPT_MATTES: usize = 8;

type MatteKey = (PathBuf, [u32; 2]);

thread_local! {
    /// Each matte read at a picture size, or `None` where it couldn't be, which was
    /// reported once. Most recently used last.
    static MATTES: RefCell<Vec<(MatteKey, Option<Arc<VideoFrame>>)>> =
        const { RefCell::new(Vec::new()) };
}

/// `frame` keyed through the clip's Image Matte Keys, in list order. A key without a
/// readable matte leaves the picture as it is.
pub(crate) fn image_matted(
    frame: Arc<VideoFrame>,
    effects: &[VideoEffect],
    timebase: Timebase,
    warnings: &mut Vec<PlaybackError>,
) -> Arc<VideoFrame> {
    let mut keys = effects
        .iter()
        .filter_map(|effect| match effect {
            VideoEffect::ImageMatteKey(key) => Some(key),
            _ => None,
        })
        .peekable();
    if keys.peek().is_none() {
        return frame;
    }
    let mut keyed = (*frame).clone();
    for key in keys {
        let Some(path) = &key.matte else {
            continue;
        };
        let size = [keyed.width, keyed.height];
        if let Some(matte) = matte(path, size, timebase, warnings) {
            key_through(&mut keyed, &matte, key);
        }
    }
    Arc::new(keyed)
}

fn matte(
    path: &Path,
    size: [u32; 2],
    timebase: Timebase,
    warnings: &mut Vec<PlaybackError>,
) -> Option<Arc<VideoFrame>> {
    MATTES.with_borrow_mut(|mattes| {
        let key = (path.to_owned(), size);
        if let Some(index) = mattes.iter().position(|(kept, _)| *kept == key) {
            let entry = mattes.remove(index);
            let matte = entry.1.clone();
            mattes.push(entry);
            return matte;
        }
        let matte = read(path, size, timebase)
            .map(Arc::new)
            .map_err(|error| {
                warnings.push(PlaybackError::ImageMatte {
                    path: path.to_owned(),
                    error,
                });
            })
            .ok();
        if mattes.len() >= KEPT_MATTES {
            mattes.remove(0);
        }
        mattes.push((key, matte.clone()));
        matte
    })
}

fn read(
    path: &Path,
    [width, height]: [u32; 2],
    timebase: Timebase,
) -> Result<VideoFrame, MediaError> {
    let mut reader = MediaReader::open_streams(path, timebase, Streams::Video)?;
    reader.set_video_size(Some((width, height)))?;
    while let Some(decoded) = reader.next_frame()? {
        if let DecodedFrame::Video(frame) = decoded {
            return Ok(frame);
        }
    }
    Err(MediaError::NoPlayableStream)
}

/// Scales each pixel's alpha by the matte's at the same place.
fn key_through(frame: &mut VideoFrame, matte: &VideoFrame, key: &ImageMatteKey) {
    let pixels = frame.pixels.as_chunks_mut::<4>().0;
    for (pixel, &[r, g, b, a]) in pixels.iter_mut().zip(matte.pixels.as_chunks::<4>().0) {
        let coverage = match key.composite_using {
            ImageMatteSource::MatteAlpha => f32::from(a) / 255.0,
            ImageMatteSource::MatteLuma => {
                let luma = 0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b);
                luma / 255.0 * f32::from(a) / 255.0
            }
        };
        let coverage = if key.reverse {
            1.0 - coverage
        } else {
            coverage
        };
        pixel[3] = (f32::from(pixel[3]) * coverage).round() as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use timeline::Ticks;

    fn frame(pixels: &[[u8; 4]]) -> VideoFrame {
        VideoFrame {
            pts: Ticks(0),
            width: pixels.len() as u32,
            height: 1,
            pixels: pixels.concat(),
        }
    }

    #[test]
    fn the_matte_luma_or_alpha_sets_how_much_of_the_clip_shows() {
        let matte = frame(&[[255, 255, 255, 0], [0, 0, 0, 255], [255, 255, 255, 255]]);
        let alphas = |composite_using, reverse| {
            let mut keyed = frame(&[[9, 9, 9, 255]; 3]);
            let key = ImageMatteKey {
                matte: None,
                composite_using,
                reverse,
            };
            key_through(&mut keyed, &matte, &key);
            keyed
                .pixels
                .iter()
                .skip(3)
                .step_by(4)
                .copied()
                .collect::<Vec<_>>()
        };
        assert_eq!(alphas(ImageMatteSource::MatteAlpha, false), [0, 255, 255]);
        assert_eq!(alphas(ImageMatteSource::MatteLuma, false), [0, 0, 255]);
        assert_eq!(alphas(ImageMatteSource::MatteLuma, true), [255, 255, 0]);
    }
}
