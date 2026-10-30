use serde::{Deserialize, Serialize};

/// Which way a guide runs across the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GuideAxis {
    /// A vertical line, `at` pixels from the frame's left.
    Vertical,
    /// A horizontal line, `at` pixels from the frame's top.
    Horizontal,
}

/// A line over the program frame to line pictures up with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Guide {
    pub axis: GuideAxis,
    pub at: i32,
}
