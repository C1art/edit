use std::ops::ControlFlow;
use std::path::Path;

use timeline::{FrameRate, Ticks, Timebase};

use crate::{
    Acceleration, DecodedFrame, EncodeProfile, MediaError, MediaReader, MediaWriter, RateControl,
    Streams, VideoCodec, VideoFrame, VideoSettings,
};

/// Good enough to judge framing and motion; the original is used for export.
const PROXY_CRF: u8 = 26;

/// The size of a proxy no taller than `max_height`, keeping the source's shape and the
/// even dimensions 4:2:0 needs. Sources already small enough keep their size.
pub fn proxy_size((width, height): (u32, u32), max_height: u32) -> (u32, u32) {
    let out_height = (height.min(max_height) / 2 * 2).max(2);
    let out_width = (u64::from(width) * u64::from(out_height) + u64::from(height) / 2)
        / u64::from(height.max(1));
    let out_width = u32::try_from(out_width).unwrap_or(u32::MAX) / 2 * 2;
    (out_width.max(2), out_height)
}

/// Writes a small copy of the pictures in `source` to `destination` that decodes quickly
/// and seeks cheaply, at the source's frame rate and length. `progress` hears the
/// fraction done and can stop the work, leaving no file behind. Blocks on decoding and
/// encoding, so run it on a worker thread.
pub fn create_proxy(
    source: &Path,
    destination: &Path,
    max_height: u32,
    mut progress: impl FnMut(f32) -> ControlFlow<()>,
) -> Result<(), MediaError> {
    let result = transcode(source, destination, max_height, &mut progress);
    if result.is_err() {
        // A partial proxy would look complete but stop early.
        let _ = std::fs::remove_file(destination);
    }
    result
}

fn transcode(
    source: &Path,
    destination: &Path,
    max_height: u32,
    progress: &mut impl FnMut(f32) -> ControlFlow<()>,
) -> Result<(), MediaError> {
    let timebase = Timebase::HZ_90K;
    let mut reader = MediaReader::open_streams(source, timebase, Streams::Video)?;
    let info = reader.info().clone();
    let video = info
        .video
        // A picture of unknown size would make a proxy too small to stand in for it.
        .filter(|video| video.width > 0 && video.height > 0)
        .ok_or(MediaError::NoPlayableStream)?;
    let rate = video.frame_rate.unwrap_or(FrameRate::FPS_25);
    let (width, height) = proxy_size((video.width, video.height), max_height);
    reader.set_video_size(Some((width, height)))?;
    let settings = VideoSettings {
        input_size: (width, height),
        width,
        height,
        frame_rate: rate,
        codec: VideoCodec::H264,
        rate_control: RateControl::Quality(PROXY_CRF),
        profile: EncodeProfile::Editing,
        acceleration: Acceleration::Hardware,
    };
    let mut writer = MediaWriter::create(destination, Some(settings), None)?;
    let mut out = Retimer {
        rate,
        timebase,
        next: 0,
    };
    let mut last: Option<VideoFrame> = None;
    while let Some(decoded) = reader.next_frame()? {
        let DecodedFrame::Video(frame) = decoded else {
            continue;
        };
        // Output frames before this one show the previous picture, or this one if it is
        // the first, so the proxy holds and drops frames exactly as playback would.
        while out.start() < frame.pts {
            writer.write_video(&last.as_ref().unwrap_or(&frame).pixels)?;
            out.next += 1;
        }
        if let Some(duration) = info.duration.filter(|d| *d > Ticks::ZERO) {
            let done = frame.pts.0 as f64 / duration.0 as f64;
            if progress(done.clamp(0.0, 1.0) as f32).is_break() {
                return Err(MediaError::Cancelled);
            }
        }
        last = Some(frame);
    }
    let last = last.ok_or(MediaError::NoPlayableStream)?;
    let end = info
        .duration
        .unwrap_or_else(|| last.pts + rate.frame_start(1, timebase));
    while out.start() < end {
        writer.write_video(&last.pixels)?;
        out.next += 1;
    }
    writer.finish()
}

/// Counts output frames at a constant rate.
struct Retimer {
    rate: FrameRate,
    timebase: Timebase,
    next: i64,
}

impl Retimer {
    fn start(&self) -> Ticks {
        self.rate.frame_start(self.next, self.timebase)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_keeps_shape_with_even_sides() {
        assert_eq!(proxy_size((1920, 1080), 540), (960, 540));
        assert_eq!(proxy_size((1440, 1080), 540), (720, 540));
        // 4096x2160 at 540 is 1024 wide; an odd source height still gives even sides.
        assert_eq!(proxy_size((4096, 2160), 540), (1024, 540));
        assert_eq!(proxy_size((1001, 563), 281), (498, 280));
        assert_eq!(proxy_size((320, 240), 540), (320, 240));
    }

    #[test]
    #[ignore = "encodes and decodes a real file with FFmpeg"]
    fn proxy_matches_source_duration_at_reduced_size() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/testsrc_2s_25fps_44k.mp4");
        let dir = std::env::temp_dir().join(format!("media-proxy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("proxy.mp4");
        create_proxy(&fixture, &path, 120, |_| ControlFlow::Continue(())).unwrap();

        let info = MediaReader::open(&path, Timebase::HZ_90K)
            .unwrap()
            .info()
            .clone();
        // The reader is gone by now, as Windows won't remove a file that is still open.
        std::fs::remove_dir_all(&dir).unwrap();
        let video = info.video.unwrap();
        assert_eq!((video.width, video.height), (160, 120));
        assert!(!info.has_audio);
        let duration = info.duration.unwrap();
        assert!((duration - Ticks(180_000)).0.abs() <= 3_600, "{duration:?}");
    }
}
