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
