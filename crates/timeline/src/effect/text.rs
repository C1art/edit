use serde::{Deserialize, Serialize};

use crate::{Param, TextAlign};

/// Burns typed text into the picture, white on a box, as Clip Name burns the name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SimpleText {
    pub text: String,
    /// The box's center, in percent of the picture's width and height.
    pub position: [Param; 2],
    /// How lines line up inside the box.
    pub alignment: TextAlign,
    /// Text height in percent of the picture's height.
    pub size: Param,
    /// Percent opacity of the box behind the text.
    pub opacity: Param,
}

impl Default for SimpleText {
    fn default() -> Self {
        Self {
            text: "Simple Text".to_owned(),
            position: [Param::from_value(50.0), Param::from_value(50.0)],
            alignment: TextAlign::Center,
            size: Param::from_value(6.0),
            opacity: Param::default(),
        }
    }
}
