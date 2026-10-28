use std::sync::Arc;

use media::{DecodedFrame, MediaError, MediaReader, Streams, VideoFrame};
use timeline::{MediaItem, SourceId, Ticks, Timebase};

/// Jumps further ahead than this seek instead of decoding through; keyframe intervals
/// are usually shorter, so decoding forward is cheaper below it.
const MAX_DECODE_AHEAD_SECONDS: i64 = 2;

enum Source {
    None,
    Open(SourceId, Box<MediaReader>),
    /// Opening failed; not retried until the track moves to other media.
    Failed(SourceId),
}

/// Decodes the pictures one video track needs, reading forward while playback is
/// continuous and seeking when it jumps.
pub(crate) struct TrackVideo {
    timebase: Timebase,
    source: Source,
    current: Option<Arc<VideoFrame>>,
    next: Option<VideoFrame>,
    ended: bool,
}

impl TrackVideo {
    pub(crate) fn new(timebase: Timebase) -> Self {
        Self {
            timebase,
            source: Source::None,
            current: None,
            next: None,
            ended: false,
        }
    }

    /// Drops decoder state, so the next request seeks.
    pub(crate) fn reset(&mut self) {
        self.current = None;
        self.next = None;
    }

    /// The picture showing at `time` in `media`. Errors are reported once per media
    /// item; after that the track shows nothing for it.
    pub(crate) fn frame_at(
        &mut self,
        media: &MediaItem,
        time: Ticks,
    ) -> Result<Option<Arc<VideoFrame>>, MediaError> {
        let same_media = match &self.source {
            Source::Open(id, _) | Source::Failed(id) => *id == media.id,
            Source::None => false,
        };
        if !same_media {
            self.reset();
            self.source =
                match MediaReader::open_streams(&media.path, self.timebase, Streams::Video) {
                    Ok(reader) => Source::Open(media.id, Box::new(reader)),
                    Err(error) => {
                        self.source = Source::Failed(media.id);
                        return Err(error);
                    }
                };
        }
        let Source::Open(_, reader) = &mut self.source else {
            return Ok(None);
        };
        let decode_ahead = Timebase::new(1, 1).map_or(Ticks::ZERO, |second| {
            Ticks(MAX_DECODE_AHEAD_SECONDS).rescale(second, self.timebase)
        });
        let continuous = self
            .current
            .as_ref()
            .is_some_and(|current| current.pts <= time && time <= current.pts + decode_ahead);
        if !continuous {
            reader.seek(time)?;
            self.current = None;
            self.next = None;
            self.ended = false;
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
        }
        Ok(self.current.clone())
    }
}
