use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use media::{DecodedFrame, MediaError, MediaReader, Streams, VideoFrame};
use timeline::{Ticks, Timebase};

use super::Canvas;

/// About as many pictures as the graphics on screen at once hold.
const KEPT_PICTURES: usize = 8;

type PictureKey = (PathBuf, [u32; 2]);

thread_local! {
    /// Each picture read to fit a canvas size, or `None` where it couldn't be read.
    /// Most recently used last.
    static PICTURES: RefCell<Vec<(PictureKey, Option<Arc<VideoFrame>>)>> =
        const { RefCell::new(Vec::new()) };
}

/// Draws the picture at `path` centered on the canvas, at its own size or shrunk to
/// fit. An unreadable file draws nothing, as a missing logo shouldn't hide the rest.
pub(super) fn draw_image(canvas: &mut Canvas, path: &Path) {
    let Some(picture) = picture(path, [canvas.width, canvas.height]) else {
        return;
    };
    let (width, height) = (picture.width as usize, picture.height as usize);
    let left = (canvas.width as usize).saturating_sub(width) / 2;
    let top = (canvas.height as usize).saturating_sub(height) / 2;
    let canvas_width = canvas.width as usize;
    for (y, row) in picture.pixels.chunks_exact(width * 4).enumerate() {
        let start = (top + y) * canvas_width + left;
        let Some(out) = canvas.pixels.get_mut(start..start + width) else {
            break;
        };
        for (out, pixel) in out.iter_mut().zip(row.as_chunks::<4>().0) {
            *out = pixel.map(|channel| f32::from(channel) / 255.0);
        }
    }
}

fn picture(path: &Path, canvas: [u32; 2]) -> Option<Arc<VideoFrame>> {
    PICTURES.with_borrow_mut(|pictures| {
        let key = (path.to_owned(), canvas);
        if let Some(index) = pictures.iter().position(|(kept, _)| *kept == key) {
            let entry = pictures.remove(index);
            let picture = entry.1.clone();
            pictures.push(entry);
            return picture;
        }
        let picture = read(path, canvas).ok().map(Arc::new);
        if pictures.len() >= KEPT_PICTURES {
            pictures.remove(0);
        }
        pictures.push((key, picture.clone()));
        picture
    })
}

fn read(path: &Path, canvas: [u32; 2]) -> Result<VideoFrame, MediaError> {
    let timebase = Timebase::new(1, 1).ok_or(MediaError::NoPlayableStream)?;
    let mut reader = MediaReader::open_streams(path, timebase, Streams::Video)?;
    let video = reader.info().video.as_ref();
    let size = video.ok_or(MediaError::NoPlayableStream)?;
    let size = fitted([size.width, size.height], canvas);
    reader.set_video_size(Some(size))?;
    while let Some(decoded) = reader.next_frame()? {
        if let DecodedFrame::Video(mut frame) = decoded {
            frame.pts = Ticks::ZERO;
            return Ok(frame);
        }
    }
    Err(MediaError::NoPlayableStream)
}

/// `size`, kept as it is when it fits within `canvas`, else shrunk to fit with its
/// shape kept.
fn fitted([width, height]: [u32; 2], [canvas_width, canvas_height]: [u32; 2]) -> (u32, u32) {
    if width <= canvas_width && height <= canvas_height {
        return (width.max(1), height.max(1));
    }
    let scale = (f64::from(canvas_width) / f64::from(width))
        .min(f64::from(canvas_height) / f64::from(height));
    let shrink = |side: u32| ((f64::from(side) * scale).round() as u32).max(1);
    (shrink(width), shrink(height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_keep_their_size_unless_too_big_for_the_frame() {
        assert_eq!(fitted([400, 200], [1920, 1080]), (400, 200));
        assert_eq!(fitted([3840, 1080], [1920, 1080]), (1920, 540));
        assert_eq!(fitted([1000, 4000], [1920, 1080]), (270, 1080));
    }

    #[test]
    #[ignore = "decodes a fixture with FFmpeg"]
    fn a_picture_smaller_than_the_frame_is_centered_at_its_own_size() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/testsrc_2s_25fps_44k.mp4");
        let picture = picture(&path, [4000, 4000]).unwrap();
        let mut canvas = Canvas::new(picture.width + 20, picture.height + 20);
        draw_image(&mut canvas, &path);
        let at = |x: u32, y: u32| canvas.pixels[(y * canvas.width + x) as usize][3];
        assert_eq!(at(5, 5), 0.0);
        assert_eq!(at(10, 10), 1.0);
        assert_eq!(at(canvas.width / 2, canvas.height / 2), 1.0);
        assert_eq!(at(canvas.width - 5, canvas.height - 5), 0.0);
    }
}
