use super::*;
use crate::Marks;

#[test]
fn extract_closes_the_marked_range_on_unlocked_tracks_and_lift_leaves_a_gap() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 600, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(SetTrackSwitch {
            track: A1,
            switch: TrackSwitch::Lock,
            on: true,
        })
        .unwrap();
    let marks = Marks::default().with_out(Ticks(300)).with_in(Ticks(100));
    timeline.apply(SetMarks { marks }).unwrap();

    timeline.apply(LiftMarked).unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 100, 0), (300, 600, 300)]);
    assert_eq!(timeline.sequence().marks, marks);
    timeline.undo();

    timeline.apply(ExtractMarked).unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 100, 0), (100, 400, 300)]);
    assert_eq!(spans(&timeline, A1), [(0, 600, 0)]);
    assert!(!timeline.sequence().marks.is_set());

    // An in mark alone runs to the end of the sequence.
    timeline
        .apply(SetMarks {
            marks: Marks::default().with_in(Ticks(350)),
        })
        .unwrap();
    timeline.apply(LiftMarked).unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 100, 0), (100, 350, 300)]);
}
