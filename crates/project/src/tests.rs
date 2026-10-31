use std::path::PathBuf;
use std::sync::Arc;

use timeline::{
    AlphaInterpretation, AudioEffect, Bin, BinId, BinSearch, BoxStyle, Caption, Clip, ClipId,
    ColorSpace, CurvePoint, FillGradient, Gain, Generator, GradientKind, Interpolation, Keyframe,
    Marks, Mask, MaskShape, MediaItem, MediaLog, Nest, Pan, Param, PixelAspect, PluginEffect,
    PluginParam, PluginState, Rgb, Scroll, SearchField, ShadowStyle, SourceId, TextAlign,
    TextStroke, Ticks, Timebase, Title, TitleFont, TitleMotion, Track, Transcript, Transition,
    TransitionKind, VideoEffect, VideoFormat, Word,
};

use super::*;

fn sample() -> ProjectFile {
    let mut sequence = Sequence::new(Timebase::HZ_90K, VideoFormat::default());
    Arc::make_mut(&mut sequence.media).push(MediaItem {
        poster: Some(Ticks(45_000)),
        transcript: None,
        id: SourceId(1),
        name: "Interview \"A\" — café\n".to_owned(),
        path: PathBuf::from(r"C:\Footage\interview 😀.mp4"),
        duration: Ticks(900_000),
        video: Some(VideoFormat {
            pixel_aspect: PixelAspect::new(10, 11).unwrap_or_default(),
            color_space: ColorSpace::Rec2100Hlg,
            ..VideoFormat::default()
        }),
        has_audio: true,
        proxy: Some(PathBuf::from(r"C:\Footage\Proxies\interview_Proxy.mp4")),
        bin: None,
        generator: None,
        nest: None,
        subclip: None,
        interpreted_rate: None,
        interpreted_aspect: PixelAspect::new(40, 33),
        alpha: AlphaInterpretation::Invert,
        interpreted_color: Some(ColorSpace::P3D65Pq),
        interpreted_fields: None,
        log: MediaLog {
            description: "Wide, \"take 3\"".to_owned(),
            scene: "12A".to_owned(),
            shot: "4".to_owned(),
            log_note: String::new(),
            good: true,
        },
        offline: false,
        still: false,
        channel_map: None,
        placed: None,
        frame_fit: None,
        timecode: Some(90_000),
    });
    Arc::make_mut(&mut sequence.media).push(MediaItem {
        poster: None,
        transcript: None,
        id: SourceId(2),
        name: "Title 01".to_owned(),
        path: PathBuf::new(),
        duration: Ticks(i64::MAX),
        video: Some(VideoFormat::default()),
        has_audio: false,
        proxy: None,
        bin: Some(BinId(5)),
        generator: Some(Generator::Title(Title {
            text: "Line one\n«two»".to_owned(),
            size: 72,
            color: Rgb::new(250, 200, 10),
            gradient: Some(FillGradient {
                kind: GradientKind::Radial,
                to: Rgb::new(10, 20, 30),
                angle: -30,
            }),
            align: TextAlign::Right,
            font: TitleFont::Mono,
            shadow: false,
            shadow_style: Some(ShadowStyle {
                size: 3,
                ..ShadowStyle::falling(11, 4)
            }),
            background: Some(Rgb::BLACK),
            background_style: BoxStyle {
                opacity: 60,
                padding: Some(24),
                radius: 9,
            },
            bold: true,
            italic: true,
            tracking: -40,
            leading: 12,
            stroke: Some(TextStroke {
                color: Rgb::new(20, 30, 40),
                width: 6,
            }),
            motion: TitleMotion::CrawlRight(Scroll {
                start_off_screen: false,
                end_off_screen: true,
                preroll: 12,
                ease_in: 5,
                ease_out: 7,
                postroll: 3,
            }),
        })),
        nest: None,
        subclip: None,
        interpreted_rate: None,
        interpreted_aspect: None,
        interpreted_color: None,
        interpreted_fields: None,
        alpha: Default::default(),
        log: Default::default(),
        offline: false,
        still: false,
        channel_map: None,
        placed: None,
        frame_fit: None,
        timecode: None,
    });
    sequence.bins = vec![
        Bin {
            id: BinId(4),
            name: "Graphics".to_owned(),
            parent: None,
            search: None,
        },
        Bin {
            id: BinId(5),
            name: "Titles".to_owned(),
            parent: Some(BinId(4)),
            search: None,
        },
        Bin {
            id: BinId(40),
            name: "Interviews".to_owned(),
            parent: None,
            search: Some(BinSearch {
                text: "interview".to_owned(),
                field: SearchField::Name,
            }),
        },
    ];
    let mut nest = Nest::with_default_tracks();
    nest.audio[1]
        .clips
        .push(Clip::new(ClipId(8), SourceId(1), Ticks(30), Ticks(60), Ticks(0)).into());
    Arc::make_mut(&mut nest.audio[1].clips[0]).audio_effects = AudioEffect::catalog().to_vec();
    let mut drive = Param::from_value(0.25);
    drive.keys.push(Keyframe::new(Ticks(45), 1_000));
    nest.master_effects = vec![AudioEffect::Plugin(Box::new(PluginEffect {
        name: "Tape \"Saturator\"".to_owned(),
        vendor: "Acme".to_owned(),
        path: PathBuf::from(r"C:\Program Files\Common Files\VST3\Tape.vst3"),
        class_id: [0xab; 16],
        params: vec![PluginParam {
            id: 7,
            value: drive,
        }],
        state: PluginState::new(vec![0, 1, 2, 250, 255]),
    }))];
    nest.master = Gain(-150);
    nest.marks = Marks::default().with_in(Ticks(10));
    Arc::make_mut(&mut sequence.media).push(MediaItem {
        poster: None,
        transcript: Some(Arc::new(Transcript {
            language: "en".to_owned(),
            words: vec![Word {
                start: Ticks(3),
                end: Ticks(9),
                text: "Hello,".to_owned(),
            }],
        })),
        id: SourceId(3),
        name: "Nested Sequence 01".to_owned(),
        path: PathBuf::new(),
        duration: Ticks(90),
        video: Some(VideoFormat::default()),
        has_audio: true,
        proxy: None,
        bin: None,
        generator: None,
        nest: Some(nest),
        subclip: None,
        interpreted_rate: None,
        interpreted_aspect: None,
        interpreted_color: None,
        interpreted_fields: None,
        alpha: Default::default(),
        log: Default::default(),
        offline: false,
        still: false,
        channel_map: None,
        placed: None,
        frame_fit: None,
        timecode: None,
    });
    let mut clip = Clip::new(ClipId(7), SourceId(1), Ticks(0), Ticks(90_000), Ticks(3));
    clip.gain = Gain(-600);
    clip.transition_in = Some(Transition::new(TransitionKind::DipToBlack, Ticks(i64::MAX)));
    clip.look.motion.scale.keys.push(Keyframe {
        interpolation: Interpolation::EaseOut,
        ..Keyframe::new(Ticks(45_000), -12_345)
    });
    clip.look.color.midtone_wheel.blue = Param::new(-2_500);
    clip.look.color.vibrance = Param::new(4_000);
    clip.look.color.input_lut = Some(PathBuf::from(r"C:\LUTs\S-Log3 to Rec709.cube"));
    clip.look.masks.push(Mask {
        inverted: true,
        ..Mask::new(MaskShape::Rectangle)
    });
    clip.look.color.curves.blue.set(CurvePoint {
        input: 500,
        output: 620,
    });
    clip.look.effects.push(VideoEffect::GaussianBlur {
        blurriness: clip.look.opacity.clone(),
    });
    clip.look.effects.extend(VideoEffect::catalog());
    clip.look.effect_masks = vec![vec![Mask::new(MaskShape::Ellipse)]];
    sequence.video.push(Track {
        clips: vec![clip.into()],
        locked: true,
        ..Track::default()
    });
    let mut automation = Param::from_value(-6.0);
    automation.toggle_key(Ticks(30_000));
    sequence.audio.push(Track {
        pan: Pan::new(-40),
        automation,
        ..Track::default()
    });
    sequence.marks = Marks::default().with_out(Ticks(45_000));
    sequence.captions.items.push(Caption {
        start: Ticks(9_000),
        end: Ticks(90_000),
        text: "Two\nlines «quoted»".to_owned(),
    });
    sequence.captions.style.background = None;
    ProjectFile::new(sequence)
}

#[test]
fn project_round_trips_through_text() {
    let project = sample();
    let text = project.to_text().unwrap();
    assert_eq!(ProjectFile::from_text(&text).unwrap(), project);
}

#[test]
fn a_file_saved_with_a_byte_order_mark_still_reads() {
    let project = sample();
    let text = format!("\u{feff}{}", project.to_text().unwrap());
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
    assert!(matches!(
        ProjectFile::from_text(r#"{"format_version": 0}"#),
        Err(ProjectError::Invalid(_))
    ));
}

#[test]
fn overlapping_clips_are_refused() {
    let mut project = sample();
    let mut clip = Clip::clone(&project.sequence.video[0].clips[0]);
    clip.id = ClipId(8);
    clip.start = Ticks(45_000);
    project.sequence.video[0].clips.push(clip.into());
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

#[test]
fn presets_round_trip_through_text() {
    let mut blurriness = Param::from_value(4.0);
    blurriness.keys.push(Keyframe::new(Ticks(9_000), 1_200));
    let file = PresetsFile::new(vec![timeline::EffectPreset {
        name: "Soft \"look\"".to_owned(),
        video: vec![VideoEffect::GaussianBlur { blurriness }],
        video_masks: vec![vec![Mask::new(MaskShape::Rectangle)]],
        audio: vec![AudioEffect::SwapChannels],
    }]);
    let text = file.to_text().unwrap();
    assert_eq!(PresetsFile::from_text(&text).unwrap(), file);
}

#[test]
fn text_styles_round_trip_through_text() {
    let title = timeline::Title {
        font: timeline::TitleFont::File(PathBuf::from(r"C:\Fonts\Bold Face.ttf")),
        stroke: Some(timeline::TextStroke::default()),
        background: Some(timeline::Rgb::BLACK),
        tracking: -40,
        ..timeline::Title::default()
    };
    let file = TextStylesFile::new(vec![timeline::TextStyle::from_title(
        "Lower".into(),
        &title,
    )]);
    let text = file.to_text().unwrap();
    assert_eq!(TextStylesFile::from_text(&text).unwrap(), file);
}

#[test]
fn ultra_keys_saved_before_spill_suppression_keep_their_edge_desaturation() {
    let text = r#"{"format_version": 1, "presets": [{"name": "Key", "video": [
        {"UltraKey": {"color": {"r": 0, "g": 255, "b": 0},
            "tolerance": {"hundredths": 3000}, "softness": {"hundredths": 1000}}}
    ]}]}"#;
    let file = PresetsFile::from_text(text).unwrap();
    let VideoEffect::UltraKey {
        spill, pedestal, ..
    } = &file.presets[0].video[0]
    else {
        panic!("expected an Ultra Key");
    };
    assert_eq!((spill.value(), pedestal.value()), (50.0, 0.0));
}

mod damaged;
