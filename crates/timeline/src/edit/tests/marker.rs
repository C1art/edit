use super::*;
use crate::{MarkerColor, Sequence, Timeline};

fn marker_times(sequence: &Sequence) -> Vec<i64> {
    sequence.markers.iter().map(|marker| marker.at.0).collect()
}

#[test]
fn markers_stay_in_time_order_through_add_move_and_undo() {
    let (mut timeline, _) = timeline();
    for at in [500, 100, 300] {
        timeline
            .apply(AddMarker {
                owner: MarkerOwner::Sequence,
                at: Ticks(at),
            })
            .unwrap();
    }
    assert_eq!(
        timeline.apply(AddMarker {
            owner: MarkerOwner::Sequence,
            at: Ticks(300)
        }),
        Err(TimelineError::NothingToEdit)
    );
    assert_eq!(marker_times(timeline.sequence()), vec![100, 300, 500]);

    let mut moved = timeline.sequence().markers[0].clone();
    moved.at = Ticks(400);
    moved.name = "Chorus".into();
    moved.color = MarkerColor::Red;
    timeline
        .apply(UpdateMarker {
            owner: MarkerOwner::Sequence,
            marker: moved.clone(),
        })
        .unwrap();
    assert_eq!(marker_times(timeline.sequence()), vec![300, 400, 500]);
    assert_eq!(timeline.sequence().markers[1], moved);
    assert_eq!(
        timeline.sequence().marker_after(Ticks(400)).map(|m| m.at),
        Some(Ticks(500))
    );
    assert_eq!(
        timeline.sequence().marker_before(Ticks(400)).map(|m| m.at),
        Some(Ticks(300))
    );

    timeline
        .apply(RemoveMarker {
            owner: MarkerOwner::Sequence,
            id: moved.id,
        })
        .unwrap();
    assert_eq!(marker_times(timeline.sequence()), vec![300, 500]);
    timeline.undo();
    timeline.undo();
    assert_eq!(marker_times(timeline.sequence()), vec![100, 300, 500]);
}

#[test]
fn clip_markers_show_only_within_the_played_source_at_their_sequence_position() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 100, 200, 300, PlaceMode::Overwrite))
        .unwrap();
    let clip = clip_on(&timeline, V1, 0);
    for at in [600, 250, 150] {
        timeline
            .apply(AddMarker {
                owner: MarkerOwner::Clip(clip),
                at: Ticks(at),
            })
            .unwrap();
    }
    let shown = |timeline: &Timeline| -> Vec<(i64, i64)> {
        let (_, clip) = timeline.sequence().clip(clip).unwrap();
        clip.shown_markers()
            .map(|(position, marker)| (position.0, marker.at.0))
            .collect()
    };
    assert_eq!(shown(&timeline), vec![(150, 250)]);
    assert!(timeline.sequence().markers.is_empty());

    timeline
        .apply(MoveClips {
            clips: vec![clip],
            delta: Ticks(50),
            video_tracks: 0,
            audio_tracks: 0,
            mode: MoveMode::Overwrite,
        })
        .unwrap();
    assert_eq!(shown(&timeline), vec![(200, 250)]);
}

#[test]
fn rippling_markers_follow_an_insert_and_go_with_a_ripple_deleted_span() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 400, PlaceMode::Overwrite))
        .unwrap();
    timeline.apply(SetRippleMarkers { ripple: true }).unwrap();
    for at in [100, 300] {
        timeline
            .apply(AddMarker {
                owner: MarkerOwner::Sequence,
                at: Ticks(at),
            })
            .unwrap();
    }
    timeline
        .apply(place(source, 200, 0, 50, PlaceMode::Insert))
        .unwrap();
    assert_eq!(marker_times(timeline.sequence()), vec![100, 350]);
    let inserted = clip_on(&timeline, V1, 1);
    timeline
        .apply(RippleDelete {
            clips: vec![clip_on(&timeline, V1, 0)],
        })
        .unwrap();
    // The first clip ran 0 to 200 once the insert split it, taking the marker at 100.
    assert_eq!(marker_times(timeline.sequence()), vec![150]);
    assert_eq!(
        timeline
            .sequence()
            .clip(inserted)
            .map(|(_, clip)| clip.start),
        Some(Ticks(0))
    );
}
