// Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
#![allow(clippy::unwrap_used)]

//! Media that strays from the common case: every file must be refused with an error or
//! decode from start to end, and seek, without a crash, a hang or time running backwards.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use media::{DecodedFrame, MediaReader, Streams};
use timeline::{Ticks, Timebase};

const TB: Timebase = Timebase::HZ_90K;
const SECOND: i64 = 90_000;

/// What a file should give, in seconds where it has a length.
#[derive(Debug, Clone, Copy)]
enum Expect {
    Refused,
    Picture {
        width: u32,
        height: u32,
    },
    Movie {
        width: u32,
        height: u32,
        seconds: f64,
        audio: bool,
    },
    Sound {
        seconds: f64,
    },
    /// Damaged: whatever decodes before the damage is enough, as long as nothing breaks.
    Damaged,
}

const CASES: &[(&str, Expect)] = &[
    (
        "alpha_odd.png",
        Expect::Picture {
            width: 33,
            height: 21,
        },
    ),
    ("corrupt_bytes.mp4", Expect::Damaged),
    ("cover_art.mp3", Expect::Sound { seconds: 1.0 }),
    ("empty.mp4", Expect::Refused),
    (
        "gray16.png",
        Expect::Picture {
            width: 96,
            height: 54,
        },
    ),
    (
        "interlaced_tff.mp4",
        Expect::Movie {
            width: 96,
            height: 56,
            seconds: 1.0,
            audio: false,
        },
    ),
    (
        "late_audio.mp4",
        Expect::Movie {
            width: 96,
            height: 54,
            seconds: 2.0,
            audio: true,
        },
    ),
    (
        "long_gop_20s.mp4",
        Expect::Movie {
            width: 64,
            height: 36,
            seconds: 20.0,
            audio: false,
        },
    ),
    ("mono_22k.m4a", Expect::Sound { seconds: 1.0 }),
    (
        "multistream.mkv",
        Expect::Movie {
            width: 96,
            height: 54,
            seconds: 1.0,
            audio: true,
        },
    ),
    ("not_media.mp4", Expect::Refused),
    (
        "ntsc_mpeg2.mpg",
        Expect::Movie {
            width: 96,
            height: 54,
            seconds: 1.0,
            audio: true,
        },
    ),
    (
        "odd_size_17x13.mkv",
        Expect::Movie {
            width: 17,
            height: 13,
            seconds: 1.0,
            audio: false,
        },
    ),
    (
        "one_frame.mp4",
        Expect::Movie {
            width: 96,
            height: 54,
            seconds: 0.04,
            audio: false,
        },
    ),
    ("rate_11025.wav", Expect::Sound { seconds: 1.0 }),
    ("rate_96k.flac", Expect::Sound { seconds: 1.0 }),
    ("raw_no_timestamps.h264", Expect::Refused),
    (
        "rotated.mp4",
        Expect::Movie {
            width: 54,
            height: 96,
            seconds: 1.0,
            audio: false,
        },
    ),
    ("sine_44k.mp3", Expect::Sound { seconds: 1.0 }),
    ("surround51.ac3", Expect::Sound { seconds: 1.0 }),
    ("tiny_audio.wav", Expect::Sound { seconds: 0.01 }),
    ("truncated.mp4", Expect::Damaged),
    ("truncated_moov_at_end.mp4", Expect::Refused),
    (
        "ts_offset_10s.ts",
        Expect::Movie {
            width: 96,
            height: 54,
            seconds: 2.0,
            audio: true,
        },
    ),
    (
        "vfr.mp4",
        Expect::Movie {
            width: 96,
            height: 54,
            seconds: 1.9,
            audio: false,
        },
    ),
    (
        "vp9_opus.webm",
        Expect::Movie {
            width: 96,
            height: 54,
            seconds: 1.0,
            audio: true,
        },
    ),
];

#[derive(Default, Debug)]
struct Decoded {
    pictures: Vec<Ticks>,
    sizes: Vec<(u32, u32)>,
    samples: usize,
    sample_rate: u32,
}

fn decode_all(reader: &mut MediaReader) -> Result<Decoded, String> {
    let mut decoded = Decoded::default();
    while let Some(frame) = reader.next_frame().map_err(|error| error.to_string())? {
        match frame {
            DecodedFrame::Video(video) => {
                decoded.pictures.push(video.pts);
                decoded.sizes.push((video.width, video.height));
            }
            DecodedFrame::Audio(audio) => {
                decoded.samples += audio.frames();
                decoded.sample_rate = audio.sample_rate;
            }
            DecodedFrame::Planar(_) => return Err("planar pictures were not asked for".into()),
        }
    }
    Ok(decoded)
}

fn seconds(ticks: Ticks) -> f64 {
    ticks.0 as f64 / SECOND as f64
}

/// The problems with one file, if any.
fn check(path: &Path, expect: Expect) -> Vec<String> {
    let mut problems = Vec::new();
    let mut reader = match (MediaReader::open(path, TB), expect) {
        (Err(_), Expect::Refused | Expect::Damaged) => return problems,
        (Err(error), _) => return vec![format!("refused: {error}")],
        (Ok(reader), _) => reader,
    };
    let info = reader.info().clone();
    // Import refuses what can't say how long it is, as it couldn't be placed or trimmed.
    let Some(duration) = info.duration.or(info.still.then_some(Ticks::ZERO)) else {
        return match expect {
            Expect::Refused | Expect::Damaged => problems,
            _ => vec!["no duration".into()],
        };
    };
    if matches!(expect, Expect::Refused) {
        return vec!["opened".into()];
    }
    let decoded = match decode_all(&mut reader) {
        Ok(decoded) => decoded,
        Err(_) if matches!(expect, Expect::Damaged) => return problems,
        Err(error) => return vec![format!("decode failed: {error}")],
    };
    if decoded.pictures.windows(2).any(|pair| pair[1] <= pair[0]) {
        problems.push(format!("pictures out of order: {:?}", decoded.pictures));
    }
    if let Some(&first) = decoded.pictures.first() {
        if first < Ticks::ZERO || first > Ticks(SECOND / 10) {
            problems.push(format!("first picture at {:.3} s", seconds(first)));
        }
    }
    match expect {
        Expect::Picture { width, height } | Expect::Movie { width, height, .. } => {
            let size = info.video.as_ref().map(|video| (video.width, video.height));
            if size != Some((width, height)) {
                problems.push(format!("reported {size:?}, expected {width}x{height}"));
            }
            if decoded.sizes.iter().any(|&decoded| Some(decoded) != size) {
                problems.push(format!("decoded sizes {:?}", decoded.sizes.first()));
            }
            if decoded.pictures.is_empty() {
                problems.push("no pictures".into());
            }
        }
        _ => {}
    }
    if let Expect::Movie {
        seconds: length,
        audio,
        ..
    } = expect
    {
        if (seconds(duration) - length).abs() > 0.1 {
            problems.push(format!(
                "lasts {:.3} s, expected {length}",
                seconds(duration)
            ));
        }
        if let Some(&last) = decoded.pictures.last() {
            if last >= duration && duration > Ticks::ZERO {
                problems.push(format!("picture at {:.3} s, past the end", seconds(last)));
            }
        }
        if audio != info.has_audio {
            problems.push(format!("has_audio is {}", info.has_audio));
        }
    }
    if let Expect::Sound { seconds: length } = expect {
        let heard = decoded.samples as f64 / f64::from(decoded.sample_rate.max(1));
        // AAC and AC-3 pad their last frame.
        if !(length - 0.001..=length + 0.05).contains(&heard) {
            problems.push(format!("{heard:.4} s of sound, expected {length}"));
        }
        if info.video.is_some() || !decoded.pictures.is_empty() {
            problems.push("has a picture".into());
        }
    }
    if !matches!(expect, Expect::Damaged) {
        for fraction in [0.75, 0.25, 0.5, 0.0] {
            let target = Ticks((duration.0 as f64 * fraction) as i64);
            if let Err(error) = reader.seek(target) {
                problems.push(format!("seek to {:.3} s: {error}", seconds(target)));
                continue;
            }
            let picture = loop {
                match reader.next_frame() {
                    Ok(Some(DecodedFrame::Video(video))) => break Some(video.pts),
                    Ok(Some(_)) => {}
                    Ok(None) => break None,
                    Err(error) => {
                        problems.push(format!("after seek to {:.3} s: {error}", seconds(target)));
                        break None;
                    }
                }
            };
            let still = matches!(expect, Expect::Picture { .. });
            // Before the first picture, the first picture is the one to show.
            let shown = target.max(decoded.pictures.first().copied().unwrap_or(Ticks::ZERO));
            match picture {
                Some(pts) if !still && (pts > shown || target - pts > Ticks(SECOND / 5)) => {
                    problems.push(format!(
                        "seek to {:.3} s showed {:.3} s",
                        seconds(target),
                        seconds(pts)
                    ));
                }
                None if info.video.is_some() => {
                    problems.push(format!("no picture after seek to {:.3} s", seconds(target)));
                }
                _ => {}
            }
        }
    }
    problems
}

#[test]
#[ignore = "decodes real files with FFmpeg"]
fn odd_media_is_refused_or_plays() {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/odd");
    let mut report = Vec::new();
    for &(file, expect) in CASES {
        let path = folder.join(file);
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(check(&path, expect));
        });
        match receiver.recv_timeout(Duration::from_secs(60)) {
            Ok(problems) => {
                report.extend(problems.iter().map(|problem| format!("{file}: {problem}")))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => report.push(format!("{file}: hangs")),
            Err(mpsc::RecvTimeoutError::Disconnected) => report.push(format!("{file}: panicked")),
        }
    }
    assert!(report.is_empty(), "\n{}", report.join("\n"));
}

#[test]
#[ignore = "decodes a real file with FFmpeg"]
fn audio_that_starts_late_keeps_its_place() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/odd/audio_starts_late.mp4");
    let mut reader = MediaReader::open_streams(&path, TB, Streams::Audio).unwrap();
    let first_sound = |reader: &mut MediaReader| loop {
        match reader.next_frame().unwrap() {
            Some(DecodedFrame::Audio(audio)) => break audio,
            Some(_) => {}
            None => panic!("no sound"),
        }
    };
    let opening = first_sound(&mut reader);
    assert_eq!(opening.pts, Ticks::ZERO);
    assert!(opening.samples.iter().all(|&sample| sample == 0.0));
    let mut heard = opening.frames();
    while let Some(frame) = reader.next_frame().unwrap() {
        if let DecodedFrame::Audio(audio) = frame {
            heard += audio.frames();
        }
    }
    // The sound starts 0.478 s in and lasts 1.021 s.
    let seconds = heard as f64 / 48_000.0;
    assert!((1.49..1.52).contains(&seconds), "{seconds:.3} s heard");

    reader.seek(Ticks(SECOND / 4)).unwrap();
    let before = first_sound(&mut reader);
    assert_eq!(before.pts, Ticks(SECOND / 4));
    assert!(before.samples.iter().all(|&sample| sample == 0.0));

    reader.seek(Ticks(SECOND)).unwrap();
    let during = first_sound(&mut reader);
    assert_eq!(during.pts, Ticks(SECOND));
    assert!(during.samples.iter().any(|&sample| sample != 0.0));
}
