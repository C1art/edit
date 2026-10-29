use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use media::{DecodedFrame, MediaError, MediaReader, Progress, Streams, VideoFrame};
use timeline::{Generator, MediaItem, SourceId, Ticks, TimeInterpolation, Timebase};

use crate::clip_frame::ClipFrame;
use crate::compose::TrackDecoders;
use crate::generated::offline_frame;
use crate::optical_flow::{self, Flow};
use crate::{frame_blend, generated_frame, Resolution};

/// Jumps further ahead than this seek instead of decoding through; keyframe intervals
/// are usually shorter, so decoding forward is cheaper below it.
const MAX_DECODE_AHEAD_SECONDS: i64 = 2;
/// Memory for pictures kept while playing backwards, which would otherwise seek and
/// decode from a keyframe for every frame.
const BACKWARD_BYTES: usize = 192 << 20;
const MAX_BACKWARD_FRAMES: usize = 60;
/// Pictures always kept, as many as the player composes ahead while paused and a few
/// more, so stepping into what was just composed finds them instead of seeking back.
const RECENT_FRAMES: usize = 12;
const RETRY_OPEN: Duration = Duration::from_secs(3);

enum Source {
    None,
    Open(SourceId, PathBuf, Box<MediaReader>),
    /// Opening failed; retried after a while, in case the file comes back, or when the
    /// track moves to other media or another file of it, such as its proxy.
    Failed(SourceId, PathBuf, Instant),
    /// Drawn once at a canvas size, and frame for what changes over time, and held for
    /// as long as that shows.
    Generated(Generator, [u32; 2], i64, Arc<VideoFrame>),
}

/// Decodes the pictures one video track needs, reading forward while playback is
/// continuous and seeking when it jumps.
pub(crate) struct TrackVideo {
    timebase: Timebase,
    source: Source,
    current: Option<ClipFrame>,
    next: Option<ClipFrame>,
    ended: bool,
    /// The pictures decoded up to `current`, oldest first, kept while requests go
    /// backwards.
    behind: VecDeque<ClipFrame>,
    /// The decoders of a nested sequence's tracks, when the clip followed is one.
    pub(crate) nested: Vec<TrackDecoders>,
    /// What the open reader's pictures are sized for.
    resolution: Resolution,
    /// The motion between the last pair of frames optical flow was asked between,
    /// which a slowed clip shows several pictures from.
    flow: Option<((Ticks, Ticks), Arc<Flow>)>,
    /// How many of the last pictures decoded to keep, going forwards too.
    history: usize,
    /// Whether pictures the GPU can turn into RGB are left in their planes.
    planar: bool,
    /// The time the reader last sought for; decoding on toward it doesn't seek again.
    sought: Option<Ticks>,
    /// Why opening failed while prerolling, for the next request to report.
    unreported: Option<MediaError>,
}

/// What the source followed has to show, once it's opened.
enum Ready {
    Reader,
    Offline,
    Nothing,
}

impl TrackVideo {
    pub(crate) fn new(timebase: Timebase) -> Self {
        Self {
            timebase,
            source: Source::None,
            current: None,
            next: None,
            ended: false,
            behind: VecDeque::new(),
            nested: Vec::new(),
            resolution: Resolution::Full,
            flow: None,
            history: 0,
            planar: false,
            sought: None,
            unreported: None,
        }
    }

    /// Leaves the pictures decoded from now on in their planes where it can, for
    /// clips no effect needs the RGBA of.
    pub(crate) fn keep_planes(&mut self, planar: bool) {
        self.planar = planar;
    }

    /// Keeps up to the last `frames` pictures decoded, so that requests that far back
    /// find them without seeking, as Echo makes.
    pub(crate) fn keep_history(&mut self, frames: usize) {
        self.history = frames;
    }

    /// Drops decoder state, so the next request seeks.
    pub(crate) fn reset(&mut self) {
        self.current = None;
        self.next = None;
        self.behind.clear();
        self.flow = None;
        self.sought = None;
    }

    /// The picture showing at `time` in `media`. A file that can't be opened is
    /// reported once, then shows as offline.
    pub(crate) fn frame_at(
        &mut self,
        media: &MediaItem,
        time: Ticks,
        canvas: [u32; 2],
        resolution: Resolution,
    ) -> Result<Option<ClipFrame>, MediaError> {
        if let Some(generator) = &media.generator {
            let frame = self.generated(media, generator, time, canvas);
            return Ok(Some(ClipFrame::Rgba(frame)));
        }
        let offline = || Ok(Some(ClipFrame::Rgba(offline_frame(canvas[0], canvas[1]))));
        if let Some(error) = self.unreported.take() {
            return Err(error);
        }
        if media.offline {
            return offline();
        }
        match self.ready(media, resolution)? {
            Ready::Reader => {}
            Ready::Offline => return offline(),
            Ready::Nothing => return Ok(None),
        }
        // A picture is decoded once and held, as seeking past its only frame finds none.
        let time = if media.still { Ticks::ZERO } else { time };
        let current = self.current.as_ref().map(ClipFrame::pts);
        let backward = current.is_some_and(|current| time < current);
        if backward {
            if let Some(frame) = frame_before(&self.behind, time) {
                return Ok(Some(frame));
            }
        }
        let history = self.history.max(RECENT_FRAMES).min(backward_frames(media));
        let keep = if backward { backward_frames(media) } else { 0 }.max(history);
        self.decode_to(media, time, keep, usize::MAX)?;
        Ok(self.current.clone())
    }

    /// Decodes toward `time` in `media`, reading at most `packets` packets at once, so that
    /// asking for it later finds it without waiting. Returns whether it's there.
    pub(crate) fn preroll(
        &mut self,
        media: &MediaItem,
        time: Ticks,
        resolution: Resolution,
        packets: usize,
    ) -> Result<bool, MediaError> {
        if media.generator.is_some() || media.offline || media.still {
            return Ok(true);
        }
        match self.ready(media, resolution) {
            Ok(Ready::Reader) => {
                self.behind.clear();
                self.decode_to(media, time, 0, packets)
            }
            Ok(Ready::Offline | Ready::Nothing) => Ok(true),
            Err(error) => {
                self.unreported = Some(error);
                Ok(true)
            }
        }
    }

    /// Opens `media` unless it's open already, sized for `resolution`.
    fn ready(&mut self, media: &MediaItem, resolution: Resolution) -> Result<Ready, MediaError> {
        let same_media = match &self.source {
            Source::Open(id, path, _) => *id == media.id && *path == media.path,
            Source::Failed(id, path, at) => {
                *id == media.id && *path == media.path && at.elapsed() < RETRY_OPEN
            }
            Source::None | Source::Generated(..) => false,
        };
        if !same_media {
            self.reset();
            self.source =
                match MediaReader::open_streams(&media.path, self.timebase, Streams::Video) {
                    Ok(reader) => Source::Open(media.id, media.path.clone(), Box::new(reader)),
                    Err(error) => {
                        let at = Instant::now();
                        self.source = Source::Failed(media.id, media.path.clone(), at);
                        return Err(error);
                    }
                };
        }
        let reader = match &mut self.source {
            Source::Open(_, _, reader) => reader,
            Source::Failed(..) => return Ok(Ready::Offline),
            Source::None | Source::Generated(..) => return Ok(Ready::Nothing),
        };
        reader.set_planar(self.planar);
        if !same_media || resolution != self.resolution {
            let size = media
                .video
                .and_then(|video| resolution.shrink(video.width, video.height));
            reader.set_video_size(size)?;
            reader.set_field_order(media.field_order());
            self.resolution = resolution;
            // Pictures decoded at the old size would otherwise keep showing.
            self.reset();
        }
        Ok(Ready::Reader)
    }

    /// Decodes until the picture showing at `time` is `current`, keeping the last
    /// `keep` decoded, or until `packets` packets are read. Returns whether it got
    /// there.
    fn decode_to(
        &mut self,
        media: &MediaItem,
        time: Ticks,
        keep: usize,
        packets: usize,
    ) -> Result<bool, MediaError> {
        let Source::Open(_, _, reader) = &mut self.source else {
            return Ok(true);
        };
        let decode_ahead = Timebase::new(1, 1).map_or(Ticks::ZERO, |second| {
            Ticks(MAX_DECODE_AHEAD_SECONDS).rescale(second, self.timebase)
        });
        let near = |from: Ticks| from <= time && time <= from + decode_ahead;
        // A seek still on its way to its target counts as being there.
        let sought = self.sought.is_some_and(near);
        let continuous = match &self.current {
            Some(current) => current.pts() <= time && (near(current.pts()) || sought),
            None => sought,
        };
        if !continuous {
            // Going backwards, start early enough that the frames before `time` are
            // decoded on the way and kept for the requests that follow.
            let back = media.video.map_or(Ticks::ZERO, |video| {
                video.frame_rate.frame_start(keep as i64, self.timebase)
            });
            reader.seek((time - back).max(Ticks::ZERO))?;
            self.sought = Some(time);
            self.current = None;
            self.next = None;
            self.ended = false;
            self.behind.clear();
        }
        let mut packets = packets;
        loop {
            if self.next.is_none() && !self.ended {
                match reader.next_frame_within(&mut packets)? {
                    Progress::Frame(DecodedFrame::Video(frame)) => {
                        self.next = Some(ClipFrame::Rgba(Arc::new(frame)));
                    }
                    Progress::Frame(DecodedFrame::Planar(frame)) => {
                        self.next = Some(ClipFrame::Planar(Arc::new(frame)));
                    }
                    Progress::Frame(DecodedFrame::Audio(_)) => continue,
                    Progress::Ended => self.ended = true,
                    Progress::Unfinished => return Ok(false),
                }
            }
            let due = self
                .next
                .as_ref()
                .is_some_and(|next| next.pts() <= time || self.current.is_none());
            if !due {
                return Ok(true);
            }
            self.current = self.next.take();
            if let Some(current) = self.current.as_ref().filter(|_| keep > 0) {
                while self.behind.len() >= keep {
                    self.behind.pop_front();
                }
                self.behind.push_back(current.clone());
            }
        }
    }

    /// The picture at `time`, made from the frames either side of it as `interpolation`
    /// says when it falls between them.
    pub(crate) fn interpolated_frame_at(
        &mut self,
        media: &MediaItem,
        time: Ticks,
        canvas: [u32; 2],
        resolution: Resolution,
        interpolation: TimeInterpolation,
    ) -> Result<Option<ClipFrame>, MediaError> {
        let Some(earlier) = self.frame_at(media, time, canvas, resolution)? else {
            return Ok(None);
        };
        // Pictures kept for going backwards aren't followed by `next`.
        let decoded_last = self
            .current
            .as_ref()
            .is_some_and(|current| current.is(&earlier));
        let later = self.next.as_ref().filter(|later| {
            decoded_last && later.pts() > earlier.pts() && later.size() == earlier.size()
        });
        let Some(later) = later else {
            return Ok(Some(earlier));
        };
        let weight = frame_blend::weight(earlier.pts(), later.pts(), time);
        if weight == 0 {
            return Ok(Some(earlier));
        }
        let (earlier, later) = (earlier.rgba()?, later.rgba()?);
        if interpolation != TimeInterpolation::OpticalFlow {
            let blended = frame_blend::blend(&earlier, &later, weight);
            return Ok(Some(ClipFrame::Rgba(Arc::new(blended))));
        }
        let pair = (earlier.pts, later.pts);
        let flow = match &self.flow {
            Some((kept, flow)) if *kept == pair => Arc::clone(flow),
            _ => {
                let flow = Arc::new(Flow::between(&earlier, &later));
                self.flow = Some((pair, Arc::clone(&flow)));
                flow
            }
        };
        let fraction = weight as f32 / frame_blend::FULL as f32;
        let interpolated = optical_flow::interpolate(&earlier, &later, &flow, fraction);
        Ok(Some(ClipFrame::Rgba(Arc::new(interpolated))))
    }

    /// The offline picture, when the file followed couldn't be opened.
    pub(crate) fn offline(&self, canvas: [u32; 2]) -> Option<Arc<VideoFrame>> {
        matches!(self.source, Source::Failed(..)).then(|| offline_frame(canvas[0], canvas[1]))
    }

    fn generated(
        &mut self,
        media: &MediaItem,
        generator: &Generator,
        time: Ticks,
        canvas: [u32; 2],
    ) -> Arc<VideoFrame> {
        let rate = media.video.unwrap_or_default().frame_rate;
        let at = match generator {
            Generator::CountingLeader(_) => rate.frame_at(time, self.timebase),
            _ => 0,
        };
        if let Source::Generated(drawn, size, drawn_at, frame) = &self.source {
            if drawn == generator && *size == canvas && *drawn_at == at {
                return Arc::clone(frame);
            }
        }
        self.reset();
        let frame = generated_frame(generator, (at, rate), canvas[0], canvas[1]);
        self.source = Source::Generated(generator.clone(), canvas, at, Arc::clone(&frame));
        frame
    }
}

/// The kept picture showing at `time`, when the kept ones reach back that far.
fn frame_before(behind: &VecDeque<ClipFrame>, time: Ticks) -> Option<ClipFrame> {
    if behind.front().is_none_or(|first| first.pts() > time) {
        return None;
    }
    behind
        .iter()
        .rev()
        .find(|frame| frame.pts() <= time)
        .cloned()
}

fn backward_frames(media: &MediaItem) -> usize {
    let bytes = media.video.map_or(usize::MAX, |video| {
        (video.width as usize * video.height as usize * 4).max(1)
    });
    (BACKWARD_BYTES / bytes).clamp(2, MAX_BACKWARD_FRAMES)
}

#[cfg(test)]
mod tests {
    use timeline::VideoFormat;

    use super::*;

    fn fixture() -> MediaItem {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/testsrc_2s_25fps_44k.mp4");
        MediaItem {
            id: SourceId(1),
            name: "testsrc".into(),
            path,
            duration: Ticks(180_000),
            video: Some(VideoFormat {
                width: 320,
                height: 240,
                ..VideoFormat::default()
            }),
            has_audio: true,
            proxy: None,
            bin: None,
            generator: None,
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
        }
    }

    #[test]
    fn preroll_decodes_toward_a_time_bit_by_bit_and_leaves_it_ready() {
        let timebase = Timebase::new(1, 90_000).unwrap();
        let mut video = TrackVideo::new(timebase);
        let (media, time) = (fixture(), Ticks(162_000));
        // The fixture's keyframes are a second apart, so 1.8 s takes some decoding
        // from the one at 1 s, which seeking again at every step would never finish.
        let steps = (1..=1000)
            .find(|_| video.preroll(&media, time, Resolution::Full, 1).unwrap())
            .unwrap();
        assert!(steps > 10, "{steps}");
        let frame = video.frame_at(&media, time, [320, 240], Resolution::Full);
        assert_eq!(frame.unwrap().unwrap().pts(), time);
    }

    #[test]
    fn stepping_back_into_what_was_just_decoded_does_not_seek() {
        let timebase = Timebase::new(1, 90_000).unwrap();
        let mut video = TrackVideo::new(timebase);
        let media = fixture();
        let mut frame_at = |time| {
            let frame = video.frame_at(&media, Ticks(time), [320, 240], Resolution::Full);
            frame.unwrap().unwrap().pts()
        };
        frame_at(108_000);
        // Composed ahead while paused, then a step lands short of it.
        frame_at(136_800);
        assert_eq!(frame_at(118_800), Ticks(118_800));
        assert_eq!(video.sought, Some(Ticks(108_000)));
    }
}
