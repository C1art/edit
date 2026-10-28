use std::path::PathBuf;

use timeline::{
    AudioEffect, Clip, ClipId, Gain, Generator, Interpolation, Keyframe, Marks, MediaItem, Nest,
    Pan, Rgb, SourceId, TextAlign, Ticks, Timebase, Title, TitleFont, Track, Transition,
    TransitionKind, VideoEffect, VideoFormat,
};

use super::*;

fn sample() -> ProjectFile {
    let mut sequence = Sequence::new(Timebase::HZ_90K, VideoFormat::default());
    sequence.media.push(MediaItem {
        id: SourceId(1),
        name: "Interview \"A\" — café\n".to_owned(),
        path: PathBuf::from(r"C:\Footage\interview 😀.mp4"),
        duration: Ticks(900_000),
        video: Some(VideoFormat::default()),
        has_audio: true,
        proxy: Some(PathBuf::from(r"C:\Footage\Proxies\interview_Proxy.mp4")),
        generator: None,
        nest: None,
    });
    sequence.media.push(MediaItem {
        id: SourceId(2),
        name: "Title 01".to_owned(),
        path: PathBuf::new(),
        duration: Ticks(i64::MAX),
        video: Some(VideoFormat::default()),
        has_audio: false,
        proxy: None,
        generator: Some(Generator::Title(Title {
            text: "Line one\n«two»".to_owned(),
            size: 72,
            color: Rgb::new(250, 200, 10),
            align: TextAlign::Right,
            font: TitleFont::Mono,
            shadow: false,
            background: Some(Rgb::BLACK),
        })),
        nest: None,
    });
    let mut nest = Nest::with_default_tracks();
    nest.audio[1].clips.push(Clip::new(
        ClipId(8),
        SourceId(1),
        Ticks(30),
        Ticks(60),
        Ticks(0),
    ));
    nest.audio[1].clips[0].audio_effects = AudioEffect::catalog().to_vec();
    nest.master = Gain(-150);
    nest.marks = Marks::default().with_in(Ticks(10));
    sequence.media.push(MediaItem {
        id: SourceId(3),
        name: "Nested Sequence 01".to_owned(),
        path: PathBuf::new(),
        duration: Ticks(90),
        video: Some(VideoFormat::default()),
        has_audio: true,
        proxy: None,
        generator: None,
        nest: Some(nest),
    });
    let mut clip = Clip::new(ClipId(7), SourceId(1), Ticks(0), Ticks(90_000), Ticks(3));
    clip.gain = Gain(-600);
    clip.transition_in = Some(Transition {
        kind: TransitionKind::DipToBlack,
        duration: Ticks(i64::MAX),
    });
    clip.look.motion.scale.keys.push(Keyframe {
        interpolation: Interpolation::EaseOut,
        ..Keyframe::new(Ticks(45_000), -12_345)
    });
    clip.look.effects.push(VideoEffect::GaussianBlur {
        blurriness: clip.look.opacity.clone(),
    });
    clip.look.effects.extend(VideoEffect::catalog());
    sequence.video.push(Track {
        clips: vec![clip],
        locked: true,
        ..Track::default()
    });
    sequence.audio.push(Track {
        pan: Pan::new(-40),
        ..Track::default()
    });
    sequence.marks = Marks::default().with_out(Ticks(45_000));
    ProjectFile::new(sequence)
}

#[test]
fn project_round_trips_through_text() {
    let project = sample();
    let text = project.to_text().unwrap();
    assert_eq!(ProjectFile::from_text(&text).unwrap(), project);
}

#[test]
fn fields_added_later_default_when_missing() {
    let project = ProjectFile::new(Sequence::new(Timebase::HZ_90K, VideoFormat::default()));
    let text = format!(
        r#"{{"format_version": 1, "sequence": {{"timebase": {}}}}}"#,
        json::to_value(&project.sequence.timebase)
            .unwrap()
            .to_text()
    );
    assert_eq!(ProjectFile::from_text(&text).unwrap(), project);
}

#[test]
fn newer_or_unversioned_files_are_refused() {
    let newer = format!(r#"{{"format_version": {}}}"#, FORMAT_VERSION + 1);
    assert!(matches!(
        ProjectFile::from_text(&newer),
        Err(ProjectError::UnsupportedVersion(v)) if v == FORMAT_VERSION + 1
    ));
    assert!(matches!(
        ProjectFile::from_text(r#"{"sequence": {}}"#),
        Err(ProjectError::MissingVersion)
    ));
}

#[test]
fn overlapping_clips_are_refused() {
    let mut project = sample();
    let mut clip = project.sequence.video[0].clips[0].clone();
    clip.id = ClipId(8);
    clip.start = Ticks(45_000);
    project.sequence.video[0].clips.push(clip);
    let text = project.to_text().unwrap();
    assert!(matches!(
        ProjectFile::from_text(&text),
        Err(ProjectError::Invalid(_))
    ));
}

#[test]
fn damaged_text_reports_the_line() {
    let error =
        ProjectFile::from_text("{\n  \"format_version\": 1,\n  \"sequence\": [\n").unwrap_err();
    assert!(
        matches!(error, ProjectError::Syntax { line: 4, .. }),
        "{error}"
    );
    assert!(Value::parse("[1.5]").is_err());
    assert!(Value::parse(&"[".repeat(10_000)).is_err());
}

#[test]
fn escapes_decode_including_surrogate_pairs() {
    let value = Value::parse(r#""a\"\\\n\u00e9\ud83d\ude00""#).unwrap();
    assert_eq!(value, Value::String("a\"\\\né😀".to_owned()));
}
