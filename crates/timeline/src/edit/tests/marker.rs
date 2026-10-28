use super::*;
use crate::{MarkerColor, Sequence};

fn marker_times(sequence: &Sequence) -> Vec<i64> {
    sequence.markers.iter().map(|marker| marker.at.0).collect()
}

#[test]
fn markers_stay_in_time_order_through_add_move_and_undo() {
    let (mut timeline, _) = timeline();
    for at in [500, 100, 300] {
        timeline.apply(AddMarker { at: Ticks(at) }).unwrap();
    }
    assert_eq!(
        timeline.apply(AddMarker { at: Ticks(300) }),
        Err(TimelineError::NothingToEdit)
    );
    assert_eq!(marker_times(timeline.sequence()), vec![100, 300, 500]);

    let mut moved = timeline.sequence().markers[0].clone();
    moved.at = Ticks(400);
    moved.name = "Chorus".into();
    moved.color = MarkerColor::Red;
    timeline
        .apply(UpdateMarker {
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

    timeline.apply(RemoveMarker { id: moved.id }).unwrap();
    assert_eq!(marker_times(timeline.sequence()), vec![300, 500]);
    timeline.undo();
    timeline.undo();
    assert_eq!(marker_times(timeline.sequence()), vec![100, 300, 500]);
}
