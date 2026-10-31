use super::*;

#[test]
fn clips_reusing_footage_are_marked_where_they_overlap_in_the_source() {
    let (mut timeline, source) = timeline();
    // The first two share source 200..400; the third shows nothing again.
    for (at, source_in, duration) in [(0, 0, 400), (1000, 200, 400), (2000, 600, 300)] {
        timeline
            .apply(place(source, at, source_in, duration, PlaceMode::Overwrite))
            .unwrap();
    }
    let ids = [0, 1, 2].map(|index| clip_on(&timeline, V1, index));
    let found = timeline.sequence().duplicate_frames();
    assert_eq!(found[&ids[0]], [Ticks(200)..Ticks(400)]);
    assert_eq!(found[&ids[1]], [Ticks(1000)..Ticks(1200)]);
    assert!(!found.contains_key(&ids[2]));
}
