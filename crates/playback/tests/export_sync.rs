// Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Cuts a real file whose flash and beep share a moment into a sequence twice, from
//! different points, and checks the export keeps each pair together through the
//! decoder's seeks, the AAC priming and the 44.1 kHz to 48 kHz resampling.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use common::{decode, export, gpu, TB};
use media::{Acceleration, AudioCodec, AudioSettings, MediaReader, RateControl, VideoCodec};
use playback::{ExportSettings, VideoEncoding, VideoExport};
use timeline::{
    FrameRate, ImportMedia, PlaceClip, PlaceMode, Sequence, Ticks, Timeline, VideoFormat,
};

const FRAME: i64 = 90_000 / 25;
/// Where the fixture's flash and beep are, in frames.
const FLASH: i64 = 25;

/// The fixture from its frame 10 at frame 30, then from its frame 20 straight after.
fn sequence() -> (Arc<Sequence>, [i64; 2]) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/flash_beep_1s_25fps_44k.mp4");
    let info = MediaReader::open(&path, TB).unwrap().info().clone();
    let picture = info.video.unwrap();
    let format = VideoFormat {
        width: picture.width,
        height: picture.height,
        frame_rate: FrameRate::FPS_25,
        ..VideoFormat::default()
    };
    let mut timeline = Timeline::new(Sequence::with_default_tracks(TB, format));
    timeline
        .apply(ImportMedia {
            name: "flash".into(),
            path,
            duration: info.duration.unwrap(),
            video: Some(format),
            has_audio: true,
            still: false,
            interpreted_rate: None,
            placed: None,
            frame_fit: None,
            timecode: None,
        })
        .unwrap();
    let source = timeline.sequence().media[0].id;
    let mut flashes = [0; 2];
    let mut at = 30;
    for (flash, source_in) in flashes.iter_mut().zip([10, 20]) {
        let frames = 45 - source_in;
        timeline
            .apply(PlaceClip {
                source,
                source_in: Ticks(source_in * FRAME),
                duration: Ticks(frames * FRAME),
                at: Ticks(at * FRAME),
                video_track: Some(0),
                audio_track: Some(0),
                mode: PlaceMode::Overwrite,
                fill: None,
            })
            .unwrap();
        *flash = at + FLASH - source_in;
        at += frames;
    }
    (Arc::new(timeline.sequence().clone()), flashes)
}

#[test]
#[ignore = "needs a GPU and decodes and encodes real files with FFmpeg"]
fn cut_media_keeps_its_picture_and_sound_together() {
    let (sequence, flashes) = sequence();
    let gpu = gpu();
    let size = (sequence.format.width, sequence.format.height);
    let mut report = Vec::new();
    let formats = [
        ("mp4", VideoCodec::H264, AudioCodec::Aac),
        ("mov", VideoCodec::ProRes422, AudioCodec::Pcm24),
    ];
    for (extension, video, audio) in formats {
        let path = std::env::temp_dir().join(format!(
            "export-sync-{}-{video:?}.{extension}",
            std::process::id()
        ));
        let settings = ExportSettings {
            path: path.clone(),
            video: Some(VideoExport {
                encoding: VideoEncoding::Movie(video, RateControl::Quality(18)),
                size,
                acceleration: Acceleration::Software,
            }),
            audio: Some(AudioSettings {
                codec: audio,
                sample_rate: 48_000,
                bits_per_second: 192_000,
            }),
            range: None,
            previews: Vec::new(),
            loudness: None,
        };
        export(&sequence, &gpu, settings).unwrap();
        let decoded = decode(&path).unwrap();
        std::fs::remove_file(&path).unwrap();

        let found = decoded.flashes();
        let wanted: Vec<usize> = flashes.iter().map(|&flash| flash as usize).collect();
        if found != wanted {
            report.push(format!(
                "{video:?}: white on {found:?}, expected {wanted:?}"
            ));
        }
        for &flash in &flashes {
            let seconds = flash as f64 * FRAME as f64 / 90_000.0;
            let from = ((seconds - 0.1) * f64::from(decoded.sample_rate)) as usize;
            let window = common::Decoded {
                sound: decoded.sound.get(from..).unwrap_or_default().to_vec(),
                sound_start: Some(Ticks(0)),
                sample_rate: decoded.sample_rate,
                ..common::Decoded::default()
            };
            let beep = window.first_sound().map(|offset| seconds - 0.1 + offset);
            match beep {
                Some(beep) if (beep - seconds).abs() <= 0.0015 => {}
                _ => report.push(format!(
                    "{video:?}: beep at {beep:?} s for the flash at {seconds:.4} s"
                )),
            }
        }
    }
    assert!(report.is_empty(), "\n{}", report.join("\n"));
}
