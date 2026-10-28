use serde::{Deserialize, Serialize};

use crate::{ClipId, Edge, Ticks, Track};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransitionKind {
    /// Blends the outgoing picture into the incoming one; a constant-power crossfade
    /// on audio.
    CrossDissolve,
    /// Fades the outgoing picture to black, then the incoming one up from black.
    DipToBlack,
    DipToWhite,
    /// Zooms into the outgoing picture, then out of the incoming one.
    CrossZoom,
    /// The incoming picture is uncovered from left to right.
    Wipe,
    /// The incoming picture opens out from a vertical line down the middle.
    BarnDoors,
    /// The incoming picture opens out from the center.
    IrisBox,
    /// The incoming picture slides in from the right over the outgoing one.
    Slide,
    /// The incoming picture pushes the outgoing one out to the left.
    Push,
}

impl TransitionKind {
    pub const VIDEO: [Self; 9] = [
        Self::CrossDissolve,
        Self::DipToBlack,
        Self::DipToWhite,
        Self::CrossZoom,
        Self::Wipe,
        Self::BarnDoors,
        Self::IrisBox,
        Self::Slide,
        Self::Push,
    ];
    pub const AUDIO: [Self; 2] = [Self::CrossDissolve, Self::DipToBlack];

    pub fn name(self) -> &'static str {
        match self {
            Self::CrossDissolve => "Cross Dissolve",
            Self::DipToBlack => "Dip to Black",
            Self::DipToWhite => "Dip to White",
            Self::CrossZoom => "Cross Zoom",
            Self::Wipe => "Wipe",
            Self::BarnDoors => "Barn Doors",
            Self::IrisBox => "Iris Box",
            Self::Slide => "Slide",
            Self::Push => "Push",
        }
    }

    pub fn audio_name(self) -> &'static str {
        if self.is_dip() {
            "Dip to Silence"
        } else {
            "Constant Power"
        }
    }

    /// Whether audio goes silent at the cut instead of crossfading through it.
    pub fn is_dip(self) -> bool {
        matches!(self, Self::DipToBlack | Self::DipToWhite)
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

/// The clip end nearest `position` that can hold a transition: the head of a clip that
/// follows another directly owns their cut, and any other start or end is its own.
pub fn nearest_transition_edge(track: &Track, position: Ticks) -> Option<(ClipId, Edge)> {
    let clips = &track.clips;
    let heads = clips.iter().map(|clip| (clip.start, clip.id, Edge::Head));
    let tails = clips
        .iter()
        .enumerate()
        .filter(|(index, clip)| {
            clips
                .get(index + 1)
                .is_none_or(|next| next.start != clip.end())
        })
        .map(|(_, clip)| (clip.end(), clip.id, Edge::Tail));
    heads
        .chain(tails)
        .min_by_key(|(time, ..)| (*time - position).0.abs())
        .map(|(_, clip, edge)| (clip, edge))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Clip, SourceId};

    #[test]
    fn nearest_edge_gives_a_shared_cut_to_the_incoming_clip() {
        let clip = |id, start| {
            Clip::new(
                ClipId(id),
                SourceId(1),
                Ticks(start),
                Ticks(100),
                Ticks::ZERO,
            )
        };
        let track = Track {
            clips: vec![clip(1, 0), clip(2, 100), clip(3, 300)],
            ..Track::default()
        };
        assert_eq!(
            nearest_transition_edge(&track, Ticks(90)),
            Some((ClipId(2), Edge::Head))
        );
        assert_eq!(
            nearest_transition_edge(&track, Ticks(210)),
            Some((ClipId(2), Edge::Tail))
        );
        assert_eq!(
            nearest_transition_edge(&track, Ticks(390)),
            Some((ClipId(3), Edge::Tail))
        );
    }
}
