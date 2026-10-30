use std::path::Path;

use timeline::{ChannelMap, ColorSpace, FieldOrder, FrameRate, PixelAspect, Ticks, Timebase};

use crate::channel_map::mapped;
use crate::ffi::{
    Decoder, Frame, InputContext, Packet, Picture, Received, SampleConverter, Scaler,
    OUTPUT_CHANNELS,
};
use crate::seek_landing::Landing;
use crate::seek_trim::{SeekTrim, Trimmed};
use crate::sequence::open_input;
use crate::{AudioBuffer, DecodedFrame, MediaError, PlanarFrame, StreamKind, VideoFrame};

/// FFmpeg's `AV_TIME_BASE`, used for container-level start time, duration and seeking.
const MICROSECONDS: Timebase = match Timebase::new(1, 1_000_000) {
    Some(timebase) => timebase,
    None => unreachable!(),
};
/// How far before the target an audio-only seek lands, in microseconds. Audio decoders
/// need the packets before the first sample they output (AAC overlaps frames, Opus
/// declares 80 ms of pre-roll), and the trim drops what comes before the target anyway.
const AUDIO_PREROLL: i64 = 250_000;

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

pub enum Progress {
    Frame(DecodedFrame),
    Ended,
    /// The packets allowed ran out before a frame came out.
    Unfinished,
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
    /// The latest picture the decoder has put out since the last seek.
    decoded: Option<Ticks>,
    landing: Option<Landing>,
    /// Between pictures, for those the container leaves unstamped.
    picture_step: Ticks,
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
        let (mut input, sequence) = open_input(path)?;
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
        let decoded: Vec<usize> = [
            video.as_ref().map(|v| v.index),
            audio.as_ref().map(|a| a.index),
        ]
        .into_iter()
        .flatten()
        .collect();
        input.keep_only(&decoded);
        let to_ticks = |micros: i64| Ticks(micros).rescale(MICROSECONDS, timebase);
        let mut video = video;
        if let Some(video) = &mut video {
            // Sequences are progressive, so interlaced footage is deinterlaced unless
            // the caller interprets it otherwise.
            video
                .converter
                .set_field_order(input.field_order(video.index));
            video.converter.set_turn(input.display_turn(video.index));
        }
        let info = SourceInfo {
            duration: input.duration().map(to_ticks),
            video: video.as_ref().map(|video| {
                let turn = input.display_turn(video.index);
                let (width, height) = turn.sides(input.frame_size(video.index).unwrap_or((0, 0)));
                VideoInfo {
                    width,
                    height,
                    frame_rate: input
                        .frame_rate(video.index)
                        .and_then(|(num, den)| FrameRate::new(num, den)),
                    pixel_aspect: input
                        .pixel_aspect(video.index)
                        .and_then(|aspect| {
                            let (num, den) = turn.sides(aspect);
                            PixelAspect::new(num, den)
                        })
                        .unwrap_or_default(),
                    color_space: input.color_space(video.index),
                    field_order: input.field_order(video.index),
                }
            }),
            has_audio: audio.is_some(),
            still: input.is_image() && !sequence,
            sequence,
        };
        let picture_step = info
            .video
            .as_ref()
            .and_then(|video| video.frame_rate)
            .map_or(Ticks::ZERO, |rate| rate.frame_start(1, timebase));
        Ok(Self {
            start: input.start_time().map_or(Ticks::ZERO, to_ticks),
            input,
            video,
            audio,
            packet: Packet::new()?,
            frame: Frame::new()?,
            timebase,
            trim: SeekTrim::new(timebase),
            decoded: None,
            landing: None,
            picture_step,
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

    /// Leaves pictures in planes [`PlanarFrame`] takes in theirs, as
    /// [`DecodedFrame::Planar`], from the next decoded frame on, while they need no
    /// deinterlacing. Planes keep their decoded size; the video size applies only to
    /// pictures turned into RGBA.
    pub fn set_planar(&mut self, planar: bool) {
        if let Some(video) = &mut self.video {
            video.converter.set_planar(planar);
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
        self.seek_ahead_of(position, 0)
    }

    /// Seeks for `position`, asking the demuxer for `lead` microseconds earlier.
    fn seek_ahead_of(&mut self, position: Ticks, lead: i64) -> Result<(), MediaError> {
        // A picture's only frame shows throughout, but is stamped at its start.
        let position = if self.info.still {
            Ticks::ZERO
        } else {
            position
        };
        let mut target = (position + self.start)
            .rescale(self.timebase, MICROSECONDS)
            .0
            - lead;
        if self.video.is_none() {
            // With video, FFmpeg seeks on its keyframes, well before the audio is needed;
            // without, it lands on the very audio packet.
            target -= AUDIO_PREROLL;
        }
        // Demuxers such as MP3's and AC-3's refuse a target before their first packet.
        let start = self.start.rescale(self.timebase, MICROSECONDS).0;
        self.input.seek(target.max(start))?;
        if let Some(video) = &mut self.video {
            video.decoder.flush();
        }
        if let Some(audio) = &mut self.audio {
            audio.decoder.flush();
        }
        self.trim.reset(position);
        self.decoded = None;
        self.landing = self.video.is_some().then_some(Landing {
            position,
            lead,
            late: false,
        });
        self.state = ReadState::Reading;
        Ok(())
    }

    /// Seeks again from further back when the last seek's first picture came after its
    /// target, or none came at all; `false` once it already went back to the start.
    fn retreat_if_landed_late(&mut self) -> Result<bool, MediaError> {
        let Some(landing) = self.landing else {
            return Ok(false);
        };
        if !landing.late && self.state != ReadState::Finished {
            return Ok(false);
        }
        let from_start = landing.position.rescale(self.timebase, MICROSECONDS).0;
        if landing.lead >= from_start {
            self.landing = None;
            return Ok(false);
        }
        let lead = (landing.lead * 2).max(1_000_000);
        self.seek_ahead_of(landing.position, lead)?;
        Ok(true)
    }

    /// Like [`Self::seek`], but when `position` lies ahead within reach, decodes on to it
    /// rather than going back to a keyframe: stepping through a long group of pictures
    /// then decodes each frame once instead of once per step.
    pub fn advance(&mut self, position: Ticks) -> Result<(), MediaError> {
        if self.reaches_without_seek(position) {
            self.trim.advance(position);
            Ok(())
        } else {
            self.seek(position)
        }
    }

    /// Whether decoding on reaches `position` no later than a seek would: the keyframe a
    /// seek lands on is already behind, and the picture on screen there is still to come.
    pub fn reaches_without_seek(&self, position: Ticks) -> bool {
        let (Some(video), Some(decoded)) = (&self.video, self.decoded) else {
            return false;
        };
        let Some(rate) = self.info.video.as_ref().and_then(|info| info.frame_rate) else {
            return false;
        };
        if self.state != ReadState::Reading
            || rate.frame_at(decoded, self.timebase) >= rate.frame_at(position, self.timebase)
        {
            return false;
        }
        let target = (position + self.start).rescale(self.timebase, video.timebase);
        self.input
            .keyframe_before(video.index, target.0)
            .is_some_and(|keyframe| {
                Ticks(keyframe).rescale(video.timebase, self.timebase) - self.start <= decoded
            })
    }

    /// Returns frames in decode order, interleaving video and audio; `None` at end of media.
    pub fn next_frame(&mut self) -> Result<Option<DecodedFrame>, MediaError> {
        let mut packets = usize::MAX;
        loop {
            match self.next_frame_within(&mut packets)? {
                Progress::Frame(frame) => return Ok(Some(frame)),
                Progress::Ended => return Ok(None),
                Progress::Unfinished => {}
            }
        }
    }

    /// Like [`Self::next_frame`], but reads no more than `packets` packets, taking off
    /// those it reads. After a seek, the frames up to the target are decoded and dropped
    /// before one comes out, so this bounds that work too.
    pub fn next_frame_within(&mut self, packets: &mut usize) -> Result<Progress, MediaError> {
        loop {
            if self.retreat_if_landed_late()? {
                continue;
            }
            if let Some(trimmed) = self.trim.pop() {
                return self.finish(trimmed).map(Progress::Frame);
            }
            match self.state {
                ReadState::Finished => return Ok(Progress::Ended),
                ReadState::Draining => {
                    self.decode(None)?;
                    self.trim.finish();
                    self.state = ReadState::Finished;
                }
                ReadState::Reading => {
                    if *packets == 0 {
                        return Ok(Progress::Unfinished);
                    }
                    *packets -= 1;
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
                let picture = if video.converter.keeps_planes() {
                    match PlanarFrame::new(pts, picture) {
                        Ok(planar) => return Ok(DecodedFrame::Planar(planar)),
                        Err(picture) => picture,
                    }
                } else {
                    picture
                };
                let picture = picture.into_memory()?;
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
            #[cfg(windows)]
            video
                .decoder
                .set_keep_on_gpu(video.converter.keeps_planes());
            video.decoder.send_packet(packet)?;
            while let Received::Frame = video.decoder.receive_frame(&mut self.frame)? {
                let pts = clock.pts(&self.frame, video.timebase, video.next_pts);
                let picture = video.decoder.take_picture(&mut self.frame)?;
                video.next_pts = pts + self.picture_step;
                self.decoded = Some(pts);
                self.landing = self.landing.and_then(|landing| landing.after_picture(pts));
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
