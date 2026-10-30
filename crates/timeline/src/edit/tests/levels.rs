use super::*;
use crate::{ClipLine, Gain, NudgeClipGain, Param, SetClipLine};

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

#[test]
fn a_clip_line_sets_the_property_it_names_on_video_clips_only() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let scale = Param::new(15_000);
    let set = |clip| SetClipLine {
        clip,
        line: ClipLine::Scale,
        param: scale.clone(),
    };
    timeline.apply(set(clip_on(&timeline, V1, 0))).unwrap();
    let look = &timeline.sequence().track(V1).unwrap().clips[0].look;
    assert_eq!(
        (&look.motion.scale, look.opacity.hundredths),
        (&scale, 10_000)
    );
    assert_eq!(
        timeline.apply(set(clip_on(&timeline, V1, 0))),
        Err(TimelineError::NothingToEdit)
    );
    assert_eq!(
        timeline.apply(set(clip_on(&timeline, A1, 0))),
        Err(TimelineError::NothingToEdit)
    );
}
