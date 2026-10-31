use super::*;
use crate::{AddTrack, Guide, GuideAxis, TrackKind};

#[test]
fn joined_edits_undo_and_redo_as_one_step() {
    let (mut timeline, _) = timeline();
    let before = timeline.sequence().clone();
    let steps = timeline.done().count();
    let guide = Guide {
        axis: GuideAxis::Vertical,
        at: 960,
    };
    let mark = timeline.edit_mark();
    timeline
        .apply(AddTrack {
            kind: TrackKind::Video,
        })
        .unwrap();
    timeline.apply(GuideEdit::Add(guide)).unwrap();
    timeline.join_since(mark);
    let after = timeline.sequence().clone();
    assert_eq!(timeline.done().count(), steps + 1);
    assert_eq!(timeline.undo_name(), Some(GuideEdit::Add(guide).name()));
    assert!(timeline.undo());
    assert_eq!(timeline.sequence(), &before);
    assert!(timeline.redo().unwrap());
    assert_eq!(timeline.sequence(), &after);
}

#[test]
fn a_join_around_an_earlier_join_keeps_older_steps_apart() {
    let (mut timeline, _) = timeline();
    let add_track = || AddTrack {
        kind: TrackKind::Video,
    };
    timeline.apply(add_track()).unwrap();
    let steps = timeline.done().count();
    let outer = timeline.edit_mark();
    let inner = timeline.edit_mark();
    timeline.apply(add_track()).unwrap();
    timeline.apply(add_track()).unwrap();
    timeline.join_since(inner);
    timeline.apply(add_track()).unwrap();
    timeline.join_since(outer);
    assert_eq!(timeline.done().count(), steps + 1);
}
