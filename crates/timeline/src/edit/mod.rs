mod apply_effect;
mod arrange;
mod attributes;
mod automate;
mod bin;
mod caption;
mod enable;
mod frame_size;
mod generate;
mod group;
mod hold;
mod interpret;
mod label;
mod levels;
mod link;
mod look;
mod marker;
mod media;
mod merge;
mod multicam;
mod nest;
mod ops;
mod paste;
mod place;
mod range;
mod reframe;
mod remap;
mod remix;
mod render_replace;
mod replace;
mod sound;
mod speed;
mod structure;
mod submix;
mod subsequence;
mod sync;
mod synchronize;
mod through;
mod trim;

#[cfg(test)]
mod tests;

pub use apply_effect::{ApplyEffect, ClipEffect};
pub use arrange::{CloseGap, Lift, MoveClips, MoveMode, RippleDelete};
pub use attributes::{Attribute, PasteAttributes, RemoveAttributes};
pub use automate::{AutomateToSequence, AutomatedPiece};
pub use bin::{AddBin, MoveToBin, RemoveBin, RenameBin, SetBinSearch};
pub use caption::SetCaptions;
pub use enable::SetClipsEnabled;
pub use frame_size::{FrameFit, SetToFrameSize};
pub use generate::{AddGenerated, SetGenerator};
pub use group::SetGrouped;
pub use hold::{AddFrameHold, HoldFrame, InsertFrameHold, SetFrameHold};
pub use interpret::InterpretFootage;
pub use label::{RenameClips, SetClipLabel};
pub use levels::{
    AutoDuck, NormalizeClipGains, NudgeClipGain, SetClipAudioEffects, SetClipGain, SetClipLine,
    SetClipPan, SetClipVolume, SetMasterEffects, SetMasterGain, SetTrackAutomation,
    SetTrackEffects, SetTrackMix,
};
pub use link::{SetLinked, SetLinkedSelection, Unlinked};
pub use look::{ApplyTransitions, RemoveTransitions, SetClipLook, SetTransition};
pub use marker::{AddMarker, ClearMarkers, MarkerOwner, RemoveMarker, UpdateMarker};
pub use media::{
    BreakoutToMono, DuplicateMedia, MakeOffline, MakeSubclip, RelinkMedia, RenameMedia,
    SetChannelMap, SetMediaLog,
};
pub use merge::MergeClips;
pub use multicam::{CreateMulticam, FlattenMulticam, SwitchAngle};
pub use nest::{AddNest, InNest, NestClips, NestFromMedia};
pub use paste::{Clipboard, PasteClips};
pub use place::{AddEdits, PlaceClip, PlaceClips, PlaceMode, Razor, RazorTarget};
pub use range::{ExtractMarked, LiftMarked, RippleTrimToPlayhead, SetMarks, TrimToPlayhead};
pub use reframe::ReframeSequence;
pub use remap::{
    AddSpeedKeyframe, RemoveSpeedKeyframe, RemoveTimeRemap, SetMaintainPitch, SetSegmentSpeed,
};
pub use remix::RemixClip;
pub use render_replace::{RenderAndReplace, RestoreUnrendered};
pub use replace::ReplaceClip;
pub use sound::{ClipSound, SetClipsSound};
pub use speed::{RateStretch, SetClipSpeed, SetTimeInterpolation};
pub use structure::{
    AddTrack, ImportMedia, RemoveMedia, RemoveTrack, RenameTrack, SetProxy, SetRippleMarkers,
    SetSequenceFormat, SetTimeDisplay, SetTrackSwitch, TrackSwitch,
};
pub use submix::{AddSubmix, RemoveSubmix, SetSubmix, SetTrackOutput, SetTrackSends};
pub use subsequence::{unused_time, CloseGaps, MakeSubsequence};
pub use sync::{IntoSync, SyncMethod};
pub use synchronize::{synchronized_clips, SynchronizeClips};
pub use through::{is_through_edit, JoinThroughEdits};
pub use trim::{Edge, Roll, Slide, Slip, Trim, TrimMode};

use crate::{preview, Edit, Sequence, Ticks};

/// The legal delta closest to `desired` for an edit built by `make`, found by bisecting
/// toward zero. Relies on legality shrinking monotonically as the delta grows, which
/// holds for trims, rolls, slips and slides. Returns zero when no step is legal.
pub fn clamp_delta<E: Edit>(
    sequence: &Sequence,
    desired: Ticks,
    make: impl Fn(Ticks) -> E,
) -> Ticks {
    DeltaBounds::default().clamp(sequence, desired, make)
}

/// What clamping one edit has found out about which of its deltas are legal, so a drag
/// that clamps every frame tries the edit only when the pointer goes somewhere new.
/// It holds only while the sequence and everything about the edit but its delta stay
/// the same.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeltaBounds {
    later: Side,
    earlier: Side,
}

/// Distances from zero one way: the furthest found legal and the nearest found not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Side {
    legal: i64,
    illegal: Option<i64>,
}

impl DeltaBounds {
    /// [`clamp_delta`], trying the edit only where earlier calls haven't settled it.
    pub fn clamp<E: Edit>(
        &mut self,
        sequence: &Sequence,
        desired: Ticks,
        make: impl Fn(Ticks) -> E,
    ) -> Ticks {
        let sign = desired.0.signum();
        let side = if sign < 0 {
            &mut self.earlier
        } else {
            &mut self.later
        };
        let legal = |distance: i64| preview(sequence, &make(Ticks(sign * distance))).is_ok();
        let wanted = desired.0.abs();
        if wanted <= side.legal {
            return desired;
        }
        if side.illegal.is_none_or(|illegal| wanted < illegal) {
            if legal(wanted) {
                side.legal = wanted;
                return desired;
            }
            side.illegal = Some(wanted);
        }
        let mut bad = side.illegal.unwrap_or(wanted);
        while bad - side.legal > 1 {
            let middle = side.legal + (bad - side.legal) / 2;
            if legal(middle) {
                side.legal = middle;
            } else {
                bad = middle;
            }
        }
        side.illegal = Some(bad);
        Ticks(sign * side.legal)
    }
}
