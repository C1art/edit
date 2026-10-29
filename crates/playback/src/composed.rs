use std::any::Any;
use std::sync::Arc;

use media::{Chroma, PlanarFrame, Plane, VideoFrame};
use render::{ChromaRows, Combine, Grade, LayerImage, LayerLook, Picture, PlaneRows};
use timeline::{MatteChannel, Ticks, TrackRef};

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
    /// A decoded picture left in its planes, for the GPU to turn into RGB.
    Planes(Arc<PlanarFrame>),
    /// What a nested sequence shows, bottom layer first, composited before the look.
    Nest(Vec<Layer>),
    /// A clip's layer shown where the layers of its track matte let it.
    Matted {
        fill: Box<Layer>,
        matte: Vec<Layer>,
        channel: MatteChannel,
        reverse: bool,
    },
    /// A clip's layer combined with the layers of the track one of its effects reads.
    Combined {
        layer: Box<Layer>,
        other: Vec<Layer>,
        combine: Combine,
    },
}

impl Layer {
    pub fn image(&self) -> LayerImage<'_> {
        let picture = match &self.picture {
            LayerPicture::Frame(frame) => Picture::Pixels {
                width: frame.width,
                height: frame.height,
                rgba: &frame.pixels,
                source: Some(Arc::clone(frame) as Arc<dyn Any + Send + Sync>),
            },
            LayerPicture::Planes(frame) => {
                let planes = frame.planes();
                Picture::Planes {
                    width: frame.width,
                    height: frame.height,
                    luma: rows(planes.luma),
                    chroma: match planes.chroma {
                        Chroma::Separate(cb, cr) => ChromaRows::Separate(rows(cb), rows(cr)),
                        Chroma::Interleaved(both) => ChromaRows::Interleaved(rows(both)),
                    },
                    color: frame.color,
                    source: Some(Arc::clone(frame) as Arc<dyn Any + Send + Sync>),
                }
            }
            LayerPicture::Nest(layers) => Picture::Group(images(layers)),
            LayerPicture::Matted {
                fill,
                matte,
                channel,
                reverse,
            } => Picture::Matted {
                fill: vec![fill.image()],
                matte: images(matte),
                channel: *channel,
                reverse: *reverse,
            },
            LayerPicture::Combined {
                layer,
                other,
                combine,
            } => Picture::Combined {
                layer: vec![layer.image()],
                other: images(other),
                combine: *combine,
            },
        };
        LayerImage {
            picture,
            look: self.look,
            grade: self.grade.clone(),
        }
    }
}

fn rows(plane: Plane<'_>) -> PlaneRows<'_> {
    PlaneRows {
        bytes: plane.bytes,
        stride: plane.stride,
    }
}

fn images(layers: &[Layer]) -> Vec<LayerImage<'_>> {
    layers.iter().map(Layer::image).collect()
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
    /// Post-fader, one entry per submix.
    pub submixes: Vec<[f32; 2]>,
    pub master: [f32; 2],
}
