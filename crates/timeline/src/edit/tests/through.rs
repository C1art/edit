use super::*;

#[test]
fn joining_a_razor_cut_restores_one_clip_on_both_linked_tracks_but_not_a_jump_cut() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 300, 500, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(Razor {
            at: Ticks(100),
            target: RazorTarget::Clips(vec![clip_on(&timeline, V1, 0)]),
        })
        .unwrap();
    let pieces = timeline.sequence().track(V1).unwrap().clips.clone();
    assert!(is_through_edit(&pieces[0], &pieces[1]));
    assert!(!is_through_edit(&pieces[1], &pieces[2]));
    timeline
        .apply(JoinThroughEdits {
            clips: vec![pieces[1].id, pieces[2].id],
        })
        .unwrap();
    let joined = [(0, 300, 0), (300, 400, 500)];
    assert_eq!(spans(&timeline, V1), joined);
    assert_eq!(spans(&timeline, A1), joined);
}
