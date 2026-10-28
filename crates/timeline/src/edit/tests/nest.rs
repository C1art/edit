use super::*;

#[test]
fn nesting_moves_clips_into_a_sequence_that_edits_and_undoes_in_place() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    let video_only = |at, track| PlaceClip {
        video_track: Some(track),
        audio_track: None,
        ..place(source, at, 0, 100, PlaceMode::Overwrite)
    };
    timeline.apply(video_only(400, 1)).unwrap();
    let clips = vec![clip_on(&timeline, V1, 0), clip_on(&timeline, V2, 0)];
    timeline
        .apply(NestClips {
            clips,
            name: "Nest".into(),
        })
        .unwrap();

    assert_eq!(spans(&timeline, V1), [(0, 500, 0)]);
    assert_eq!(spans(&timeline, A1), [(0, 500, 0)]);
    assert!(spans(&timeline, V2).is_empty());
    let nest = timeline.sequence().media[1].id;
    assert!(timeline.set_open_nest(Some(nest)));
    assert_eq!(spans(&timeline, V1), [(0, 300, 0)]);
    assert_eq!(spans(&timeline, V2), [(400, 500, 0)]);
    assert_eq!(spans(&timeline, A1), [(0, 300, 0)]);

    // Edits act on the open nest, which grows to hold them.
    timeline.apply(video_only(600, 0)).unwrap();
    assert_eq!(
        timeline.project().media_item(nest).unwrap().duration,
        Ticks(700)
    );
    let into_itself = PlaceClip {
        audio_track: None,
        ..place(nest, 800, 0, 100, PlaceMode::Overwrite)
    };
    assert_eq!(
        timeline.apply(into_itself),
        Err(TimelineError::NestCycle(nest))
    );

    timeline.undo();
    timeline.undo();
    assert_eq!(timeline.open_nest(), None);
    assert_eq!(spans(&timeline, V1), [(0, 300, 0)]);
    assert_eq!(spans(&timeline, V2), [(400, 500, 0)]);
}

#[test]
fn a_duplicated_sequence_gives_its_clips_and_links_new_ids() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    let clips = vec![clip_on(&timeline, V1, 0), clip_on(&timeline, A1, 0)];
    let name = "Nest".into();
    timeline.apply(NestClips { clips, name }).unwrap();
    let nest = timeline.sequence().media[1].id;
    timeline.apply(DuplicateMedia { source: nest }).unwrap();

    let contents = |index: usize| {
        let item = &timeline.project().media[index];
        let nest = item.nest.as_ref().unwrap();
        let clips: Vec<_> = nest.tracks().flat_map(|(_, track)| &track.clips).collect();
        clips
            .iter()
            .map(|clip| (clip.id, clip.link))
            .collect::<Vec<_>>()
    };
    let (original, copy) = (contents(1), contents(2));
    assert_eq!(copy.len(), 2);
    assert!(copy.iter().all(|(id, link)| original
        .iter()
        .all(|(other, other_link)| id != other && link != other_link)));
    assert!(copy[0].1.is_some() && copy[0].1 == copy[1].1);
}
