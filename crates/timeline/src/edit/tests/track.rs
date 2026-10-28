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
