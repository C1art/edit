//! Responsive Design – Time: keyframes near a clip's start and end stay there through
//! trims, so an animation in or out keeps its timing when the clip changes length.

use serde::{Deserialize, Serialize};

use crate::{Clip, Param, Ticks};

/// How much of a clip's source, from its in point and back from its out point, holds
/// keyframes that move with that end when it's trimmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct PinnedEnds {
    pub intro: Ticks,
    pub outro: Ticks,
}

impl PinnedEnds {
    pub fn is_unpinned(&self) -> bool {
        self.intro <= Ticks::ZERO && self.outro <= Ticks::ZERO
    }
}

impl Clip {
    /// Every keyframed property of the clip, sound and picture.
    pub(crate) fn params_mut(&mut self) -> impl Iterator<Item = &mut Param> {
        self.look
            .params_mut()
            .chain([&mut self.volume, &mut self.pan])
            .chain(
                self.audio_effects
                    .iter_mut()
                    .flat_map(|effect| effect.params_mut()),
            )
    }

    /// Played backwards, held or remapped, source time doesn't follow the clip's ends.
    fn pins_follow_trims(&self) -> bool {
        !self.reverse && !self.hold && self.remap.is_none()
    }

    /// Moves the intro's keyframes with an in point that moved from `old_in`.
    pub(crate) fn carry_intro(&mut self, old_in: Ticks) {
        let by = self.source_in - old_in;
        if self.pinned.intro <= Ticks::ZERO || by == Ticks::ZERO || !self.pins_follow_trims() {
            return;
        }
        let last = old_in + self.pinned.intro;
        for param in self.params_mut() {
            carry(param, |time| time <= last, by);
        }
    }

    /// Moves the outro's keyframes with an out point that moved from `old_out`.
    pub(crate) fn carry_outro(&mut self, old_out: Ticks) {
        let by = self.source_out() - old_out;
        if self.pinned.outro <= Ticks::ZERO || by == Ticks::ZERO || !self.pins_follow_trims() {
            return;
        }
        let first = old_out - self.pinned.outro;
        for param in self.params_mut() {
            carry(param, |time| time >= first, by);
        }
    }

    /// Moves both ends' keyframes with a slip that moved the in and out points alike
    /// from `old_in`, each keyframe once even where the intro and outro overlap.
    pub(crate) fn carry_pins_through_slip(&mut self, old_in: Ticks) {
        let by = self.source_in - old_in;
        if self.pinned.is_unpinned() || by == Ticks::ZERO || !self.pins_follow_trims() {
            return;
        }
        let old_out = self.source_out() - by;
        let intro_end = (self.pinned.intro > Ticks::ZERO).then(|| old_in + self.pinned.intro);
        let outro_start = (self.pinned.outro > Ticks::ZERO).then(|| old_out - self.pinned.outro);
        let pinned = |time: Ticks| {
            intro_end.is_some_and(|end| time <= end)
                || outro_start.is_some_and(|start| time >= start)
        };
        for param in self.params_mut() {
            carry(param, pinned, by);
        }
    }
}

/// Moves the keyframes `pinned` picks by `by`; where one lands on a keyframe left in
/// place, the moved one wins.
fn carry(param: &mut Param, pinned: impl Fn(Ticks) -> bool, by: Ticks) {
    let (mut moved, mut kept): (Vec<_>, Vec<_>) =
        param.keys.drain(..).partition(|key| pinned(key.time));
    for key in &mut moved {
        key.time = (key.time + by).max(Ticks::ZERO);
    }
    kept.retain(|key| !moved.iter().any(|other| other.time == key.time));
    kept.extend(moved);
    kept.sort_by_key(|key| key.time);
    kept.dedup_by_key(|key| key.time);
    param.keys = kept;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipId, Edit, Keyframe, Razor, RazorTarget, SourceId};

    fn keyed_clip() -> Clip {
        let mut clip = Clip::new(ClipId(1), SourceId(1), Ticks(0), Ticks(1_000), Ticks(0));
        clip.look.opacity.keys = [0, 100, 500, 900, 1_000]
            .map(|time| Keyframe::new(Ticks(time), 0))
            .to_vec();
        clip.pinned = PinnedEnds {
            intro: Ticks(100),
            outro: Ticks(100),
        };
        clip
    }

    fn key_times(clip: &Clip) -> Vec<i64> {
        clip.look
            .opacity
            .keys
            .iter()
            .map(|key| key.time.0)
            .collect()
    }

    #[test]
    fn pinned_keys_follow_trimmed_ends_and_the_middle_stays() {
        let mut clip = keyed_clip();
        clip.trim_tail(Ticks(200));
        assert_eq!(key_times(&clip), [0, 100, 500, 1_100, 1_200]);
        clip.trim_head(Ticks(50));
        clip.start = Ticks(50);
        assert_eq!(key_times(&clip), [50, 150, 500, 1_100, 1_200]);
        clip.trim_tail(Ticks(-700));
        assert_eq!(key_times(&clip), [50, 150, 400, 500]);
    }

    #[test]
    fn slipping_carries_both_pinned_ends_and_leaves_the_middle() {
        let mut sequence =
            crate::Sequence::with_default_tracks(crate::Timebase::HZ_90K, Default::default());
        let clip = keyed_clip();
        let id = clip.id;
        sequence.video[0].clips.push(clip.into());
        let slip = crate::Slip {
            clips: vec![id],
            delta: Ticks(200),
        };
        slip.edit(&mut sequence).unwrap();
        let slipped = &sequence.video[0].clips[0];
        assert_eq!(key_times(slipped), [200, 300, 500, 1_100, 1_200]);
    }

    #[test]
    fn splitting_leaves_every_keyframe_in_place() {
        let mut sequence =
            crate::Sequence::with_default_tracks(crate::Timebase::HZ_90K, Default::default());
        let track = crate::TrackRef::video(0);
        sequence.video[0].clips.push(keyed_clip().into());
        let razor = Razor {
            at: Ticks(500),
            target: RazorTarget::Tracks(vec![track]),
        };
        razor.edit(&mut sequence).unwrap();
        let [left, right] = [0, 1].map(|index| &sequence.video[0].clips[index]);
        assert_eq!(key_times(left), [0, 100, 500, 900, 1_000]);
        assert_eq!(key_times(right), key_times(left));
        assert_eq!(left.pinned.outro, Ticks::ZERO);
        assert_eq!(right.pinned.intro, Ticks::ZERO);
    }
}
