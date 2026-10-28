use serde::{Deserialize, Serialize};

use crate::{AudioEffect, Gain, Pan};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SubmixId(pub u64);

/// A bus audio tracks can be sent to instead of the master, so a group of them shares
/// one fader and one set of effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submix {
    pub id: SubmixId,
    pub name: String,
    #[serde(default)]
    pub volume: Gain,
    #[serde(default)]
    pub pan: Pan,
    #[serde(default)]
    pub muted: bool,
    /// Run in order over the sum of the tracks sent here, before the fader.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<AudioEffect>,
}

impl Submix {
    pub fn new(id: SubmixId, name: String) -> Self {
        Self {
            id,
            name,
            volume: Gain::UNITY,
            pan: Pan::CENTER,
            muted: false,
            effects: Vec::new(),
        }
    }
}
