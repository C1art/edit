use super::*;

#[test]
fn a_track_name_is_trimmed_cleared_when_blank_and_restored_by_undo() {
    let (mut timeline, _) = timeline();
    let rename = |name: &str| RenameTrack {
        track: A1,
        name: Some(name.into()),
    };
    timeline.apply(rename("  Dialogue ")).unwrap();
    let name = |timeline: &Timeline| timeline.sequence().audio[0].name.clone();
    assert_eq!(name(&timeline).as_deref(), Some("Dialogue"));
    assert_eq!(
        timeline.apply(rename("Dialogue")),
        Err(TimelineError::NothingToEdit)
    );
    timeline.apply(rename("   ")).unwrap();
    assert_eq!(name(&timeline), None);
    timeline.undo();
    assert_eq!(name(&timeline).as_deref(), Some("Dialogue"));
}

#[test]
fn a_ripple_leaves_tracks_without_sync_lock_alone_unless_it_edits_them() {
    let (mut timeline, source) = timeline();
    for at in [0, 300] {
        timeline
            .apply(place(source, at, 0, 100, PlaceMode::Overwrite))
            .unwrap();
    }
    timeline
        .apply(SetTrackSwitch {
            track: A1,
            switch: TrackSwitch::SyncLock,
            on: false,
        })
        .unwrap();
    let close = |track| CloseGap {
        track,
        at: Ticks(150),
    };

    timeline.apply(close(V1)).unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 100, 0), (100, 200, 0)]);
    assert_eq!(spans(&timeline, A1), [(0, 100, 0), (300, 400, 0)]);
    timeline.apply(close(A1)).unwrap();
    assert_eq!(spans(&timeline, A1), [(0, 100, 0), (100, 200, 0)]);
}
