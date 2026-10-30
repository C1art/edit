// Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use media::{Chroma, DecodedFrame, MediaReader, Streams};
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
            DecodedFrame::Planar(_) => panic!("planar pictures were not asked for"),
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
            DecodedFrame::Planar(_) => panic!("planar pictures were not asked for"),
        }
    }
    assert_eq!(first_video, Some(Ticks(30 * FRAME)));
    assert_eq!(first_audio, Some(target));
}

#[test]
#[ignore = "decodes a real file with FFmpeg"]
fn a_planar_picture_converts_to_the_one_the_reader_gives_as_rgba() {
    let first_picture = |planar: bool| {
        let mut reader = MediaReader::open_streams(&fixture(), TB, Streams::Video).unwrap();
        reader.set_planar(planar);
        reader.seek(Ticks(10 * FRAME)).unwrap();
        reader.next_frame().unwrap().unwrap()
    };
    let DecodedFrame::Planar(planar) = first_picture(true) else {
        panic!("expected a planar picture");
    };
    let DecodedFrame::Video(rgba) = first_picture(false) else {
        panic!("expected an RGBA picture");
    };
    let planes = planar.planes().unwrap();
    assert_eq!((planes.luma.width, planes.luma.height), (320, 240));
    // Hardware decoders hand back NV12, software ones separate planes.
    let chroma = match planes.chroma {
        Chroma::Separate(u, _) | Chroma::Interleaved(u) => u,
    };
    assert_eq!((chroma.width, chroma.height), (160, 120));
    let converted = planar.to_rgba().unwrap();
    assert_eq!(converted.pts, rgba.pts);
    assert_eq!(converted.pixels, rgba.pixels);
}

#[test]
#[ignore = "decodes a real file with FFmpeg"]
fn threads_that_converted_a_planar_picture_can_exit() {
    let converter = std::thread::spawn(|| {
        let mut reader = MediaReader::open_streams(&fixture(), TB, Streams::Video).unwrap();
        reader.set_planar(true);
        let Some(DecodedFrame::Planar(planar)) = reader.next_frame().unwrap() else {
            panic!("expected a planar picture");
        };
        planar.to_rgba().unwrap();
    });
    // Waits from another thread: one stuck exiting under the loader lock would leave
    // the joining thread stuck too.
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        converter.join().unwrap();
        std::thread::spawn(|| ()).join().unwrap();
        done.send(()).unwrap();
    });
    finished
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("a thread that converted a picture never finished exiting");
}
