use super::*;

/// Three clips back to back at 0, 100 and 200 on V1 and A1, from source 0, 100 and 200.
fn three_clips() -> Timeline {
    let (mut timeline, source) = timeline();
    for at in [0, 100, 200] {
        timeline
            .apply(place(source, at, at, 100, PlaceMode::Overwrite))
            .unwrap();
    }
    timeline
}

fn moved(clip: ClipId, delta: i64, mode: MoveMode) -> MoveClips {
    MoveClips {
        clips: vec![clip],
        delta: Ticks(delta),
        video_tracks: 0,
        audio_tracks: 0,
        mode,
    }
}

#[test]
fn an_insert_move_leaves_a_gap_and_splits_and_pushes_the_destination() {
    let mut timeline = three_clips();
    let last = clip_on(&timeline, V1, 2);
    timeline.apply(moved(last, -150, MoveMode::Insert)).unwrap();
    let expected = [(0, 50, 0), (50, 150, 200), (150, 200, 50), (200, 300, 100)];
    assert_eq!(spans(&timeline, V1), expected);
    assert_eq!(spans(&timeline, A1), expected);
}

#[test]
fn a_rearrange_closes_the_gap_so_neighbours_swap_either_way() {
    let mut timeline = three_clips();
    let first = clip_on(&timeline, V1, 0);
    timeline
        .apply(moved(first, 200, MoveMode::Rearrange))
        .unwrap();
    let swapped = [(0, 100, 100), (100, 200, 0), (200, 300, 200)];
    assert_eq!(spans(&timeline, V1), swapped);
    assert_eq!(spans(&timeline, A1), swapped);
    let middle = clip_on(&timeline, V1, 1);
    timeline
        .apply(moved(middle, -100, MoveMode::Rearrange))
        .unwrap();
    let restored = [(0, 100, 0), (100, 200, 100), (200, 300, 200)];
    assert_eq!(spans(&timeline, V1), restored);
}
