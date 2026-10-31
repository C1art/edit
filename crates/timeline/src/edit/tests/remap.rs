use super::*;
use crate::Speed;

fn speed(percent: u64) -> Speed {
    Speed::from_basis_points(percent * 100).unwrap()
}

#[test]
fn a_faster_segment_shortens_the_clip_and_a_cut_keeps_the_frame_on_either_side() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 100, 400, PlaceMode::Overwrite))
        .unwrap();
    let video = clip_on(&timeline, V1, 0);
    timeline
        .apply(AddSpeedKeyframe {
            clip: video,
            at: Ticks(200),
        })
        .unwrap();
    timeline
        .apply(SetSegmentSpeed {
            clip: video,
            index: 0,
            speed: speed(200),
        })
        .unwrap();
    // The keyframe stays on source 300, now reached in half the time.
    assert_eq!(spans(&timeline, V1), [(0, 300, 100)]);
    // The linked sound speeds up with it, so it stays in sync.
    assert_eq!(spans(&timeline, A1), [(0, 300, 100)]);
    let on = |timeline: &Timeline, track, index| {
        let id = clip_on(timeline, track, index);
        timeline.sequence().clip(id).unwrap().1.clone()
    };
    let clip = |timeline: &Timeline, index| on(timeline, V1, index);
    assert_eq!(clip(&timeline, 0).source_time(Ticks(150)), Ticks(350));
    assert_eq!(on(&timeline, A1, 0).source_time(Ticks(150)), Ticks(350));
    assert_eq!(clip(&timeline, 0).source_out(), Ticks(500));

    timeline
        .apply(Razor {
            at: Ticks(50),
            target: RazorTarget::Tracks(vec![V1]),
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 50, 100), (50, 300, 200)]);
    let right = clip(&timeline, 1);
    assert_eq!(right.source_time(Ticks(150)), Ticks(350));
    assert_eq!(right.position_of(Ticks(300)), Ticks(100));

    timeline.apply(RemoveTimeRemap { clip: right.id }).unwrap();
    let right = clip(&timeline, 1);
    assert_eq!((right.remap.clone(), right.speed), (None, speed(120)));
    assert_eq!(right.source_out(), Ticks(500));
    // Cut on its own, the picture no longer plays in step with the sound under it.
    assert!(on(&timeline, A1, 0).remap.is_some());
}

#[test]
fn a_slower_segment_grows_the_clip_only_up_to_the_next_one() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 200, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 300, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let video = clip_on(&timeline, V1, 0);
    timeline
        .apply(AddSpeedKeyframe {
            clip: video,
            at: Ticks(100),
        })
        .unwrap();
    timeline
        .apply(SetSegmentSpeed {
            clip: video,
            index: 1,
            speed: speed(25),
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 300, 0), (300, 400, 0)]);
    assert_eq!(spans(&timeline, A1), [(0, 300, 0), (300, 400, 0)]);

    timeline
        .apply(RemoveSpeedKeyframe {
            clip: video,
            offset: Ticks(100),
        })
        .unwrap();
    // One segment from source 0 to 200 over 500 ticks, cut short at the next clip.
    let clip = timeline.sequence().clip(video).unwrap().1;
    assert_eq!(clip.source_time(Ticks(250)), Ticks(100));
    assert_eq!(spans(&timeline, V1), [(0, 300, 0), (300, 400, 0)]);
}
