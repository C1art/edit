use super::*;

#[test]
fn synchronize_picks_each_linked_sets_sound_and_moves_the_set_together() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 200, PlaceMode::Overwrite))
        .unwrap();
    let mut second = place(source, 500, 100, 200, PlaceMode::Overwrite);
    second.video_track = Some(1);
    second.audio_track = Some(1);
    timeline.apply(second).unwrap();
    let selection = [clip_on(&timeline, V2, 0), clip_on(&timeline, V1, 0)];
    let picked = synchronized_clips(timeline.sequence(), &selection);
    assert_eq!(
        picked,
        [clip_on(&timeline, A1, 0), clip_on(&timeline, A2, 0)]
    );
    timeline
        .apply(SynchronizeClips {
            starts: vec![(picked[1], Ticks(100))],
        })
        .unwrap();
    assert_eq!(
        (spans(&timeline, V2), spans(&timeline, A2)),
        (vec![(100, 300, 100)], vec![(100, 300, 100)])
    );
    assert_eq!(
        timeline.apply(SynchronizeClips {
            starts: vec![(picked[1], Ticks(100))],
        }),
        Err(TimelineError::NothingToEdit)
    );
}
