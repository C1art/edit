use super::*;

#[test]
fn remix_lays_the_music_out_as_its_pieces_with_crossfades_and_undoes() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 100, 200, 400, PlaceMode::Overwrite))
        .unwrap();
    let music = clip_on(&timeline, A1, 0);
    let before = timeline.sequence().clone();
    timeline
        .apply(RemixClip {
            clip: music,
            pieces: vec![Ticks(0)..Ticks(300), Ticks(600)..Ticks(800)],
            crossfade: Ticks(10),
        })
        .unwrap();
    assert_eq!(spans(&timeline, A1), [(100, 400, 0), (400, 600, 600)]);
    let laid = &timeline.sequence().audio[0].clips;
    assert_eq!(laid[0].transition_in, None);
    assert_eq!(
        laid[1].transition_in.as_ref().map(|seam| seam.duration),
        Some(Ticks(10))
    );
    assert!(laid.iter().all(|piece| piece.link.is_none()));
    assert_eq!(spans(&timeline, V1), [(100, 500, 200)]);

    assert!(timeline.undo());
    assert_eq!(timeline.sequence(), &before);
}
