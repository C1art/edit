use std::sync::Arc;

use media::VideoFrame;
use render::{LayerImage, LayerLook};
use timeline::{Ticks, TrackRef};

/// One source picture placed in the program picture.
#[derive(Debug, Clone)]
pub struct Layer {
    pub track: TrackRef,
    pub frame: Arc<VideoFrame>,
    pub look: LayerLook,
}

impl Layer {
    pub fn image(&self) -> LayerImage<'_> {
        LayerImage {
            width: self.frame.width,
            height: self.frame.height,
            rgba: &self.frame.pixels,
            look: self.look,
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
