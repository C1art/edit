use crate::{Captions, Marker, Marks, Nest, Sequence, Ticks, TimelineError, Track, TrackRef};

/// Years at any timebase, yet far enough from `i64::MAX` that times can be added to one
/// another and scaled by the fastest speed without overflowing.
const LATEST: Ticks = Ticks(1 << 52);

/// Checks the invariants every edit must preserve: on each track clips are sorted,
/// don't overlap and have positive length, as do their transitions, every clip stays
/// within its media, and
/// locked tracks are untouched since `before`. Markers are in time order, and no nested
/// sequence contains itself. Captions are in order and don't overlap. No time is past
/// [`LATEST`]. Nested sequences are held to the same rules.
pub(crate) fn check(before: &Sequence, after: &Sequence) -> Result<(), TimelineError> {
    check_tracks(after, before.tracks(), after.tracks())?;
    check_markers(&after.markers)?;
    check_marks(after.marks)?;
    check_captions(&after.captions)?;
    for item in &after.media {
        let Some(nest) = &item.nest else {
            continue;
        };
        let old = before.media_item(item.id).and_then(|old| old.nest.as_ref());
        check_tracks(after, old.into_iter().flat_map(Nest::tracks), nest.tracks())?;
        check_markers(&nest.markers)?;
        check_marks(nest.marks)?;
        check_captions(&nest.captions)?;
        if after.nest_contains_itself(item.id) {
            return Err(TimelineError::NestCycle(item.id));
        }
    }
    Ok(())
}

/// `media` resolves the clips' sources.
fn check_tracks<'a>(
    media: &Sequence,
    before: impl Iterator<Item = (TrackRef, &'a Track)>,
    after: impl Iterator<Item = (TrackRef, &'a Track)>,
) -> Result<(), TimelineError> {
    let before: Vec<(TrackRef, &Track)> = before.collect();
    for (track_ref, track) in after {
        let was_locked = before
            .iter()
            .find(|(old_ref, old)| *old_ref == track_ref && old.locked && track.locked);
        if was_locked.is_some_and(|(_, old)| old.clips != track.clips) {
            return Err(TimelineError::TrackLocked(track_ref));
        }
        let mut previous_end = Ticks::ZERO;
        for clip in &track.clips {
            if clip.start < Ticks::ZERO {
                return Err(TimelineError::BeforeSequenceStart);
            }
            if [clip.start, clip.duration, clip.source_in]
                .iter()
                .any(|&time| time > LATEST)
            {
                return Err(TimelineError::TooLate);
            }
            let transitions = [clip.transition_in, clip.transition_out];
            if clip.duration <= Ticks::ZERO
                || transitions
                    .iter()
                    .flatten()
                    .any(|transition| transition.duration <= Ticks::ZERO)
            {
                return Err(TimelineError::NonPositiveDuration);
            }
            check_markers(&clip.markers)?;
            if clip.start < previous_end {
                return Err(TimelineError::ClipOverlap {
                    track: track_ref,
                    at: clip.start,
                });
            }
            let item = media
                .media_item(clip.source)
                .ok_or(TimelineError::MediaNotFound(clip.source))?;
            if let Some(unrendered) = clip.unrendered {
                media
                    .media_item(unrendered)
                    .ok_or(TimelineError::MediaNotFound(unrendered))?;
            }
            if !clip.speed.is_in_range() || clip.remap.as_ref().is_some_and(|r| !r.is_valid()) {
                return Err(TimelineError::SpeedOutOfRange(clip.id));
            }
            if clip.source_in < Ticks::ZERO || clip.source_out() > item.duration {
                return Err(TimelineError::OutsideMedia(clip.id));
            }
            previous_end = clip.end();
        }
    }
    Ok(())
}

fn check_markers(markers: &[Marker]) -> Result<(), TimelineError> {
    if markers
        .iter()
        .any(|marker| marker.at > LATEST || !(Ticks::ZERO..=LATEST).contains(&marker.duration))
    {
        return Err(TimelineError::TooLate);
    }
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

fn check_marks(marks: Marks) -> Result<(), TimelineError> {
    match [marks.mark_in, marks.mark_out]
        .into_iter()
        .flatten()
        .any(|at| at > LATEST)
    {
        true => Err(TimelineError::TooLate),
        false => Ok(()),
    }
}

fn check_captions(captions: &Captions) -> Result<(), TimelineError> {
    if captions.items.iter().any(|caption| caption.end > LATEST) {
        return Err(TimelineError::TooLate);
    }
    captions.validate()
}

impl Sequence {
    /// Checks a sequence that didn't come from edits, such as one read from a file.
    pub fn validate(&self) -> Result<(), TimelineError> {
        check(self, self)
    }
}
