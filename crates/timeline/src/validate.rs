use crate::{Sequence, Ticks, TimelineError};

/// Checks the invariants every edit must preserve: on each track clips are sorted,
/// don't overlap and have positive length, every clip stays within its media, and
/// locked tracks are untouched since `before`.
pub(crate) fn check(before: &Sequence, after: &Sequence) -> Result<(), TimelineError> {
    for (track_ref, track) in after.tracks() {
        let was_locked = before
            .track(track_ref)
            .filter(|old| old.locked && track.locked);
        if was_locked.is_some_and(|old| old.clips != track.clips) {
            return Err(TimelineError::TrackLocked(track_ref));
        }
        let mut previous_end = Ticks::ZERO;
        for clip in &track.clips {
            if clip.start < Ticks::ZERO {
                return Err(TimelineError::BeforeSequenceStart);
            }
            if clip.duration <= Ticks::ZERO {
                return Err(TimelineError::NonPositiveDuration);
            }
            if clip.start < previous_end {
                return Err(TimelineError::ClipOverlap {
                    track: track_ref,
                    at: clip.start,
                });
            }
            let media = after
                .media_item(clip.source)
                .ok_or(TimelineError::MediaNotFound(clip.source))?;
            if clip.source_in < Ticks::ZERO || clip.source_out() > media.duration {
                return Err(TimelineError::OutsideMedia(clip.id));
            }
            previous_end = clip.end();
        }
    }
    Ok(())
}
