use super::*;
use crate::Marks;

#[test]
fn a_subsequence_copies_the_marked_span_from_zero_and_leaves_the_sequence_alone() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 100, 0, 500, PlaceMode::Overwrite))
        .unwrap();
    let marks = Marks::default().with_in(Ticks(200)).with_out(Ticks(400));
    timeline.apply(SetMarks { marks }).unwrap();
    let before = timeline.sequence().video.clone();

    timeline
        .apply(MakeSubsequence {
            clips: Vec::new(),
            name: "Sub".into(),
        })
        .unwrap();
    let sequence = timeline.sequence();
    assert_eq!(sequence.video, before);
    let item = sequence.media.last().unwrap();
    assert_eq!(item.duration, Ticks(200));
    let nest = item.nest.as_ref().unwrap();
    let spans: Vec<_> = nest
        .tracks()
        .flat_map(|(_, track)| &track.clips)
        .map(|clip| (clip.start.0, clip.duration.0, clip.source_in.0))
        .collect();
    assert_eq!(spans, [(0, 200, 100), (0, 200, 100)]);
    // The copies are clips of their own, still linked to each other.
    let copies: Vec<_> = nest.tracks().flat_map(|(_, track)| &track.clips).collect();
    assert!(copies
        .iter()
        .all(|copy| copy.id != clip_on(&timeline, V1, 0)));
    assert!(copies[0].link.is_some() && copies[0].link == copies[1].link);
}

#[test]
fn closing_gaps_removes_only_time_no_track_uses() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 100, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 300, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    // Picture alone on V2 fills part of the gap between them.
    timeline
        .apply(PlaceClip {
            video_track: Some(1),
            audio_track: None,
            ..place(source, 150, 0, 100, PlaceMode::Overwrite)
        })
        .unwrap();

    timeline.apply(CloseGaps).unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 100, 0), (150, 250, 0)]);
    assert_eq!(spans(&timeline, A1), [(0, 100, 0), (150, 250, 0)]);
    assert_eq!(spans(&timeline, V2), [(50, 150, 0)]);
    assert_eq!(timeline.apply(CloseGaps), Err(TimelineError::NothingToEdit));
}
