// Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use media::{DecodedFrame, MediaReader, RateControl, VideoCodec};
use playback::{ExportEvent, ExportJob, ExportSettings, VideoExport};
use timeline::{
    AddGenerated, FrameRate, Generator, PlaceClip, PlaceMode, Rgb, Sequence, Shape, ShapeKind,
    Ticks, Timebase, Timeline, VideoFormat,
};

const TB: Timebase = Timebase::HZ_90K;

/// wgpu's native futures are ready when first polled or soon after.
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::yield_now();
    }
}

/// A white rectangle half the frame's size in its middle, and nothing under it.
fn sequence() -> Arc<Sequence> {
    let format = VideoFormat {
        width: 64,
        height: 36,
        frame_rate: FrameRate::FPS_25,
    };
    let mut timeline = Timeline::new(Sequence::with_default_tracks(TB, format));
    let shape = Shape {
        width: 32,
        height: 18,
        fill: Some(Rgb::WHITE),
        ..Shape::new(ShapeKind::Rectangle)
    };
    timeline
        .apply(AddGenerated {
            name: "Box".into(),
            generator: Generator::Shape(shape),
        })
        .unwrap();
    timeline
        .apply(PlaceClip {
            source: timeline.sequence().media[0].id,
            source_in: Ticks::ZERO,
            duration: Ticks(90_000 / 5),
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
#[ignore = "needs a GPU and encodes a real file with FFmpeg"]
fn prores_4444_export_is_clear_where_nothing_is_drawn() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("a GPU adapter");
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("a GPU device");
    let path = std::env::temp_dir().join(format!("alpha-export-{}.mov", std::process::id()));
    let settings = ExportSettings {
        path: path.clone(),
        video: Some(VideoExport {
            codec: VideoCodec::ProRes4444,
            rate_control: RateControl::Quality(0),
            size: (64, 36),
        }),
        audio: None,
        range: None,
    };
    let job = ExportJob::start(sequence(), device, queue, settings).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    'export: loop {
        assert!(Instant::now() < deadline, "timed out");
        for event in job.poll() {
            match event {
                ExportEvent::Finished => break 'export,
                ExportEvent::Failed(error) => panic!("{error}"),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    let mut reader = MediaReader::open(&path, TB).unwrap();
    let Some(DecodedFrame::Video(frame)) = reader.next_frame().unwrap() else {
        panic!("no picture");
    };
    let rgba = |x: usize, y: usize| {
        let at = (y * 64 + x) * 4;
        &frame.pixels[at..at + 4]
    };
    assert!(rgba(2, 2)[3] < 4, "{:?}", rgba(2, 2));
    assert!(
        rgba(32, 18).iter().all(|&value| value > 250),
        "{:?}",
        rgba(32, 18)
    );
    std::fs::remove_file(&path).unwrap();
}
