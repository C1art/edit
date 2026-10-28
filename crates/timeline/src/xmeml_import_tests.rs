use super::*;
use crate::{ImportMedia, PlaceClip, PlaceMode, Timebase, Timeline};

fn spans(track: &Track) -> Vec<(i64, i64, i64)> {
    track
        .clips
        .iter()
        .map(|clip| (clip.start.0, clip.end().0, clip.source_in.0))
        .collect()
}

#[test]
fn an_exported_sequence_reads_back_as_a_nest_of_the_same_cut() {
    let format = VideoFormat::default();
    let mut sequence = Sequence::new(Timebase::new(1, 25).expect("valid"), format);
    sequence.video.resize_with(1, Default::default);
    sequence.audio.resize_with(1, Default::default);
    let mut timeline = Timeline::new(sequence);
    let import = ImportMedia {
        name: "a.mp4".into(),
        path: "/media/a b.mp4".into(),
        duration: Ticks(1000),
        video: Some(format),
        has_audio: true,
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
    let mut sequence = timeline.sequence().clone();
    let dissolve = Transition::new(TransitionKind::DipToBlack, Ticks(10));
    sequence.video[0].clips[1].transition_in = Some(dissolve);
    sequence.audio[0].clips[0].gain = Gain::from_db(-6.0);
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
    let links: Vec<_> = [&nest.video[0], &nest.audio[0]]
        .iter()
        .map(|track| track.clips.iter().map(|clip| clip.link).collect::<Vec<_>>())
        .collect();
    assert_eq!(links[0], links[1]);
    assert_ne!(links[0][0], links[0][1]);
}

#[test]
fn a_windows_file_url_becomes_a_drive_path() {
    let path = path_from_url("file://localhost/C%3A/Media/Day%201/take%232.mov");
    assert_eq!(path, PathBuf::from("C:/Media/Day 1/take#2.mov"));
}
