use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use media::{DecodedFrame, MediaError, MediaReader, Streams, VideoFrame};
use timeline::{Generator, MediaItem, SourceId, Ticks, TimeInterpolation, Timebase};

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
    current: Option<Arc<VideoFrame>>,
    next: Option<VideoFrame>,
    ended: bool,
    /// The pictures decoded up to `current`, oldest first, kept while requests go
    /// backwards.
    behind: VecDeque<Arc<VideoFrame>>,
    /// The decoders of a nested sequence's tracks, when the clip followed is one.
    pub(crate) nested: Vec<TrackDecoders>,
    /// What the open reader's pictures are sized for.
    resolution: Resolution,
    /// The motion between the last pair of frames optical flow was asked between,
    /// which a slowed clip shows several pictures from.
    flow: Option<((Ticks, Ticks), Arc<Flow>)>,
    /// How many of the last pictures decoded to keep, going forwards too.
    history: usize,
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
        }
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
    }

    /// The picture showing at `time` in `media`. A file that can't be opened is
    /// reported once, then shows as offline.
    pub(crate) fn frame_at(
        &mut self,
        media: &MediaItem,
        time: Ticks,
        canvas: [u32; 2],
        resolution: Resolution,
    ) -> Result<Option<Arc<VideoFrame>>, MediaError> {
        if let Some(generator) = &media.generator {
            return Ok(Some(self.generated(media, generator, time, canvas)));
        }
        if media.offline {
            return Ok(Some(offline_frame(canvas[0], canvas[1])));
        }
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
            Source::Failed(..) => return Ok(Some(offline_frame(canvas[0], canvas[1]))),
            Source::None | Source::Generated(..) => return Ok(None),
        };
        if !same_media || resolution != self.resolution {
            let size = media
                .video
                .and_then(|video| resolution.shrink(video.width, video.height));
            reader.set_video_size(size)?;
            reader.set_field_order(media.field_order());
            self.resolution = resolution;
            // Pictures decoded at the old size would otherwise keep showing.
            self.current = None;
            self.next = None;
            self.behind.clear();
            self.flow = None;
        }
        // A picture is decoded once and held, as seeking past its only frame finds none.
        let time = if media.still { Ticks::ZERO } else { time };
        let decode_ahead = Timebase::new(1, 1).map_or(Ticks::ZERO, |second| {
            Ticks(MAX_DECODE_AHEAD_SECONDS).rescale(second, self.timebase)
        });
        let current = self.current.as_ref().map(|current| current.pts);
        let backward = current.is_some_and(|current| time < current);
        if backward {
            if let Some(frame) = frame_before(&self.behind, time) {
                return Ok(Some(frame));
            }
        } else if self.history == 0 {
            self.behind.clear();
        }
        let continuous =
            current.is_some_and(|current| current <= time && time <= current + decode_ahead);
        let history = self.history.min(backward_frames(media));
        let keep = if backward { backward_frames(media) } else { 0 }.max(history);
        if !continuous {
            // Going backwards, start early enough that the frames before `time` are
            // decoded on the way and kept for the requests that follow.
            let back = media.video.map_or(Ticks::ZERO, |video| {
                video.frame_rate.frame_start(keep as i64, self.timebase)
            });
            reader.seek((time - back).max(Ticks::ZERO))?;
            self.current = None;
            self.next = None;
            self.ended = false;
            self.behind.clear();
        }
        loop {
            if self.next.is_none() && !self.ended {
                match reader.next_frame()? {
                    Some(DecodedFrame::Video(frame)) => self.next = Some(frame),
                    Some(DecodedFrame::Audio(_)) => continue,
                    None => self.ended = true,
                }
            }
            let due = self
                .next
                .as_ref()
                .is_some_and(|next| next.pts <= time || self.current.is_none());
            if !due {
                break;
            }
            self.current = self.next.take().map(Arc::new);
            if let Some(current) = self.current.as_ref().filter(|_| keep > 0) {
                if self.behind.len() == keep {
                    self.behind.pop_front();
                }
                self.behind.push_back(Arc::clone(current));
            }
        }
        Ok(self.current.clone())
    }

    /// Like `frame_at`, but faded toward the following frame by how far `time` lies
    /// past the showing one.
    /// The picture at `time`, made from the frames either side of it as `interpolation`
    /// says when it falls between them.
    pub(crate) fn interpolated_frame_at(
        &mut self,
        media: &MediaItem,
        time: Ticks,
        canvas: [u32; 2],
        resolution: Resolution,
        interpolation: TimeInterpolation,
    ) -> Result<Option<Arc<VideoFrame>>, MediaError> {
        let Some(earlier) = self.frame_at(media, time, canvas, resolution)? else {
            return Ok(None);
        };
        // Pictures kept for going backwards aren't followed by `next`.
        let decoded_last = self
            .current
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &earlier));
        let later = self.next.as_ref().filter(|later| {
            decoded_last && later.pts > earlier.pts && later.pixels.len() == earlier.pixels.len()
        });
        let Some(later) = later else {
            return Ok(Some(earlier));
        };
        let weight = frame_blend::weight(earlier.pts, later.pts, time);
        if weight == 0 {
            return Ok(Some(earlier));
        }
        if interpolation != TimeInterpolation::OpticalFlow {
            return Ok(Some(Arc::new(frame_blend::blend(&earlier, later, weight))));
        }
        let pair = (earlier.pts, later.pts);
        let flow = match &self.flow {
            Some((kept, flow)) if *kept == pair => Arc::clone(flow),
            _ => {
                let flow = Arc::new(Flow::between(&earlier, later));
                self.flow = Some((pair, Arc::clone(&flow)));
                flow
            }
        };
        let fraction = weight as f32 / frame_blend::FULL as f32;
        Ok(Some(Arc::new(optical_flow::interpolate(
            &earlier, later, &flow, fraction,
        ))))
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
fn frame_before(behind: &VecDeque<Arc<VideoFrame>>, time: Ticks) -> Option<Arc<VideoFrame>> {
    if behind.front().is_none_or(|first| first.pts > time) {
        return None;
    }
    behind.iter().rev().find(|frame| frame.pts <= time).cloned()
}

fn backward_frames(media: &MediaItem) -> usize {
    let bytes = media.video.map_or(usize::MAX, |video| {
        (video.width as usize * video.height as usize * 4).max(1)
    });
    (BACKWARD_BYTES / bytes).clamp(2, MAX_BACKWARD_FRAMES)
}
