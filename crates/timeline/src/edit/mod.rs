mod arrange;
mod attributes;
mod bin;
mod caption;
mod enable;
mod frame_size;
mod generate;
mod hold;
mod label;
mod levels;
mod link;
mod look;
mod marker;
mod media;
mod multicam;
mod nest;
mod ops;
mod paste;
mod place;
mod range;
mod remap;
mod sound;
mod speed;
mod structure;
mod trim;

#[cfg(test)]
mod tests;

pub use arrange::{CloseGap, Lift, MoveClips, RippleDelete};
pub use attributes::{Attribute, PasteAttributes, RemoveAttributes};
pub use bin::{AddBin, MoveToBin, RemoveBin, RenameBin};
pub use caption::SetCaptions;
pub use enable::SetClipsEnabled;
pub use frame_size::{FrameFit, SetToFrameSize};
pub use generate::{AddGenerated, SetGenerator};
pub use hold::AddFrameHold;
pub use label::SetClipLabel;
pub use levels::{
    AutoDuck, NormalizeClipGains, SetClipAudioEffects, SetClipGain, SetClipVolume, SetMasterGain,
    SetTrackAutomation, SetTrackMix,
};
pub use link::SetLinked;
pub use look::{SetClipLook, SetTransition};
pub use marker::{AddMarker, MarkerOwner, RemoveMarker, UpdateMarker};
pub use media::{MakeSubclip, RelinkMedia, RenameMedia};
pub use multicam::{CreateMulticam, FlattenMulticam, SwitchAngle};
pub use nest::{AddNest, InNest, NestClips};
pub use paste::{Clipboard, PasteClips};
pub use place::{AddEdits, PlaceClip, PlaceMode, Razor, RazorTarget};
pub use range::{ExtractMarked, LiftMarked, RippleTrimToPlayhead, SetMarks};
pub use remap::{AddSpeedKeyframe, RemoveSpeedKeyframe, RemoveTimeRemap, SetSegmentSpeed};
pub use sound::{ClipSound, SetClipsSound};
pub use speed::{RateStretch, SetClipSpeed, SetTimeInterpolation};
pub use structure::{
    AddTrack, ImportMedia, RemoveMedia, RemoveTrack, SetProxy, SetSequenceFormat, SetTrackSwitch,
    TrackSwitch,
};
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
    let legal = |delta: i64| preview(sequence, &make(Ticks(delta))).is_ok();
    if desired == Ticks::ZERO || legal(desired.0) {
        return desired;
    }
    let (mut good, mut bad) = (0_i64, desired.0);
    while (bad - good).abs() > 1 {
        let middle = good + (bad - good) / 2;
        if legal(middle) {
            good = middle;
        } else {
            bad = middle;
        }
    }
    Ticks(good)
}
