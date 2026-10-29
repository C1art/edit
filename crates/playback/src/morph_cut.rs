use std::sync::Arc;

use media::VideoFrame;

use crate::optical_flow::{interpolate, Flow};
use crate::LayerPicture;

/// What a morph cut shows `fraction` of the way from `outgoing` to `incoming`: each
/// picture warped along the motion between them and the two mixed. `None` unless both
/// are decoded pictures of one size, when the cut dissolves instead.
pub(crate) fn morphed(
    outgoing: &LayerPicture,
    incoming: &LayerPicture,
    fraction: f32,
) -> Option<LayerPicture> {
    let (Some(from), Some(to)) = (rgba(outgoing), rgba(incoming)) else {
        return None;
    };
    if (from.width, from.height) != (to.width, to.height) || from.pixels.len() != to.pixels.len() {
        return None;
    }
    let flow = Flow::between(&from, &to);
    Some(LayerPicture::Frame(Arc::new(interpolate(
        &from, &to, &flow, fraction,
    ))))
}

fn rgba(picture: &LayerPicture) -> Option<Arc<VideoFrame>> {
    match picture {
        LayerPicture::Frame(frame) => Some(Arc::clone(frame)),
        LayerPicture::Planes(frame, _) => frame.to_rgba().ok().map(Arc::new),
        _ => None,
    }
}
