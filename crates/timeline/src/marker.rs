use serde::{Deserialize, Serialize};

use crate::Ticks;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MarkerId(pub u64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarkerColor {
    #[default]
    Green,
    Red,
    Purple,
    Orange,
    Yellow,
    White,
    Blue,
    Cyan,
}

impl MarkerColor {
    pub const ALL: [Self; 8] = [
        Self::Green,
        Self::Red,
        Self::Purple,
        Self::Orange,
        Self::Yellow,
        Self::White,
        Self::Blue,
        Self::Cyan,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Green => "Green",
            Self::Red => "Red",
            Self::Purple => "Purple",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::White => "White",
            Self::Blue => "Blue",
            Self::Cyan => "Cyan",
        }
    }
}

/// A named point in the sequence, for notes and navigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    pub id: MarkerId,
    pub at: Ticks,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub color: MarkerColor,
}
