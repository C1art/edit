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

/// A copy of an audio track's sound sent to a submix as well as to its output, as for
/// a reverb several tracks share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackSend {
    pub to: SubmixId,
    #[serde(default)]
    pub level: Gain,
    #[serde(default)]
    pub pan: Pan,
    #[serde(default)]
    pub tap: SendTap,
    #[serde(default)]
    pub muted: bool,
}

impl TrackSend {
    /// The most sends a track has, as in Premiere's mixer.
    pub const MAX: usize = 5;

    pub fn new(to: SubmixId) -> Self {
        Self {
            to,
            level: Gain::UNITY,
            pan: Pan::CENTER,
            tap: SendTap::default(),
            muted: false,
        }
    }
}

/// Where along a track a send takes its sound from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum SendTap {
    /// After the track's fader and automation, so it follows them.
    #[default]
    PostFader,
    /// After the track's effects but before its fader, so the fader leaves it be.
    PreFader,
}

impl SendTap {
    pub const ALL: [Self; 2] = [Self::PostFader, Self::PreFader];

    pub fn name(self) -> &'static str {
        match self {
            Self::PostFader => "Post-Fader",
            Self::PreFader => "Pre-Fader",
        }
    }
}
