use serde::{Deserialize, Serialize};

use crate::{Border, ClipId, Edge, Sweep, Ticks, Track, WipePattern};

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
    IrisRound,
    IrisDiamond,
    IrisCross,
    ClockWipe,
    WedgeWipe,
    RadialWipe,
    Pinwheel,
    VenetianBlinds,
    CheckerWipe,
    Inset,
    RandomBlocks,
    /// Bands across the picture wipe in from alternate sides.
    BandWipe,
    /// The incoming picture is added onto the outgoing one, brightening the middle.
    AdditiveDissolve,
    /// The brighter of the two pictures shows at each pixel through the middle.
    NonAdditiveDissolve,
    /// The outgoing picture turns away edge-on, then the incoming one turns in.
    FlipOver,
    /// The pictures turn together as two faces of a cube.
    CubeSpin,
    /// Audio only: a linear crossfade, which dips in loudness through the middle.
    ConstantGain,
    /// Audio only: a crossfade linear in decibels, quick to fade out and slow to fade in.
    ExponentialFade,
}

impl TransitionKind {
    pub const VIDEO: [Self; 25] = [
        Self::CrossDissolve,
        Self::AdditiveDissolve,
        Self::NonAdditiveDissolve,
        Self::DipToBlack,
        Self::DipToWhite,
        Self::CrossZoom,
        Self::FlipOver,
        Self::CubeSpin,
        Self::IrisBox,
        Self::IrisRound,
        Self::IrisDiamond,
        Self::IrisCross,
        Self::Wipe,
        Self::BarnDoors,
        Self::ClockWipe,
        Self::WedgeWipe,
        Self::RadialWipe,
        Self::Pinwheel,
        Self::VenetianBlinds,
        Self::CheckerWipe,
        Self::Inset,
        Self::RandomBlocks,
        Self::BandWipe,
        Self::Slide,
        Self::Push,
    ];
    pub const AUDIO: [Self; 4] = [
        Self::CrossDissolve,
        Self::ConstantGain,
        Self::ExponentialFade,
        Self::DipToBlack,
    ];

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
            Self::IrisRound => "Iris Round",
            Self::IrisDiamond => "Iris Diamond",
            Self::IrisCross => "Iris Cross",
            Self::ClockWipe => "Clock Wipe",
            Self::WedgeWipe => "Wedge Wipe",
            Self::RadialWipe => "Radial Wipe",
            Self::Pinwheel => "Pinwheel",
            Self::VenetianBlinds => "Venetian Blinds",
            Self::CheckerWipe => "Checker Wipe",
            Self::Inset => "Inset",
            Self::RandomBlocks => "Random Blocks",
            Self::BandWipe => "Band Wipe",
            Self::AdditiveDissolve => "Additive Dissolve",
            Self::NonAdditiveDissolve => "Non-Additive Dissolve",
            Self::FlipOver => "Flip Over",
            Self::CubeSpin => "Cube Spin",
            Self::ConstantGain => "Constant Gain",
            Self::ExponentialFade => "Exponential Fade",
        }
    }

    pub fn audio_name(self) -> &'static str {
        match self {
            _ if self.is_dip() => "Dip to Silence",
            Self::ConstantGain | Self::ExponentialFade => self.name(),
            _ => "Constant Power",
        }
    }

    /// The shape a wipe uncovers the incoming picture in, for kinds that wipe.
    pub fn pattern(self) -> Option<WipePattern> {
        Some(match self {
            Self::Wipe => WipePattern::Edge,
            Self::BarnDoors => WipePattern::Split,
            Self::IrisBox => WipePattern::Box,
            Self::IrisRound => WipePattern::Round,
            Self::IrisDiamond => WipePattern::Diamond,
            Self::IrisCross => WipePattern::Cross,
            Self::ClockWipe => WipePattern::Clock,
            Self::WedgeWipe => WipePattern::Wedge,
            Self::RadialWipe => WipePattern::Radial,
            Self::Pinwheel => WipePattern::Pinwheel,
            Self::VenetianBlinds => WipePattern::Blinds,
            Self::CheckerWipe => WipePattern::Checker,
            Self::Inset => WipePattern::Inset,
            Self::RandomBlocks => WipePattern::Blocks,
            Self::BandWipe => WipePattern::Bands,
            _ => return None,
        })
    }

    /// Whether the side the incoming picture enters from can be chosen.
    pub fn has_direction(self) -> bool {
        matches!(
            self,
            Self::Wipe
                | Self::BarnDoors
                | Self::Slide
                | Self::Push
                | Self::RadialWipe
                | Self::VenetianBlinds
                | Self::CheckerWipe
                | Self::Inset
                | Self::BandWipe
                | Self::FlipOver
                | Self::CubeSpin
        )
    }

    pub fn default_side(self) -> Side {
        match self {
            Self::Slide | Self::Push | Self::FlipOver | Self::CubeSpin => Side::Right,
            Self::VenetianBlinds => Side::Top,
            _ => Side::Left,
        }
    }

    /// Whether audio goes silent at the cut instead of crossfading through it.
    pub fn is_dip(self) -> bool {
        matches!(self, Self::DipToBlack | Self::DipToWhite)
    }
}

/// The canvas edge a transition's incoming picture enters from. Barn doors open across
/// it: left or right opens from a vertical line, top or bottom from a horizontal one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Left,
    Top,
    Right,
    Bottom,
}

impl Side {
    pub const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Top, Self::Bottom];

    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "From Left",
            Self::Top => "From Top",
            Self::Right => "From Right",
            Self::Bottom => "From Bottom",
        }
    }
}

/// Where a transition at a cut sits: Premiere's Center at Cut, Start at Cut and End at
/// Cut. Only the clips under it run past their edges, so starting at the cut needs no
/// handle on the outgoing clip and ending at it none on the incoming one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Alignment {
    #[default]
    Center,
    StartAtCut,
    EndAtCut,
}

impl Alignment {
    pub const ALL: [Self; 3] = [Self::Center, Self::StartAtCut, Self::EndAtCut];

    pub fn name(self) -> &'static str {
        match self {
            Self::Center => "Center at Cut",
            Self::StartAtCut => "Start at Cut",
            Self::EndAtCut => "End at Cut",
        }
    }

    fn is_center(&self) -> bool {
        *self == Self::Center
    }
}

/// A transition at one end of a clip. At the head, when another clip ends exactly where
/// this one starts, it sits on the cut as `alignment` says and plays both clips past
/// their edges; otherwise it fades this clip in from nothing or out to nothing within
/// its own length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Transition {
    pub kind: TransitionKind,
    /// In the sequence timebase.
    pub duration: Ticks,
    #[serde(default, skip_serializing_if = "Alignment::is_center")]
    pub alignment: Alignment,
    /// The kind's usual side when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Side>,
    /// For wipes only.
    #[serde(default, skip_serializing_if = "Sweep::is_forward")]
    pub sweep: Sweep,
    /// For wipes only.
    #[serde(default, skip_serializing_if = "Border::is_none")]
    pub border: Border,
}

impl Transition {
    pub const fn new(kind: TransitionKind, duration: Ticks) -> Self {
        Self {
            kind,
            duration,
            alignment: Alignment::Center,
            from: None,
            sweep: Sweep::Forward,
            border: Border::NONE,
        }
    }

    pub fn side(self) -> Side {
        self.from.unwrap_or(self.kind.default_side())
    }
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
