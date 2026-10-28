use serde::{Deserialize, Serialize};

/// A color for telling clips apart on the timeline, as Premiere's label colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Label {
    Violet,
    Iris,
    Caribbean,
    Lavender,
    Cerulean,
    Forest,
    Rose,
    Mango,
    Purple,
    Blue,
    Teal,
    Magenta,
    Tan,
    Green,
    Brown,
    Yellow,
}

impl Label {
    pub const ALL: [Self; 16] = [
        Self::Violet,
        Self::Iris,
        Self::Caribbean,
        Self::Lavender,
        Self::Cerulean,
        Self::Forest,
        Self::Rose,
        Self::Mango,
        Self::Purple,
        Self::Blue,
        Self::Teal,
        Self::Magenta,
        Self::Tan,
        Self::Green,
        Self::Brown,
        Self::Yellow,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Violet => "Violet",
            Self::Iris => "Iris",
            Self::Caribbean => "Caribbean",
            Self::Lavender => "Lavender",
            Self::Cerulean => "Cerulean",
            Self::Forest => "Forest",
            Self::Rose => "Rose",
            Self::Mango => "Mango",
            Self::Purple => "Purple",
            Self::Blue => "Blue",
            Self::Teal => "Teal",
            Self::Magenta => "Magenta",
            Self::Tan => "Tan",
            Self::Green => "Green",
            Self::Brown => "Brown",
            Self::Yellow => "Yellow",
        }
    }
}
