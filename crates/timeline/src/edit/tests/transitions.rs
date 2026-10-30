use super::*;
use crate::{Transition, TransitionKind};

#[test]
fn default_transitions_go_on_cuts_between_chosen_clips_and_their_free_ends() {
    let (mut timeline, source) = timeline();
    for at in [0, 300, 600] {
        timeline
            .apply(place(source, at, 0, 300, PlaceMode::Overwrite))
            .unwrap();
    }
    let [first, second] = [0, 1].map(|index| clip_on(&timeline, V1, index));
    let dissolve = Transition::new(TransitionKind::CrossDissolve, Ticks(500));
    timeline
        .apply(ApplyTransitions {
            clips: vec![first, second],
            transition: dissolve,
        })
        .unwrap();
    let edges: Vec<_> = timeline.sequence().video[0]
        .clips
        .iter()
        .map(|clip| {
            let duration = |transition: Option<Transition>| transition.map(|t| t.duration.0);
            (duration(clip.transition_in), duration(clip.transition_out))
        })
        .collect();
    // The cut into the unchosen third clip is left alone, and no transition outlasts
    // the clips it joins.
    assert_eq!(edges, [(Some(300), None), (Some(300), None), (None, None)]);
    assert!(timeline.sequence().audio[0]
        .clips
        .iter()
        .all(|clip| clip.transition_in.is_none() && clip.transition_out.is_none()));
}
