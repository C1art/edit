use super::*;

#[test]
fn renaming_a_clip_trims_the_name_and_an_empty_one_restores_the_media_name() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let clip = clip_on(&timeline, V1, 0);
    let rename = |name: &str| RenameClips {
        clips: vec![clip],
        name: name.into(),
    };
    let name = |timeline: &Timeline| timeline.sequence().clip(clip).unwrap().1.name.clone();
    timeline.apply(rename("  Interview  ")).unwrap();
    assert_eq!(name(&timeline).as_deref(), Some("Interview"));
    assert_eq!(
        timeline.apply(rename("Interview")),
        Err(TimelineError::NothingToEdit)
    );
    timeline.apply(rename(" ")).unwrap();
    assert_eq!(name(&timeline), None);
}
