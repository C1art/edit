use super::*;

fn grouped_with(timeline: &Timeline, clip: ClipId) -> usize {
    timeline.sequence().with_groups(&[clip]).len()
}

#[test]
fn a_group_selects_its_clips_with_their_links_until_ungrouped() {
    let (mut timeline, source) = timeline();
    for at in [0, 300] {
        timeline
            .apply(place(source, at, 0, 200, PlaceMode::Overwrite))
            .unwrap();
    }
    let [first, second] = [0, 1].map(|index| clip_on(&timeline, V1, index));
    let group = |grouped| SetGrouped {
        clips: vec![first, second],
        grouped,
    };
    timeline.apply(group(true)).unwrap();
    assert_eq!(grouped_with(&timeline, first), 4);
    assert_eq!(
        timeline.apply(group(true)),
        Err(TimelineError::NothingToEdit)
    );

    let clipboard = Clipboard::copy(timeline.sequence(), &[first, second]).unwrap();
    timeline
        .apply(PasteClips {
            clipboard,
            at: Ticks(600),
            mode: PlaceMode::Overwrite,
            video_offset: 0,
            audio_offset: 0,
        })
        .unwrap();
    let pasted = clip_on(&timeline, V1, 2);
    assert_eq!(grouped_with(&timeline, pasted), 4);
    assert!(!timeline.sequence().with_groups(&[pasted]).contains(&first));

    timeline.apply(group(false)).unwrap();
    assert_eq!(grouped_with(&timeline, first), 2);
    timeline.undo();
    assert_eq!(grouped_with(&timeline, first), 4);
}
