use std::ops::Range;

use crate::edit::ops::{self, Relinks};
use crate::{
    Clip, ClipId, Edit, Sequence, Ticks, TimelineError, TrackKind, Transition, TransitionKind,
};

/// Lays an audio clip out again as `pieces` of its source, in source time, played one
/// after another from where it starts, with a `crossfade` across each seam. Whatever
/// the remixed clip comes to cover on its track is overwritten. The pieces are unlinked
/// from any picture, which they no longer keep time with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemixClip {
    pub clip: ClipId,
    pub pieces: Vec<Range<Ticks>>,
    pub crossfade: Ticks,
}

impl Edit for RemixClip {
    fn name(&self) -> &'static str {
        "Remix"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let (track, clip) = sequence
            .clip(self.clip)
            .ok_or(TimelineError::ClipNotFound(self.clip))?;
        let plain = clip.rate().is_normal() && !clip.reverse && clip.remap.is_none();
        let pieces_fit = self.pieces.iter().all(|piece| piece.start < piece.end);
        if track.kind != TrackKind::Audio || !plain || self.pieces.is_empty() || !pieces_fit {
            return Err(TimelineError::NothingToEdit);
        }
        let original = clip.clone();
        let length = self
            .pieces
            .iter()
            .fold(Ticks(0), |length, piece| length + (piece.end - piece.start));
        ops::remove(sequence, &[self.clip]);
        let mut relinks = Relinks::default();
        ops::clear(
            sequence,
            track,
            original.start..original.start + length,
            &mut relinks,
        )?;
        let last = self.pieces.len() - 1;
        let mut start = original.start;
        for (index, piece) in self.pieces.iter().enumerate() {
            let seam = (self.crossfade > Ticks(0))
                .then(|| Transition::new(TransitionKind::CrossDissolve, self.crossfade));
            let laid = Clip {
                id: match index {
                    0 => original.id,
                    _ => ClipId(sequence.ids.next()),
                },
                start,
                duration: piece.end - piece.start,
                source_in: piece.start,
                link: None,
                transition_in: match index {
                    0 => original.transition_in,
                    _ => seam,
                },
                transition_out: (index == last).then_some(original.transition_out).flatten(),
                ..original.clone()
            };
            start = start + laid.duration;
            ops::insert(sequence, track, laid)?;
        }
        Ok(())
    }
}
