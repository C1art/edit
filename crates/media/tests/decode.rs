// Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use media::{DecodedFrame, MediaReader};
use timeline::{Ticks, Timebase};

const TB: Timebase = Timebase::HZ_90K;
const FRAME: i64 = 90_000 / 25;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/testsrc_2s_25fps_44k.mp4")
}

fn decode_all(reader: &mut MediaReader) -> (Vec<Ticks>, usize) {
    let mut video_pts = Vec::new();
    let mut audio_frames = 0;
    while let Some(frame) = reader.next_frame().unwrap() {
        match frame {
            DecodedFrame::Video(video) => {
                assert_eq!(video.pixels.len(), 320 * 240 * 4);
                video_pts.push(video.pts);
            }
            DecodedFrame::Audio(audio) => audio_frames += audio.frames(),
        }
    }
    (video_pts, audio_frames)
}

#[test]
#[ignore = "decodes a real file with FFmpeg"]
fn decodes_every_frame_with_timestamps_from_zero() {
    let mut reader = MediaReader::open(&fixture(), TB).unwrap();
    assert!(reader.info().has_audio);
    let (video_pts, audio_frames) = decode_all(&mut reader);
    assert_eq!(video_pts.len(), 50);
    assert_eq!(video_pts[0], Ticks::ZERO);
    assert_eq!(video_pts[49], Ticks(49 * FRAME));
    assert!(
        (audio_frames as i64 - 88_200).abs() < 1024,
        "{audio_frames}"
    );
}

#[test]
#[ignore = "decodes a real file with FFmpeg"]
fn seek_between_frames_starts_on_the_covering_frame() {
    let mut reader = MediaReader::open(&fixture(), TB).unwrap();
    let target = Ticks(30 * FRAME + FRAME / 2);
    reader.seek(target).unwrap();
    let mut first_video = None;
    let mut first_audio = None;
    while first_video.is_none() || first_audio.is_none() {
        match reader.next_frame().unwrap().unwrap() {
            DecodedFrame::Video(video) => {
                first_video.get_or_insert(video.pts);
            }
            DecodedFrame::Audio(audio) => {
                first_audio.get_or_insert(audio.pts);
            }
        }
    }
    assert_eq!(first_video, Some(Ticks(30 * FRAME)));
    assert_eq!(first_audio, Some(target));
}
