use crate::{Sequence, Ticks, TimelineError};

/// Checks the invariants every edit must preserve: on each track clips are sorted,
/// don't overlap and have positive length, every clip stays within its media, and
/// locked tracks are untouched since `before`. Markers are in time order.
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
    let markers = &after.markers;
    if markers
        .first()
        .is_some_and(|marker| marker.at < Ticks::ZERO)
    {
        return Err(TimelineError::BeforeSequenceStart);
    }
    if markers.windows(2).any(|pair| pair[0].at > pair[1].at) {
        return Err(TimelineError::MarkersOutOfOrder);
    }
    Ok(())
}

impl Sequence {
    /// Checks a sequence that didn't come from edits, such as one read from a file.
    pub fn validate(&self) -> Result<(), TimelineError> {
        check(self, self)
    }
}
