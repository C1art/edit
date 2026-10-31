// Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Its own test binary: `stop_exports` reaches every export in the process.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{gpu, TB};
use media::{Acceleration, RateControl, VideoCodec};
use playback::{ExportEvent, ExportJob, ExportSettings, VideoEncoding, VideoExport};
use timeline::{
    AddGenerated, FrameRate, Generator, PlaceClip, PlaceMode, Sequence, Ticks, Timeline,
    VideoFormat,
};

const FRAME: i64 = 90_000 / 25;

fn ten_minutes_of_black() -> Arc<Sequence> {
    let format = VideoFormat {
        width: 640,
        height: 360,
        frame_rate: FrameRate::FPS_25,
        ..VideoFormat::default()
    };
    let mut timeline = Timeline::new(Sequence::with_default_tracks(TB, format));
    timeline
        .apply(AddGenerated {
            name: "Black".into(),
            generator: Generator::BlackVideo,
            placed: None,
        })
        .unwrap();
    let source = timeline.sequence().media[0].id;
    timeline
        .apply(PlaceClip {
            source,
            source_in: Ticks::ZERO,
            duration: Ticks(25 * 600 * FRAME),
            at: Ticks::ZERO,
            video_track: Some(0),
            audio_track: None,
            mode: PlaceMode::Overwrite,
            fill: None,
        })
        .unwrap();
    Arc::new(timeline.sequence().clone())
}

#[test]
#[ignore = "needs a GPU and encodes with FFmpeg"]
fn quitting_during_an_export_leaves_no_partial_file() {
    let directory = std::env::temp_dir().join(format!("export-quit-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let gpu = gpu();
    let settings = ExportSettings {
        path: directory.join("out.mp4"),
        video: Some(VideoExport {
            encoding: VideoEncoding::Movie(VideoCodec::H264, RateControl::Quality(18)),
            size: (640, 360),
            acceleration: Acceleration::Software,
        }),
        audio: None,
        range: None,
        previews: Vec::new(),
        loudness: None,
    };
    let job = ExportJob::start(
        ten_minutes_of_black(),
        gpu.0.clone(),
        gpu.1.clone(),
        settings,
    )
    .unwrap();
    let writing = 'waiting: loop {
        for event in job.poll() {
            match event {
                ExportEvent::Progress { frames_done, .. } if frames_done > 0 => {
                    break 'waiting true
                }
                ExportEvent::Finished | ExportEvent::Failed(_) => break 'waiting false,
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(writing, "the export ended before writing anything");

    assert!(playback::stop_exports(Duration::from_secs(10)));
    let left: Vec<_> = std::fs::read_dir(&directory).unwrap().collect();
    let _ = std::fs::remove_dir_all(&directory);
    assert!(left.is_empty(), "left behind: {left:?}");
}
