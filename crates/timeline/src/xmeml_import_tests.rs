use std::sync::Arc;

use super::*;
use crate::{ImportMedia, Marker, MarkerId, PlaceClip, PlaceMode, Timebase, Timeline};

fn spans(track: &Track) -> Vec<(i64, i64, i64)> {
    track
        .clips
        .iter()
        .map(|clip| (clip.start.0, clip.end().0, clip.source_in.0))
        .collect()
}

fn times(markers: &[Marker]) -> Vec<(i64, i64, &str, &str)> {
    markers
        .iter()
        .map(|marker| {
            let (name, comment) = (marker.name.as_str(), marker.comment.as_str());
            (marker.at.0, marker.duration.0, name, comment)
        })
        .collect()
}

/// Two clips of one file cut together on V1 and A1, at ticks 0 and 50.
fn two_clips(timebase: Timebase) -> Sequence {
    let format = VideoFormat::default();
    let mut sequence = Sequence::new(timebase, format);
    sequence.video.resize_with(1, Default::default);
    sequence.audio.resize_with(1, Default::default);
    let mut timeline = Timeline::new(sequence);
    let import = ImportMedia {
        name: "a.mp4".into(),
        path: "/media/a b.mp4".into(),
        duration: Ticks(1000),
        video: Some(format),
        has_audio: true,
        still: false,
        interpreted_rate: None,
        placed: None,
        frame_fit: None,
        timecode: None,
    };
    timeline.apply(import).unwrap();
    let source = timeline.sequence().media[0].id;
    for (at, source_in) in [(0, 0), (50, 200)] {
        let place = PlaceClip {
            source,
            source_in: Ticks(source_in),
            duration: Ticks(50),
            at: Ticks(at),
            video_track: Some(0),
            audio_track: Some(0),
            mode: PlaceMode::Overwrite,
            fill: None,
        };
        timeline.apply(place).unwrap();
    }
    timeline.sequence().clone()
}

#[test]
fn an_exported_sequence_reads_back_as_a_nest_of_the_same_cut() {
    let mut sequence = two_clips(Timebase::new(1, 25).expect("valid"));
    let dissolve = Transition::new(TransitionKind::DipToBlack, Ticks(10));
    Arc::make_mut(&mut sequence.video[0].clips[1]).transition_in = Some(dissolve);
    Arc::make_mut(&mut sequence.audio[0].clips[0]).gain = Gain::from_db(-6.0);
    for track in [&mut sequence.video[0], &mut sequence.audio[0]] {
        let clip = Arc::make_mut(&mut track.clips[1]);
        clip.speed = Speed::from_basis_points(20_000).unwrap();
        clip.reverse = true;
    }
    let marker = |at, duration, name: &str| Marker {
        id: MarkerId(900),
        at: Ticks(at),
        duration: Ticks(duration),
        name: name.into(),
        comment: "a < b".into(),
        color: Default::default(),
        kind: Default::default(),
    };
    sequence.markers = vec![marker(20, 0, "Point"), marker(60, 15, "Span")];
    Arc::make_mut(&mut sequence.video[0].clips[1]).markers = vec![marker(210, 0, "Take")];
    let mut timeline = Timeline::new(sequence.clone());

    let import = read_xmeml(&crate::to_xmeml(&sequence, "Cut")).unwrap();
    timeline.apply(import).unwrap();

    let media = &timeline.sequence().media;
    assert_eq!(media.len(), 2, "the file is reused");
    let nest = media[1].nest.as_ref().unwrap();
    assert_eq!(media[1].name, "Cut");
    assert_eq!(spans(&nest.video[0]), spans(&sequence.video[0]));
    assert_eq!(spans(&nest.audio[0]), spans(&sequence.audio[0]));
    assert_eq!(nest.video[0].clips[1].transition_in, Some(dissolve));
    assert_eq!(nest.audio[0].clips[0].gain, Gain::from_db(-6.0));
    for track in [&nest.video[0], &nest.audio[0]] {
        let clip = &track.clips[1];
        assert_eq!((clip.speed.percent(), clip.reverse), (200.0, true));
    }
    assert_eq!(
        times(&nest.markers),
        [(20, 0, "Point", "a < b"), (60, 15, "Span", "a < b")]
    );
    assert_eq!(
        times(&nest.video[0].clips[1].markers),
        [(210, 0, "Take", "a < b")]
    );
    let links: Vec<_> = [&nest.video[0], &nest.audio[0]]
        .iter()
        .map(|track| track.clips.iter().map(|clip| clip.link).collect::<Vec<_>>())
        .collect();
    assert_eq!(links[0], links[1]);
    assert_ne!(links[0][0], links[0][1]);
}

#[test]
fn a_centred_dissolve_of_an_odd_frame_count_reads_back_at_its_cut() {
    // Two ticks a frame, so the dissolve's half before the cut ends mid-frame.
    let mut sequence = two_clips(Timebase::new(1, 50).expect("valid"));
    let dissolve = Transition::new(TransitionKind::CrossDissolve, Ticks(26));
    Arc::make_mut(&mut sequence.video[0].clips[1]).transition_in = Some(dissolve);
    let mut timeline = Timeline::new(sequence.clone());

    let import = read_xmeml(&crate::to_xmeml(&sequence, "Cut")).unwrap();
    timeline.apply(import).unwrap();

    let nest = timeline.sequence().media[1].nest.as_ref().unwrap();
    let clips = &nest.video[0].clips;
    assert_eq!((clips[0].end(), clips[1].start), (Ticks(50), Ticks(50)));
}
