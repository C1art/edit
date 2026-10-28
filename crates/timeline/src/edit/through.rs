use std::collections::HashSet;

use crate::{Clip, ClipId, Edit, Sequence, TimelineError};

/// Whether `right` carries on exactly where `left` stops in the same media at the same
/// speed, as a razor cut leaves them: Premiere's through edit, which can be joined.
pub fn is_through_edit(left: &Clip, right: &Clip) -> bool {
    let plain = |clip: &Clip| !clip.reverse && !clip.hold && clip.remap.is_none();
    left.end() == right.start
        && left.source == right.source
        && left.angle == right.angle
        && left.speed == right.speed
        && left.conform == right.conform
        && plain(left)
        && plain(right)
        && right.transition_in.is_none()
        && left.source_out() == right.source_in
}

/// Joins the selected clips (and their linked clips) across the through edits at either
/// end, each run of pieces becoming its first with the length of them all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinThroughEdits {
    pub clips: Vec<ClipId>,
}

impl Edit for JoinThroughEdits {
    fn name(&self) -> &'static str {
        "Join Through Edits"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let chosen: HashSet<ClipId> = sequence.with_links(&self.clips).into_iter().collect();
        let mut joined = false;
        for (_, track) in sequence.tracks_mut() {
            let mut index = 1;
            while index < track.clips.len() {
                let (left, right) = (&track.clips[index - 1], &track.clips[index]);
                let touched = chosen.contains(&left.id) || chosen.contains(&right.id);
                if !(touched && is_through_edit(left, right)) {
                    index += 1;
                    continue;
                }
                let right = track.clips.remove(index);
                let left = &mut track.clips[index - 1];
                left.duration = left.duration + right.duration;
                left.transition_out = right.transition_out;
                left.markers.extend(right.markers);
                joined = true;
            }
        }
        if joined {
            Ok(())
        } else {
            Err(TimelineError::NothingToEdit)
        }
    }
}
