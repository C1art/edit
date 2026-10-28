use std::sync::Arc;

use media::VideoFrame;
use render::{Grade, LayerImage, LayerLook, Picture};
use timeline::{Ticks, TrackRef};

/// One picture placed in the program picture.
#[derive(Debug, Clone)]
pub struct Layer {
    pub track: TrackRef,
    pub picture: LayerPicture,
    pub look: LayerLook,
    pub grade: Option<Arc<Grade>>,
}

#[derive(Debug, Clone)]
pub enum LayerPicture {
    Frame(Arc<VideoFrame>),
    /// What a nested sequence shows, bottom layer first, composited before the look.
    Nest(Vec<Layer>),
}

impl Layer {
    pub fn image(&self) -> LayerImage<'_> {
        let picture = match &self.picture {
            LayerPicture::Frame(frame) => Picture::Pixels {
                width: frame.width,
                height: frame.height,
                rgba: &frame.pixels,
            },
            LayerPicture::Nest(layers) => Picture::Group(layers.iter().map(Layer::image).collect()),
        };
        LayerImage {
            picture,
            look: self.look,
            grade: self.grade.clone(),
        }
    }
}

/// Everything visible at one frame of the sequence, bottom layer first.
#[derive(Debug, Clone)]
pub struct ComposedFrame {
    pub pts: Ticks,
    pub layers: Vec<Layer>,
}

/// Peak sample magnitudes, left and right, of one stretch of mixed audio.
#[derive(Debug, Clone, PartialEq)]
pub struct Levels {
    /// Sequence time where the measured audio starts.
    pub pts: Ticks,
    /// Post-fader, one entry per audio track.
    pub tracks: Vec<[f32; 2]>,
    pub master: [f32; 2],
}
