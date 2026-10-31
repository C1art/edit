use super::*;

#[test]
fn frame_hold_freezes_the_tail_on_the_frame_at_the_cut_while_the_sound_plays_on() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 100, 200, 400, PlaceMode::Overwrite))
        .unwrap();
    let video = clip_on(&timeline, V1, 0);
    timeline
        .apply(AddFrameHold {
            clips: vec![video],
            at: Ticks(250),
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1), [(100, 250, 200), (250, 500, 350)]);
    assert_eq!(spans(&timeline, A1), [(100, 500, 200)]);
    let held = timeline.sequence().video[0].clips[1].clone();
    assert_eq!(held.source_time(Ticks(499)), Ticks(350));
    assert_ne!(held.link, timeline.sequence().audio[0].clips[0].link);

    // Cutting or trimming a hold keeps the same frame.
    timeline
        .apply(Razor {
            at: Ticks(400),
            target: RazorTarget::Tracks(vec![V1]),
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1)[2], (400, 500, 350));
}

#[test]
fn a_frame_hold_segment_goes_between_the_cut_pieces_and_pushes_the_rest_along() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 100, 200, 400, PlaceMode::Overwrite))
        .unwrap();
    let video = clip_on(&timeline, V1, 0);
    timeline
        .apply(InsertFrameHold {
            clips: vec![video],
            at: Ticks(250),
            length: Ticks(100),
        })
        .unwrap();
    assert_eq!(
        spans(&timeline, V1),
        [(100, 250, 200), (250, 350, 350), (350, 600, 350)]
    );
    assert_eq!(spans(&timeline, A1), [(100, 250, 200), (350, 600, 350)]);
    let sequence = timeline.sequence();
    let [before, hold, after] = [0, 1, 2].map(|index| sequence.video[0].clips[index].clone());
    assert!(hold.hold && hold.link.is_none());
    assert_eq!(hold.source_time(Ticks(349)), Ticks(350));
    assert_eq!(after.link, sequence.audio[0].clips[1].link);
    assert_ne!(before.link, after.link);
}

#[test]
fn frame_hold_options_hold_a_whole_clip_on_one_frame_and_release_it() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 100, 200, 400, PlaceMode::Overwrite))
        .unwrap();
    let video = clip_on(&timeline, V1, 0);
    let hold = |frame| SetFrameHold {
        clips: vec![video],
        frame,
    };
    timeline
        .apply(hold(Some(HoldFrame::At(Ticks(150)))))
        .unwrap();
    assert_eq!(spans(&timeline, V1), [(100, 500, 250)]);
    assert_eq!(
        timeline.apply(hold(Some(HoldFrame::At(Ticks(600))))),
        Err(TimelineError::NothingToEdit)
    );

    // Released, it plays on from the frame it held.
    timeline.apply(hold(None)).unwrap();
    let released = &timeline.sequence().video[0].clips[0];
    assert!(!released.hold);
    assert_eq!(released.source_time(Ticks(499)), Ticks(649));
    assert_eq!(spans(&timeline, A1), [(100, 500, 200)]);

    // Released near the media's end, it starts early enough to stay within it.
    timeline.apply(hold(Some(HoldFrame::Out))).unwrap();
    assert_eq!(spans(&timeline, V1), [(100, 500, 649)]);
    timeline.apply(hold(None)).unwrap();
    assert_eq!(spans(&timeline, V1), [(100, 500, 600)]);
}

#[test]
fn a_hold_made_on_a_remapped_clip_plays_on_from_its_frame_once_released() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    let video = clip_on(&timeline, V1, 0);
    timeline
        .apply(AddSpeedKeyframe {
            clip: video,
            at: Ticks(100),
        })
        .unwrap();
    timeline
        .apply(InsertFrameHold {
            clips: vec![video],
            at: Ticks(150),
            length: Ticks(50),
        })
        .unwrap();
    let hold = clip_on(&timeline, V1, 1);
    timeline
        .apply(SetFrameHold {
            clips: vec![hold],
            frame: None,
        })
        .unwrap();
    let released = timeline.sequence().clip(hold).unwrap().1;
    assert_eq!(released.source_time(released.start), Ticks(150));
    assert_eq!(released.source_out(), Ticks(200));
}
