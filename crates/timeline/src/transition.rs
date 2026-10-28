use serde::{Deserialize, Serialize};

use crate::Ticks;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransitionKind {
    /// Blends the outgoing picture into the incoming one.
    CrossDissolve,
    /// Fades the outgoing picture to black, then the incoming one up from black.
    DipToBlack,
}

impl TransitionKind {
    pub const ALL: [Self; 2] = [Self::CrossDissolve, Self::DipToBlack];

    pub fn name(self) -> &'static str {
        match self {
            Self::CrossDissolve => "Cross Dissolve",
            Self::DipToBlack => "Dip to Black",
        }
    }
}

/// A transition at one end of a clip. At the head, when another clip ends exactly where
/// this one starts, it is centered on the cut and plays both clips past their edges;
/// otherwise it fades this clip in from nothing or out to nothing within its own length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Transition {
    pub kind: TransitionKind,
    /// In the sequence timebase.
    pub duration: Ticks,
}
