//! Where the sound in a stretch of a sequence pauses for long enough to be cut out,
//! and cutting it out.

use std::ops::Range;

use super::ops::{self, merge_ranges, Relinks};
use super::range::extract;
use crate::{active_spans, Edit, FrameRate, Sequence, Ticks, Timebase, TimelineError, TrackRef};

/// Takes `pauses` out of `tracks`, either closing them up on every sync-locked track or
/// leaving gaps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovePauses {
    pub pauses: Vec<Range<Ticks>>,
    pub tracks: Vec<TrackRef>,
    pub ripple: bool,
}

impl Edit for RemovePauses {
    fn name(&self) -> &'static str {
        "Remove Pauses"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        remove_ranges(sequence, &self.pauses, &self.tracks, self.ripple)
    }
}

/// Takes the stretches where words selected in a transcript play out of `tracks`,
/// closing them up on every sync-locked track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteText {
    pub spans: Vec<Range<Ticks>>,
    pub tracks: Vec<TrackRef>,
}

impl Edit for DeleteText {
    fn name(&self) -> &'static str {
        "Delete Text"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        remove_ranges(sequence, &self.spans, &self.tracks, true)
    }
}

fn remove_ranges(
    sequence: &mut Sequence,
    ranges: &[Range<Ticks>],
    tracks: &[TrackRef],
    ripple: bool,
) -> Result<(), TimelineError> {
    if ranges.is_empty() {
        return Err(TimelineError::NothingToEdit);
    }
    let mut relinks = Relinks::default();
    // From the last, so closing one up leaves those before it where they were.
    for range in merge_ranges(ranges.to_vec()).into_iter().rev() {
        if ripple {
            extract(sequence, tracks, range)?;
        } else {
            for &track in tracks {
                ops::clear(sequence, track, range.clone(), &mut relinks)?;
            }
        }
    }
    Ok(())
}

/// How loud counts as sound, what's too short to count as a pause, and how much quiet
/// to keep beside sound so words aren't clipped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PauseRules {
    pub threshold_db: f32,
    pub shortest: Ticks,
    pub padding: Ticks,
}

/// The whole frames of `within` where `levels`, one per `window` from `start`, stay
/// under the rules' threshold for at least their shortest pause, less the padding next
/// to sound. Pauses at the ends of `within` run right to them.
pub fn pauses(
    levels: &[f32],
    (start, window): (Ticks, Ticks),
    rules: PauseRules,
    within: Vec<Range<Ticks>>,
    (rate, timebase): (FrameRate, Timebase),
) -> Vec<Range<Ticks>> {
    let sound = active_spans(levels, start, window, rules.threshold_db, rules.shortest);
    let first_frame_from = |time: Ticks| {
        let frame = rate.frame_at(time, timebase);
        let at = rate.frame_start(frame, timebase);
        if at < time {
            rate.frame_start(frame + 1, timebase)
        } else {
            at
        }
    };
    let frame_start_before =
        |time: Ticks| rate.frame_start(rate.frame_at(time, timebase), timebase);
    merge_ranges(within)
        .into_iter()
        .flat_map(|range| quiet_in(&sound, range))
        .filter(|quiet| quiet.range.end - quiet.range.start >= rules.shortest)
        .filter_map(|quiet| {
            let pad = |by_sound: bool| if by_sound { rules.padding } else { Ticks::ZERO };
            let start = first_frame_from(quiet.range.start + pad(quiet.after_sound));
            let end = frame_start_before(quiet.range.end - pad(quiet.before_sound));
            (start < end).then_some(start..end)
        })
        .collect()
}

struct Quiet {
    range: Range<Ticks>,
    after_sound: bool,
    before_sound: bool,
}

/// The parts of `range` outside the sorted `sound`.
fn quiet_in(sound: &[Range<Ticks>], range: Range<Ticks>) -> Vec<Quiet> {
    let mut quiet = Vec::new();
    let mut from = (range.start, false);
    for heard in sound
        .iter()
        .filter(|heard| heard.end > range.start && heard.start < range.end)
    {
        if heard.start > from.0 {
            quiet.push(Quiet {
                range: from.0..heard.start,
                after_sound: from.1,
                before_sound: true,
            });
        }
        from = (heard.end, true);
    }
    if from.0 < range.end {
        quiet.push(Quiet {
            range: from.0..range.end,
            after_sound: from.1,
            before_sound: false,
        });
    }
    quiet
}
