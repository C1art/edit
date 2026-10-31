use std::path::Path;
use std::time::{Duration, Instant};

use timeline::{Chapter, FrameRate, Timebase};

use crate::audio_encoding::AudioEncoding;
use crate::ffi::{Converter, Encoder, Frame, OutputContext, Packet, Received};
use crate::{Acceleration, AudioCodec, MediaError, RateControl, VideoCodec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoSettings {
    /// Size of the pictures given to [`MediaWriter::write_video`], scaled to the
    /// encoded size when it differs.
    pub input_size: (u32, u32),
    /// Both even, as 4:2:0 chroma needs.
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub codec: VideoCodec,
    pub rate_control: RateControl,
    pub profile: EncodeProfile,
    pub acceleration: Acceleration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EncodeProfile {
    /// Compresses well for viewing and sharing.
    #[default]
    Delivery,
    /// Encodes quickly and seeks cheaply, with a keyframe every half second, for proxies.
    /// Applies to H.264 and HEVC only.
    Editing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSettings {
    pub codec: AudioCodec,
    pub sample_rate: u32,
    /// Ignored by lossless codecs.
    pub bits_per_second: u32,
}

struct VideoStream {
    encoder: Encoder,
    index: usize,
    frame: Frame,
    converter: Converter,
    next_pts: i64,
    converting: Duration,
}

/// Encodes pictures and sound into a file, in the container the file extension names. Call [`MediaWriter::finish`] to make the file playable.
pub struct MediaWriter {
    output: OutputContext,
    packet: Packet,
    video: Option<VideoStream>,
    audio: Option<AudioEncoding>,
}

impl MediaWriter {
    pub fn create(
        path: &Path,
        video: Option<VideoSettings>,
        audio: Option<AudioSettings>,
    ) -> Result<Self, MediaError> {
        Self::with_chapters(path, video, audio, &[], Timebase::HZ_90K)
    }

    /// As [`MediaWriter::create`], marking `chapters` timed in `timebase` in the file.
    pub fn with_chapters(
        path: &Path,
        video: Option<VideoSettings>,
        audio: Option<AudioSettings>,
        chapters: &[Chapter],
        timebase: Timebase,
    ) -> Result<Self, MediaError> {
        if video.is_none() && audio.is_none() {
            return Err(MediaError::InvalidSettings("nothing to export"));
        }
        let mut output = OutputContext::create(path)?;
        let global_header = output.needs_global_header();
        let video = video
            .map(|settings| {
                if !settings.width.is_multiple_of(2) || !settings.height.is_multiple_of(2) {
                    return Err(MediaError::InvalidSettings("frame size must be even"));
                }
                let size = (settings.width, settings.height);
                let encoder = Encoder::video(&settings, global_header)?;
                Ok(VideoStream {
                    index: output.add_stream(&encoder)?,
                    frame: encoder.video_frame(size)?,
                    converter: Converter::new(&settings, &encoder)?,
                    encoder,
                    next_pts: 0,
                    converting: Duration::ZERO,
                })
            })
            .transpose()?;
        let audio = audio
            .map(|settings| {
                let encoder = Encoder::audio(&settings, global_header)?;
                let index = output.add_stream(&encoder)?;
                AudioEncoding::start(encoder, index, settings.sample_rate)
            })
            .transpose()?;
        output.set_chapters(chapters, timebase)?;
        output.write_header()?;
        Ok(Self {
            output,
            packet: Packet::new()?,
            video,
            audio,
        })
    }

    /// What encodes the video, such as "NVIDIA NVENC (hardware)".
    pub fn video_encoder(&self) -> Option<String> {
        self.video.as_ref().map(|stream| stream.encoder.describe())
    }

    /// Encodes the next picture: tightly packed, sRGB-encoded RGBA8 at the export size.
    pub fn write_video(&mut self, rgba: &[u8]) -> Result<(), MediaError> {
        let stream = self
            .video
            .as_mut()
            .ok_or(MediaError::InvalidSettings("export has no video"))?;
        let converting = Instant::now();
        stream.converter.convert(rgba, &mut stream.frame)?;
        stream.converting += converting.elapsed();
        stream.frame.set_timing(stream.next_pts, 1);
        stream.next_pts += 1;
        stream.encoder.send_frame(Some(&stream.frame))?;
        drain(&mut self.output, &mut self.packet, stream)?;
        self.mux_audio()
    }

    /// How long turning pictures into the encoder's pixel format has taken so far, the
    /// rest of writing them being the encoding.
    pub fn converting_time(&self) -> Duration {
        self.video
            .as_ref()
            .map_or(Duration::ZERO, |stream| stream.converting)
    }

    /// Encodes the next stretch of interleaved stereo at the export sample rate.
    pub fn write_audio(&mut self, stereo: &[f32]) -> Result<(), MediaError> {
        self.audio
            .as_mut()
            .ok_or(MediaError::InvalidSettings("export has no audio"))?
            .send(stereo.to_vec())?;
        self.mux_audio()
    }

    /// Muxes the sound encoded so far.
    fn mux_audio(&mut self) -> Result<(), MediaError> {
        let Some(audio) = &self.audio else {
            return Ok(());
        };
        for packet in audio.ready() {
            self.output
                .write_packet(&mut packet?, audio.index, audio.timebase)?;
        }
        Ok(())
    }

    /// Flushes the encoders and writes the container's index.
    pub fn finish(mut self) -> Result<(), MediaError> {
        if let Some(audio) = &mut self.audio {
            let (index, timebase) = (audio.index, audio.timebase);
            for packet in audio.finish() {
                self.output.write_packet(&mut packet?, index, timebase)?;
            }
        }
        if let Some(stream) = &mut self.video {
            stream.encoder.send_frame(None)?;
            drain(&mut self.output, &mut self.packet, stream)?;
        }
        self.output.write_trailer()
    }
}

/// Moves every picture packet the encoder has ready into the muxer.
fn drain(
    output: &mut OutputContext,
    packet: &mut Packet,
    stream: &mut VideoStream,
) -> Result<(), MediaError> {
    let timebase = stream.encoder.packet_timebase();
    loop {
        match stream.encoder.receive_packet(packet)? {
            Received::Frame => {
                // Every video packet is one frame, one tick of the encoder timebase.
                packet.fill_duration(1);
                output.write_packet(packet, stream.index, timebase)?;
            }
            Received::NeedsInput | Received::Drained => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MediaReader;
    use timeline::{Ticks, Timebase};

    #[test]
    #[ignore = "encodes and decodes real files with FFmpeg"]
    fn every_format_reads_back_with_matching_duration() {
        let video = |codec, width| VideoSettings {
            input_size: (64, 36),
            width,
            height: width * 9 / 16,
            frame_rate: FrameRate::FPS_25,
            codec,
            rate_control: RateControl::Bitrate(500_000),
            profile: EncodeProfile::Delivery,
            acceleration: Acceleration::Software,
        };
        let audio = |codec| AudioSettings {
            codec,
            sample_rate: 48_000,
            bits_per_second: 128_000,
        };
        let cases = [
            (
                "h264.mp4",
                Some(video(VideoCodec::H264, 64)),
                Some(audio(AudioCodec::Aac)),
            ),
            ("hevc.mp4", Some(video(VideoCodec::Hevc, 32)), None),
            (
                "vp9.webm",
                Some(video(VideoCodec::Vp9, 64)),
                Some(audio(AudioCodec::Opus)),
            ),
            ("av1.mp4", Some(video(VideoCodec::Av1, 64)), None),
            (
                "prores.mov",
                Some(video(VideoCodec::ProRes422Hq, 128)),
                Some(audio(AudioCodec::Pcm24)),
            ),
            (
                "dnxhr.mov",
                Some(video(VideoCodec::Dnxhr(crate::DnxhrQuality::Hqx), 256)),
                Some(audio(AudioCodec::Pcm24)),
            ),
            ("animated.gif", Some(video(VideoCodec::Gif, 32)), None),
            ("sound.wav", None, Some(audio(AudioCodec::Pcm16))),
            ("sound.flac", None, Some(audio(AudioCodec::Flac))),
            ("sound.mp3", None, Some(audio(AudioCodec::Mp3))),
        ];
        let dir = std::env::temp_dir().join(format!("media-writer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (name, video, audio) in cases {
            let path = dir.join(name);
            let mut writer = MediaWriter::create(&path, video, audio).unwrap();
            let picture = vec![128u8; 64 * 36 * 4];
            for _ in 0..25 {
                if video.is_some() {
                    writer.write_video(&picture).unwrap();
                }
                if audio.is_some() {
                    writer.write_audio(&[0.25; 1920 * 2]).unwrap();
                }
            }
            writer.finish().unwrap();
            let reader = MediaReader::open(&path, Timebase::HZ_90K).unwrap();
            let duration = reader.info().duration.unwrap();
            // One second, give or take encoder priming and frame padding.
            assert!(
                (duration - Ticks(90_000)).0.abs() < 4_500,
                "{name}: {duration:?}"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[ignore = "encodes and decodes a real file with FFmpeg"]
    fn gif_frames_keep_their_own_colors() {
        let video = VideoSettings {
            input_size: (64, 36),
            width: 64,
            height: 36,
            frame_rate: FrameRate::FPS_25,
            codec: VideoCodec::Gif,
            rate_control: RateControl::Quality(0),
            profile: EncodeProfile::Delivery,
            acceleration: Acceleration::Software,
        };
        let path = std::env::temp_dir().join(format!("media-gif-{}.gif", std::process::id()));
        let mut writer = MediaWriter::create(&path, Some(video), None).unwrap();
        let colors = [
            [200u8, 30, 30],
            [30, 30, 200],
            [30, 30, 200],
            [240, 240, 60],
            // Back to the first picture's palette, with the same indices as the last.
            [200, 30, 30],
        ];
        for [r, g, b] in colors {
            writer.write_video(&[r, g, b, 255].repeat(64 * 36)).unwrap();
        }
        writer.finish().unwrap();
        let mut reader = MediaReader::open(&path, Timebase::HZ_90K).unwrap();
        for wanted in colors {
            let Some(crate::DecodedFrame::Video(frame)) = reader.next_frame().unwrap() else {
                panic!("too few pictures");
            };
            let middle = (18 * 64 + 32) * 4;
            let got = &frame.pixels[middle..middle + 3];
            let off = got.iter().zip(wanted).map(|(&a, b)| a.abs_diff(b)).max();
            assert!(off < Some(4), "{got:?} for {wanted:?}");
        }
        // Windows won't remove a file that is still open.
        drop(reader);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    #[ignore = "encodes and decodes a real file with FFmpeg"]
    fn prores_4444_keeps_transparency() {
        let video = VideoSettings {
            input_size: (64, 36),
            width: 64,
            height: 36,
            frame_rate: FrameRate::FPS_25,
            codec: VideoCodec::ProRes4444,
            rate_control: RateControl::Quality(0),
            profile: EncodeProfile::Delivery,
            acceleration: Acceleration::Software,
        };
        let path = std::env::temp_dir().join(format!("media-alpha-{}.mov", std::process::id()));
        let mut writer = MediaWriter::create(&path, Some(video), None).unwrap();
        // The left half clear, the right half opaque.
        let picture: Vec<u8> = (0..64 * 36)
            .flat_map(|pixel| match pixel % 64 < 32 {
                true => [0, 0, 0, 0],
                false => [200, 100, 50, 255],
            })
            .collect();
        for _ in 0..5 {
            writer.write_video(&picture).unwrap();
        }
        writer.finish().unwrap();
        let mut reader = MediaReader::open(&path, Timebase::HZ_90K).unwrap();
        let Some(crate::DecodedFrame::Video(frame)) = reader.next_frame().unwrap() else {
            panic!("no picture");
        };
        let alpha = |x: usize| frame.pixels[(18 * 64 + x) * 4 + 3];
        assert!(alpha(8) < 4, "{}", alpha(8));
        assert!(alpha(56) > 251, "{}", alpha(56));
        // Windows won't remove a file that is still open.
        drop(reader);
        std::fs::remove_file(&path).unwrap();
    }
}
