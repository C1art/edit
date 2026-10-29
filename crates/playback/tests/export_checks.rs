// Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Exports one sequence to every format the app offers and reads each file back:
//! its length, its levels, where a one-frame flash and a one-frame beep land, and
//! that both still land together.

mod common;

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::{decode, export, gpu, Decoded, Gpu, TB};
use media::{AudioCodec, AudioSettings, DnxhrQuality, RateControl, StillFormat, VideoCodec};
use playback::{ExportSettings, VideoEncoding, VideoExport};
use timeline::{
    AddGenerated, FrameRate, Generator, PlaceClip, PlaceMode, Rgb, Sequence, Ticks, Timeline,
    VideoFormat,
};

const FRAME: i64 = 90_000 / 25;
const FRAMES: i64 = 100;
const FLASH: i64 = 50;
const SIZE: (u32, u32) = (256, 144);
const SAMPLE_RATE: u32 = 48_000;

/// Four seconds of black with one white frame and one frame of tone, both at frame 50.
fn sequence() -> Arc<Sequence> {
    let format = VideoFormat {
        width: SIZE.0,
        height: SIZE.1,
        frame_rate: FrameRate::FPS_25,
        ..VideoFormat::default()
    };
    let mut timeline = Timeline::new(Sequence::with_default_tracks(TB, format));
    let generators = [
        Generator::BlackVideo,
        Generator::ColorMatte { color: Rgb::WHITE },
        Generator::BarsAndTone,
    ];
    for generator in generators {
        let name = generator.kind_name().into();
        timeline.apply(AddGenerated { name, generator }).unwrap();
    }
    let media: Vec<_> = timeline.sequence().media.iter().map(|m| m.id).collect();
    let places = [
        (media[0], 0, FRAMES, Some(0), None),
        (media[1], FLASH, 1, Some(1), None),
        (media[2], FLASH, 1, None, Some(0)),
    ];
    for (source, at, frames, video_track, audio_track) in places {
        timeline
            .apply(PlaceClip {
                source,
                source_in: Ticks::ZERO,
                duration: Ticks(frames * FRAME),
                at: Ticks(at * FRAME),
                video_track,
                audio_track,
                mode: PlaceMode::Overwrite,
                fill: None,
            })
            .unwrap();
    }
    Arc::new(timeline.sequence().clone())
}

/// Where the content of `range` should put the flash, in frames from the file's start.
fn expected_flash(range: &Range<i64>) -> i64 {
    FLASH - range.start
}

fn check_pictures(decoded: &Decoded, range: &Range<i64>, lossy_levels: bool) -> Vec<String> {
    let mut problems = Vec::new();
    let frames = range.end - range.start;
    if decoded.pictures.len() as i64 != frames {
        problems.push(format!(
            "{} pictures, expected {frames}",
            decoded.pictures.len()
        ));
    }
    for (index, &(pts, _)) in decoded.pictures.iter().enumerate() {
        if pts != Ticks(index as i64 * FRAME) {
            problems.push(format!("picture {index} starts at {pts:?}"));
            break;
        }
    }
    let flashes = decoded.flashes();
    if flashes != [expected_flash(range) as usize] {
        problems.push(format!(
            "white on pictures {flashes:?}, expected [{}]",
            expected_flash(range)
        ));
    }
    let tolerance = if lossy_levels { 6.0 } else { 2.0 };
    let darkest = decoded
        .pictures
        .iter()
        .map(|p| p.1)
        .fold(f64::MAX, f64::min);
    let brightest = decoded.pictures.iter().map(|p| p.1).fold(0.0, f64::max);
    if darkest > tolerance {
        problems.push(format!("black reads {darkest:.1}, not 0"));
    }
    if brightest < 255.0 - tolerance {
        problems.push(format!("white reads {brightest:.1}, not 255"));
    }
    problems
}

fn check_sound(decoded: &Decoded, range: &Range<i64>, codec: AudioCodec) -> Vec<String> {
    let mut problems = Vec::new();
    let rate = f64::from(decoded.sample_rate.max(1));
    let seconds_per_frame = FRAME as f64 / 90_000.0;
    let start = decoded
        .sound_start
        .map_or(0.0, |pts| pts.0 as f64 / 90_000.0);
    if start.abs() > 0.0005 {
        problems.push(format!("sound starts at {:.4} s", start));
    }
    let length = decoded.sound.len() as f64 / rate;
    let expected = (range.end - range.start) as f64 * seconds_per_frame;
    // Decoders hand back the encoder's padding at the end of the last AAC frame; the
    // container's duration, checked below, is what players stop at.
    let padding = if codec == AudioCodec::Aac {
        1024.0 / rate
    } else {
        0.0
    };
    if !(expected - 0.001..=expected + 0.001 + padding).contains(&length) {
        problems.push(format!("{length:.4} s of sound, expected {expected:.4}"));
    }
    // MP3 declares whole frames, encoder delay included, and trims through its gapless
    // header when decoding, which the length check above covers.
    let declared = decoded
        .duration
        .map(|duration| duration.0 as f64 / 90_000.0);
    let off = declared.is_none_or(|declared| (declared - expected).abs() > 0.001);
    if off && codec != AudioCodec::Mp3 {
        problems.push(format!("declares {declared:?} s, expected {expected:.4}"));
    }
    let beep = decoded.first_sound();
    let flash = expected_flash(range) as f64 * seconds_per_frame;
    match beep {
        Some(beep) if (beep - flash).abs() > 0.0015 => problems.push(format!(
            "beep at {beep:.4} s, flash at {flash:.4} s ({:+.1} ms)",
            (beep - flash) * 1000.0
        )),
        None => problems.push("no beep".into()),
        _ => {}
    }
    problems
}

/// The color tags a player reads to show the picture as we made it.
fn check_tags(path: &Path) -> Vec<String> {
    let Ok(output) = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries"])
        .arg("stream=color_primaries,color_transfer,color_space")
        .args(["-of", "default=noprint_wrappers=1"])
        .arg(path)
        .output()
    else {
        eprintln!("ffprobe isn't installed; color tags not checked");
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .filter(|line| !line.ends_with("=bt709"))
        .map(|line| format!("tagged {line}"))
        .collect()
}

/// Set `EXPORT_CHECKS_KEEP` to keep the exported files for a look.
struct Case {
    name: String,
    extension: &'static str,
    video: Option<VideoEncoding>,
    audio: Option<AudioCodec>,
    /// Pictures that go through a palette or heavy compression come back less exact.
    lossy_levels: bool,
}

fn cases() -> Vec<Case> {
    let movie = |name: &str, extension, codec, audio, lossy_levels| Case {
        name: name.into(),
        extension,
        video: Some(VideoEncoding::Movie(codec, RateControl::Quality(18))),
        audio: Some(audio),
        lossy_levels,
    };
    let sound = |name: &str, extension, codec| Case {
        name: name.into(),
        extension,
        video: None,
        audio: Some(codec),
        lossy_levels: false,
    };
    let mut cases = vec![
        movie("H.264", "mp4", VideoCodec::H264, AudioCodec::Aac, true),
        movie("HEVC", "mp4", VideoCodec::Hevc, AudioCodec::Aac, true),
        movie(
            "ProRes 422",
            "mov",
            VideoCodec::ProRes422,
            AudioCodec::Pcm24,
            false,
        ),
        movie(
            "ProRes 422 HQ",
            "mov",
            VideoCodec::ProRes422Hq,
            AudioCodec::Pcm24,
            false,
        ),
        movie(
            "ProRes 4444",
            "mov",
            VideoCodec::ProRes4444,
            AudioCodec::Pcm24,
            false,
        ),
    ];
    cases.extend(DnxhrQuality::ALL.into_iter().map(|quality| {
        movie(
            &format!("DNxHR {quality:?}"),
            "mov",
            VideoCodec::Dnxhr(quality),
            AudioCodec::Pcm24,
            true,
        )
    }));
    cases.push(Case {
        name: "GIF".into(),
        extension: "gif",
        video: Some(VideoEncoding::Movie(
            VideoCodec::Gif,
            RateControl::Quality(0),
        )),
        audio: None,
        lossy_levels: true,
    });
    cases.extend([
        sound("AAC", "m4a", AudioCodec::Aac),
        sound("MP3", "mp3", AudioCodec::Mp3),
        sound("WAV", "wav", AudioCodec::Pcm16),
        sound("FLAC", "flac", AudioCodec::Flac),
    ]);
    cases
}

fn run_case(
    case: &Case,
    sequence: &Arc<Sequence>,
    gpu: &Gpu,
    range: &Range<i64>,
    directory: &Path,
) -> Vec<String> {
    let path = directory.join(format!(
        "{}-{}.{}",
        case.name.replace(' ', "_"),
        range.start,
        case.extension
    ));
    let whole = range.start == 0 && range.end == FRAMES;
    let settings = ExportSettings {
        path: path.clone(),
        video: case.video.map(|encoding| VideoExport {
            encoding,
            size: SIZE,
        }),
        audio: case.audio.map(|codec| AudioSettings {
            codec,
            sample_rate: SAMPLE_RATE,
            bits_per_second: 192_000,
        }),
        range: (!whole).then(|| Ticks(range.start * FRAME)..Ticks(range.end * FRAME)),
    };
    if let Err(error) = export(sequence, gpu, settings) {
        return vec![format!("export failed: {error}")];
    }
    let decoded = match decode(&path) {
        Ok(decoded) => decoded,
        Err(error) => return vec![format!("reading back failed: {error}")],
    };
    let mut problems = Vec::new();
    if case.video.is_some() {
        problems.extend(check_pictures(&decoded, range, case.lossy_levels));
        if case.extension != "gif" {
            problems.extend(check_tags(&path));
        }
    }
    if let Some(codec) = case.audio {
        problems.extend(check_sound(&decoded, range, codec));
    }
    if std::env::var_os("EXPORT_CHECKS_KEEP").is_none() {
        let _ = std::fs::remove_file(&path);
    }
    problems
}

fn still_frames(sequence: &Arc<Sequence>, gpu: &Gpu, directory: &Path) -> Vec<String> {
    let stills = directory.join("stills");
    std::fs::create_dir_all(&stills).unwrap();
    let mut problems = Vec::new();
    for format in StillFormat::ALL {
        let settings = ExportSettings {
            path: stills.join(format!("frame.{}", format.extension())),
            video: Some(VideoExport {
                encoding: VideoEncoding::Stills(format),
                size: SIZE,
            }),
            audio: None,
            range: None,
        };
        if let Err(error) = export(sequence, gpu, settings) {
            problems.push(format!("{format:?}: export failed: {error}"));
            continue;
        }
        let mut files: Vec<PathBuf> = std::fs::read_dir(&stills)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        files.sort();
        if files.len() as i64 != FRAMES {
            problems.push(format!("{format:?}: {} files", files.len()));
        }
        let white: Vec<usize> = files
            .iter()
            .enumerate()
            .filter(|(_, file)| {
                decode(file)
                    .ok()
                    .and_then(|decoded| decoded.pictures.first().map(|p| p.1 > 128.0))
                    .unwrap_or(false)
            })
            .map(|(index, _)| index)
            .collect();
        if white != [FLASH as usize] {
            problems.push(format!("{format:?}: white files {white:?}"));
        }
        for file in files {
            std::fs::remove_file(file).unwrap();
        }
    }
    problems
}

#[test]
#[ignore = "needs a GPU and encodes real files with FFmpeg"]
fn every_export_format_keeps_length_levels_and_sync() {
    let gpu = gpu();
    let directory = std::env::temp_dir().join(format!("export-checks-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let sequence = sequence();

    let mut report = Vec::new();
    for range in [0..FRAMES, 40..61] {
        for case in cases() {
            for problem in run_case(&case, &sequence, &gpu, &range, &directory) {
                report.push(format!("{} {range:?}: {problem}", case.name));
            }
        }
    }
    report.extend(still_frames(&sequence, &gpu, &directory));
    if std::env::var_os("EXPORT_CHECKS_KEEP").is_none() {
        let _ = std::fs::remove_dir_all(&directory);
    }
    assert!(report.is_empty(), "\n{}", report.join("\n"));
}
