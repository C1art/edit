use super::*;
use crate::{Gain, NudgeClipGain};

#[test]
fn nudging_gain_steps_from_each_clips_own_level_and_stops_at_the_ceiling() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    let video = clip_on(&timeline, V1, 0);
    let nudge = NudgeClipGain {
        clips: vec![video],
        by: Gain::from_db(6.0),
    };
    for _ in 0..3 {
        timeline.apply(nudge.clone()).unwrap();
    }
    assert_eq!(
        timeline.sequence().track(A1).unwrap().clips[0].gain,
        Gain::MAX
    );
    assert_eq!(timeline.apply(nudge), Err(TimelineError::NothingToEdit));
}
