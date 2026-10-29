use std::path::Path;

use timeline::{ChannelMap, ColorSpace, FieldOrder, FrameRate, PixelAspect, Ticks, Timebase};

use crate::channel_map::mapped;
use crate::ffi::{
    Decoder, Frame, InputContext, Packet, Picture, Received, SampleConverter, Scaler,
    OUTPUT_CHANNELS,
};
use crate::seek_trim::{SeekTrim, Trimmed};
use crate::sequence::open_input;
use crate::{AudioBuffer, DecodedFrame, MediaError, StreamKind, VideoFrame};

/// FFmpeg's `AV_TIME_BASE`, used for container-level start time, duration and seeking.
const MICROSECONDS: Timebase = match Timebase::new(1, 1_000_000) {
    Some(timebase) => timebase,
    None => unreachable!(),
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInfo {
    /// In the reader's output timebase; `None` when the container doesn't declare it.
    pub duration: Option<Ticks>,
    pub video: Option<VideoInfo>,
    pub has_audio: bool,
    /// A single picture rather than moving video.
    pub still: bool,
    /// Numbered pictures read as one video.
    pub sequence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub frame_rate: Option<FrameRate>,
    pub pixel_aspect: PixelAspect,
    pub color_space: ColorSpace,
    pub field_order: FieldOrder,
}

/// Which of a file's streams a reader decodes. Timeline playback opens one reader per
/// stream kind so each can seek on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Streams {
    VideoAndAudio,
    Video,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadState {
    Reading,
    Draining,
    Finished,
}

struct StreamDecoder<C> {
    index: usize,
    timebase: Timebase,
    decoder: Decoder,
    converter: C,
    /// Fallback timestamp for frames the container leaves unstamped.
    next_pts: Ticks,
}

impl<C: Default> StreamDecoder<C> {
    fn open(input: &InputContext, kind: StreamKind) -> Result<Option<Self>, MediaError> {
        let Some(index) = input.best_stream(kind) else {
            return Ok(None);
        };
        let timebase = input.stream_timebase(index).ok_or(MediaError::NoDecoder)?;
        Ok(Some(Self {
            index,
            timebase,
            decoder: Decoder::open(input, index)?,
            converter: C::default(),
            next_pts: Ticks::ZERO,
        }))
    }
}

/// Demuxes and decodes the best video and audio stream of a file. All calls block on file
/// I/O and decoding, so the reader belongs on a worker thread.
pub struct MediaReader {
    input: InputContext,
    video: Option<StreamDecoder<Scaler>>,
    audio: Option<StreamDecoder<SampleConverter>>,
    packet: Packet,
    frame: Frame,
    timebase: Timebase,
    /// Container start time in `timebase`, subtracted so that media time starts at zero.
    start: Ticks,
    trim: SeekTrim<Picture>,
    state: ReadState,
    info: SourceInfo,
    channel_map: Option<ChannelMap>,
}

impl MediaReader {
    /// Timestamps of decoded frames, seeks and the reported duration are all in `timebase`.
    pub fn open(path: &Path, timebase: Timebase) -> Result<Self, MediaError> {
        Self::open_streams(path, timebase, Streams::VideoAndAudio)
    }

    pub fn open_streams(
        path: &Path,
        timebase: Timebase,
        streams: Streams,
    ) -> Result<Self, MediaError> {
        let (input, sequence) = open_input(path)?;
        let video = match streams {
            Streams::Audio => None,
            _ => StreamDecoder::<Scaler>::open(&input, StreamKind::Video)?,
        };
        let audio = match streams {
            Streams::Video => None,
            _ => StreamDecoder::<SampleConverter>::open(&input, StreamKind::Audio)?,
        };
        if video.is_none() && audio.is_none() {
            return Err(MediaError::NoPlayableStream);
        }
        let to_ticks = |micros: i64| Ticks(micros).rescale(MICROSECONDS, timebase);
        let mut video = video;
        if let Some(video) = &mut video {
            // Sequences are progressive, so interlaced footage is deinterlaced unless
            // the caller interprets it otherwise.
            video
                .converter
                .set_field_order(input.field_order(video.index));
        }
        let info = SourceInfo {
            duration: input.duration().map(to_ticks),
            video: video.as_ref().map(|video| {
                let (width, height) = input.frame_size(video.index).unwrap_or((0, 0));
                VideoInfo {
                    width,
                    height,
                    frame_rate: input
                        .frame_rate(video.index)
                        .and_then(|(num, den)| FrameRate::new(num, den)),
                    pixel_aspect: input
                        .pixel_aspect(video.index)
                        .and_then(|(num, den)| PixelAspect::new(num, den))
                        .unwrap_or_default(),
                    color_space: input.color_space(video.index),
                    field_order: input.field_order(video.index),
                }
            }),
            has_audio: audio.is_some(),
            still: input.is_image() && !sequence,
            sequence,
        };
        Ok(Self {
            start: input.start_time().map_or(Ticks::ZERO, to_ticks),
            input,
            video,
            audio,
            packet: Packet::new()?,
            frame: Frame::new()?,
            timebase,
            trim: SeekTrim::new(timebase),
            state: ReadState::Reading,
            info,
            channel_map: None,
        })
    }

    pub fn info(&self) -> &SourceInfo {
        &self.info
    }

    /// Resizes pictures to `size` from the next decoded frame on; `None` restores the
    /// decoded size.
    pub fn set_video_size(&mut self, size: Option<(u32, u32)>) -> Result<(), MediaError> {
        match &mut self.video {
            Some(video) => video.converter.set_output_size(size),
            None => Ok(()),
        }
    }

    /// Rebuilds interlaced pictures from their first field from the next decoded frame
    /// on; [`FieldOrder::Progressive`] shows them as decoded.
    pub fn set_field_order(&mut self, fields: FieldOrder) {
        if let Some(video) = &mut self.video {
            video.converter.set_field_order(fields);
        }
    }

    /// Plays the source channels `map` names on each side from the next decoded sound
    /// on; `None` mixes them all to stereo again.
    pub fn set_channel_map(&mut self, map: Option<ChannelMap>) {
        self.channel_map = map;
        if let Some(audio) = &mut self.audio {
            audio.converter.set_keep_channels(map.is_some());
        }
    }

    /// Positions the reader so that the next frames are the picture showing at `position`
    /// and the audio starting exactly there.
    pub fn seek(&mut self, position: Ticks) -> Result<(), MediaError> {
        // A picture's only frame shows throughout, but is stamped at its start.
        let position = if self.info.still {
            Ticks::ZERO
        } else {
            position
        };
        let target = (position + self.start).rescale(self.timebase, MICROSECONDS);
        self.input.seek(target.0)?;
        if let Some(video) = &mut self.video {
            video.decoder.flush();
        }
        if let Some(audio) = &mut self.audio {
            audio.decoder.flush();
        }
        self.trim.reset(position);
        self.state = ReadState::Reading;
        Ok(())
    }

    /// Returns frames in decode order, interleaving video and audio; `None` at end of media.
    pub fn next_frame(&mut self) -> Result<Option<DecodedFrame>, MediaError> {
        loop {
            if let Some(trimmed) = self.trim.pop() {
                return self.finish(trimmed).map(Some);
            }
            match self.state {
                ReadState::Finished => return Ok(None),
                ReadState::Draining => {
                    self.decode(None)?;
                    self.trim.finish();
                    self.state = ReadState::Finished;
                }
                ReadState::Reading => {
                    if self.input.read_packet(&mut self.packet)? {
                        let index = self.packet.stream_index();
                        self.decode(index)?;
                    } else {
                        self.state = ReadState::Draining;
                    }
                }
            }
        }
    }

    fn finish(&mut self, trimmed: Trimmed<Picture>) -> Result<DecodedFrame, MediaError> {
        match trimmed {
            Trimmed::Audio(buffer) => Ok(DecodedFrame::Audio(buffer)),
            Trimmed::Video(pts, picture) => {
                let video = self.video.as_mut().ok_or(MediaError::InvalidFrame)?;
                let (width, height, pixels) = video.converter.convert(&picture)?;
                Ok(DecodedFrame::Video(VideoFrame {
                    pts,
                    width,
                    height,
                    pixels,
                }))
            }
        }
    }

    /// Feeds the current packet to the decoder of stream `index`, or drains both decoders
    /// when `index` is `None`.
    fn decode(&mut self, index: Option<usize>) -> Result<(), MediaError> {
        let draining = index.is_none();
        let packet = (!draining).then_some(&self.packet);
        let clock = Clock {
            output: self.timebase,
            start: self.start,
        };
        if let Some(video) = self
            .video
            .as_mut()
            .filter(|v| draining || index == Some(v.index))
        {
            video.decoder.send_packet(packet)?;
            while let Received::Frame = video.decoder.receive_frame(&mut self.frame)? {
                let pts = clock.pts(&self.frame, video.timebase, video.next_pts);
                let picture = Picture::take(&mut self.frame, video.decoder.picture_format())?;
                video.next_pts = pts;
                self.trim.push_video(pts, picture);
            }
        }
        if let Some(audio) = self
            .audio
            .as_mut()
            .filter(|a| draining || index == Some(a.index))
        {
            audio.decoder.send_packet(packet)?;
            while let Received::Frame = audio.decoder.receive_frame(&mut self.frame)? {
                let pts = clock.pts(&self.frame, audio.timebase, audio.next_pts);
                let (sample_rate, channels, samples) =
                    audio.converter.convert(&audio.decoder, &self.frame)?;
                self.frame.unref();
                let samples = match self.channel_map {
                    Some(map) => mapped(&samples, channels, map),
                    None => samples,
                };
                let buffer = AudioBuffer {
                    pts,
                    sample_rate,
                    channels: OUTPUT_CHANNELS,
                    samples,
                };
                audio.next_pts = buffer.end(self.timebase);
                self.trim.push_audio(buffer);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Clock {
    output: Timebase,
    start: Ticks,
}

impl Clock {
    fn pts(self, frame: &Frame, stream: Timebase, fallback: Ticks) -> Ticks {
        frame.timestamp().map_or(fallback, |ts| {
            Ticks(ts).rescale(stream, self.output) - self.start
        })
    }
}
