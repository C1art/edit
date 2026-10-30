use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use media::VideoFrame;
use timeline::{Clip, Generator, GraphicContent, MediaItem, Sequence, Ticks};

use super::layers::{draw_layer, Drawn, Placement};
use super::Canvas;

/// Layers kept drawn between frames; a graphic rarely has more.
const KEPT_LAYERS: usize = 12;

type DrawnLayers = Vec<((GraphicContent, [u32; 2]), Rc<Option<Drawn>>)>;

thread_local! {
    /// Each layer drawn once, as only where and how it lands changes while animating.
    static DRAWN: RefCell<DrawnLayers> = const { RefCell::new(Vec::new()) };
}

/// A graphic's picture at source `time` of `clip`, with its layers coming on or going
/// off; other pictures, and graphics with every layer settled, pass through.
pub(crate) fn animated(
    picture: Arc<VideoFrame>,
    sequence: &Sequence,
    clip: &Clip,
    media: &MediaItem,
    time: Ticks,
) -> Arc<VideoFrame> {
    let Some(Generator::Graphic(graphic)) = &media.generator else {
        return picture;
    };
    if graphic
        .layers
        .iter()
        .all(|layer| layer.animation.is_still())
    {
        return picture;
    }
    let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
    let frame = rate.frame_at(clip.position_of(time) - clip.start, timebase);
    let frames = rate.frame_at(clip.duration, timebase);
    let poses: Vec<_> = graphic
        .layers
        .iter()
        .map(|layer| layer.animation.pose(frame, frames))
        .collect();
    if poses.iter().all(|pose| pose.is_as_set()) {
        return picture;
    }
    let (width, height) = (picture.width, picture.height);
    let mut canvas = Canvas::new(width, height);
    for (layer, pose) in graphic.layers.iter().zip(poses).rev() {
        if layer.hidden {
            continue;
        }
        let drawn = drawn(&layer.content, [width, height]);
        if let Some(drawn) = drawn.as_ref() {
            let placement = Placement::of(layer, width).posed(pose, height);
            canvas.cover(drawn, &placement);
        }
    }
    Arc::new(VideoFrame {
        pts: Ticks::ZERO,
        width,
        height,
        pixels: canvas.into_pixels(),
    })
}

fn drawn(content: &GraphicContent, size: [u32; 2]) -> Rc<Option<Drawn>> {
    DRAWN.with_borrow_mut(|kept| {
        if let Some((_, drawn)) = kept
            .iter()
            .find(|((kept, kept_size), _)| kept == content && *kept_size == size)
        {
            return Rc::clone(drawn);
        }
        let drawn = Rc::new(draw_layer(content, size[0], size[1]));
        if kept.len() >= KEPT_LAYERS {
            kept.remove(0);
        }
        kept.push(((content.clone(), size), Rc::clone(&drawn)));
        drawn
    })
}
