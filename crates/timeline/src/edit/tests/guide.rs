use super::*;
use crate::{Guide, GuideAxis};

#[test]
fn guides_move_and_go_by_index_and_undo_back() {
    let (mut timeline, _) = timeline();
    let guide = Guide {
        axis: GuideAxis::Vertical,
        at: 960,
    };
    timeline.apply(GuideEdit::Add(guide)).unwrap();
    assert_eq!(
        timeline.apply(GuideEdit::Add(guide)),
        Err(TimelineError::NothingToEdit)
    );
    timeline
        .apply(GuideEdit::Move { index: 0, to: 640 })
        .unwrap();
    assert_eq!(
        timeline.apply(GuideEdit::Remove(1)),
        Err(TimelineError::NothingToEdit)
    );
    timeline.undo();
    assert_eq!(timeline.sequence().guides, vec![guide]);
    timeline.apply(GuideEdit::Clear).unwrap();
    assert!(timeline.sequence().guides.is_empty());
}
