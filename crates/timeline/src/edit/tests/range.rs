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

#[test]
fn ripple_trim_to_playhead_closes_up_to_the_nearest_cut_on_any_track() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 600, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(Razor {
            at: Ticks(200),
            target: RazorTarget::Tracks(vec![A1]),
        })
        .unwrap();

    // The cut on A1 at 200 is nearer than V1's start.
    timeline
        .apply(RippleTrimToPlayhead {
            at: Ticks(250),
            edge: Edge::Head,
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 200, 0), (200, 550, 250)]);
    assert_eq!(spans(&timeline, A1), [(0, 200, 0), (200, 550, 250)]);

    timeline
        .apply(RippleTrimToPlayhead {
            at: Ticks(500),
            edge: Edge::Tail,
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 200, 0), (200, 500, 250)]);
}

#[test]
fn trim_to_playhead_leaves_a_gap_on_each_track_under_it() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 600, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(Razor {
            at: Ticks(200),
            target: RazorTarget::Tracks(vec![A1]),
        })
        .unwrap();

    // Each track trims its own clip: V1 from 0, A1 from its cut at 200.
    timeline
        .apply(TrimToPlayhead {
            at: Ticks(250),
            edge: Edge::Head,
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1), [(250, 600, 250)]);
    assert_eq!(spans(&timeline, A1), [(0, 200, 0), (250, 600, 250)]);

    timeline
        .apply(TrimToPlayhead {
            at: Ticks(500),
            edge: Edge::Tail,
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1), [(250, 500, 250)]);
    assert_eq!(spans(&timeline, A1), [(0, 200, 0), (250, 500, 250)]);
}
