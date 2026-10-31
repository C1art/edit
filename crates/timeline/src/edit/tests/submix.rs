use super::*;
use crate::TrackSend;

#[test]
fn deleting_a_submix_sends_its_tracks_to_the_master_and_undo_restores_the_route() {
    let (mut timeline, _) = timeline();
    timeline.apply(AddSubmix).unwrap();
    timeline.apply(AddSubmix).unwrap();
    let [first, second] = [0, 1].map(|index| timeline.sequence().submixes[index].id);
    timeline
        .apply(SetTrackOutput {
            track: A1,
            output: Some(first),
        })
        .unwrap();
    timeline
        .apply(SetTrackOutput {
            track: A2,
            output: Some(second),
        })
        .unwrap();
    let sends = vec![TrackSend::new(second), TrackSend::new(first)];
    timeline.apply(SetTrackSends { track: A1, sends }).unwrap();
    timeline.apply(RemoveSubmix { submix: first }).unwrap();

    let outputs = |timeline: &Timeline| -> Vec<_> {
        timeline
            .sequence()
            .audio
            .iter()
            .map(|track| track.output)
            .collect()
    };
    assert_eq!(outputs(&timeline), [None, Some(second)]);
    assert_eq!(timeline.sequence().audio[0].sends, [TrackSend::new(second)]);
    let unknown = SetTrackOutput {
        track: A2,
        output: Some(first),
    };
    assert_eq!(timeline.apply(unknown), Err(TimelineError::NothingToEdit));
    let unknown_send = SetTrackSends {
        track: A2,
        sends: vec![TrackSend::new(first)],
    };
    assert_eq!(
        timeline.apply(unknown_send),
        Err(TimelineError::NothingToEdit)
    );
    timeline.undo();
    assert_eq!(outputs(&timeline), [Some(first), Some(second)]);
    assert_eq!(timeline.sequence().audio[0].sends.len(), 2);
}

#[test]
fn a_nest_keeps_its_own_submixes_and_master_effects() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    timeline.apply(AddSubmix).unwrap();
    let clips = vec![clip_on(&timeline, V1, 0)];
    let name = "Nest".into();
    timeline.apply(NestClips { clips, name }).unwrap();
    let nest = timeline.sequence().media[1].id;
    assert!(timeline.set_open_nest(Some(nest)));
    assert!(timeline.sequence().submixes.is_empty());

    timeline.apply(AddSubmix).unwrap();
    let effects = vec![crate::AudioEffect::SwapChannels];
    timeline.apply(SetMasterEffects { effects }).unwrap();

    let project = timeline.project();
    let nested = project.media_item(nest).unwrap().nest.as_ref().unwrap();
    assert_eq!((nested.submixes.len(), nested.master_effects.len()), (1, 1));
    assert_eq!(project.submixes.len(), 1);
    assert!(project.master_effects.is_empty());
}
