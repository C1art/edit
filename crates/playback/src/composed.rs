use std::sync::Arc;

use media::VideoFrame;
use timeline::{Ticks, TrackRef};

/// One source picture placed in the program picture.
#[derive(Debug, Clone)]
pub struct Layer {
    pub track: TrackRef,
    pub frame: Arc<VideoFrame>,
}

/// Everything visible at one frame of the sequence, bottom layer first.
#[derive(Debug, Clone)]
pub struct ComposedFrame {
    pub pts: Ticks,
    pub layers: Vec<Layer>,
}
