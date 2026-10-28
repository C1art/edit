use std::path::Path;

use timeline::FrameRate;

use crate::ffi::{Encoder, Frame, OutputContext, Packet, Received, RgbaToYuv};
use crate::MediaError;

const STEREO: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoSettings {
    /// Both even, as 4:2:0 chroma needs.
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    /// x264 constant rate factor: lower is better and larger; 18 looks lossless on
    /// most material.
    pub crf: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSettings {
    pub sample_rate: u32,
    pub bits_per_second: u32,
}

struct VideoStream {
    encoder: Encoder,
    index: usize,
    frame: Frame,
    converter: RgbaToYuv,
    next_pts: i64,
}

struct AudioStream {
    encoder: Encoder,
    index: usize,
    frame: Frame,
    frame_size: usize,
    /// Interleaved stereo waiting to fill a whole encoder frame.
    pending: Vec<f32>,
    next_pts: i64,
}

/// Encodes pictures and sound into a file: H.264 and AAC, in the container the file
/// extension names. Call [`MediaWriter::finish`] to make the file playable.
pub struct MediaWriter {
    output: OutputContext,
    packet: Packet,
    video: Option<VideoStream>,
    audio: Option<AudioStream>,
}

impl MediaWriter {
    pub fn create(
        path: &Path,
        video: Option<VideoSettings>,
        audio: Option<AudioSettings>,
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
                let rate = (settings.frame_rate.num(), settings.frame_rate.den());
                let encoder = Encoder::video(size, rate, settings.crf, global_header)?;
                Ok(VideoStream {
                    index: output.add_stream(&encoder)?,
                    encoder,
                    frame: Frame::for_video(size)?,
                    converter: RgbaToYuv::new(size)?,
                    next_pts: 0,
                })
            })
            .transpose()?;
        let audio = audio
            .map(|settings| {
                let encoder = Encoder::audio(
                    settings.sample_rate,
                    settings.bits_per_second,
                    global_header,
                )?;
                // Encoders taking any size get about 20 ms per frame.
                let frame_size = encoder
                    .frame_size()
                    .unwrap_or(settings.sample_rate as usize / 50);
                Ok::<_, MediaError>(AudioStream {
                    index: output.add_stream(&encoder)?,
                    frame: Frame::for_audio(settings.sample_rate, frame_size)?,
                    encoder,
                    frame_size,
                    pending: Vec::with_capacity(frame_size * STEREO * 2),
                    next_pts: 0,
                })
            })
            .transpose()?;
        output.write_header()?;
        Ok(Self {
            output,
            packet: Packet::new()?,
            video,
            audio,
        })
    }

    /// Encodes the next picture: tightly packed, sRGB-encoded RGBA8 at the export size.
    pub fn write_video(&mut self, rgba: &[u8]) -> Result<(), MediaError> {
        let stream = self
            .video
            .as_mut()
            .ok_or(MediaError::InvalidSettings("export has no video"))?;
        stream.converter.convert(rgba, &mut stream.frame)?;
        stream.frame.set_timing(stream.next_pts, 1);
        stream.next_pts += 1;
        stream.encoder.send_frame(Some(&stream.frame))?;
        drain(&mut self.output, &mut self.packet, stream.as_target())
    }

    /// Encodes the next stretch of interleaved stereo at the export sample rate.
    pub fn write_audio(&mut self, stereo: &[f32]) -> Result<(), MediaError> {
        let stream = self
            .audio
            .as_mut()
            .ok_or(MediaError::InvalidSettings("export has no audio"))?;
        stream.pending.extend_from_slice(stereo);
        let whole = stream.frame_size * STEREO;
        let mut offset = 0;
        while stream.pending.len() - offset >= whole {
            encode_audio(
                &mut self.output,
                &mut self.packet,
                stream,
                offset..offset + whole,
            )?;
            offset += whole;
        }
        stream.pending.drain(..offset);
        Ok(())
    }

    /// Flushes the encoders and writes the container's index.
    pub fn finish(mut self) -> Result<(), MediaError> {
        if let Some(stream) = &mut self.audio {
            if !stream.pending.is_empty() {
                let all = 0..stream.pending.len();
                encode_audio(&mut self.output, &mut self.packet, stream, all)?;
                stream.pending.clear();
            }
            stream.encoder.send_frame(None)?;
            drain(&mut self.output, &mut self.packet, stream.as_target())?;
        }
        if let Some(stream) = &mut self.video {
            stream.encoder.send_frame(None)?;
            drain(&mut self.output, &mut self.packet, stream.as_target())?;
        }
        self.output.write_trailer()
    }
}

fn encode_audio(
    output: &mut OutputContext,
    packet: &mut Packet,
    stream: &mut AudioStream,
    range: std::ops::Range<usize>,
) -> Result<(), MediaError> {
    let samples = &stream.pending[range];
    stream.frame.fill_stereo(samples, stream.frame_size)?;
    let frames = (samples.len() / STEREO) as i64;
    stream.frame.set_timing(stream.next_pts, frames);
    stream.next_pts += frames;
    stream.encoder.send_frame(Some(&stream.frame))?;
    drain(output, packet, stream.as_target())
}

/// An encoder and where its packets go.
struct Target<'a> {
    encoder: &'a mut Encoder,
    index: usize,
    /// Duration to give packets the encoder left without one, in its timebase.
    packet_duration: Option<i64>,
}

impl VideoStream {
    fn as_target(&mut self) -> Target<'_> {
        Target {
            encoder: &mut self.encoder,
            index: self.index,
            // Every video packet is one frame, one tick of the encoder timebase.
            packet_duration: Some(1),
        }
    }
}

impl AudioStream {
    fn as_target(&mut self) -> Target<'_> {
        Target {
            encoder: &mut self.encoder,
            index: self.index,
            packet_duration: None,
        }
    }
}

/// Moves every packet the encoder has ready into the muxer.
fn drain(
    output: &mut OutputContext,
    packet: &mut Packet,
    target: Target<'_>,
) -> Result<(), MediaError> {
    loop {
        match target.encoder.receive_packet(packet)? {
            Received::Frame => {
                if let Some(duration) = target.packet_duration {
                    packet.fill_duration(duration);
                }
                output.write_packet(packet, target.index, target.encoder.timebase())?;
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
    #[ignore = "encodes and decodes a real file with FFmpeg"]
    fn written_file_reads_back_with_matching_duration() {
        let dir = std::env::temp_dir().join(format!("media-writer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("out.mp4");
        let video = VideoSettings {
            width: 64,
            height: 36,
            frame_rate: FrameRate::FPS_25,
            crf: 23,
        };
        let audio = AudioSettings {
            sample_rate: 48_000,
            bits_per_second: 128_000,
        };
        let mut writer = MediaWriter::create(&path, Some(video), Some(audio)).unwrap();
        let picture = vec![128u8; 64 * 36 * 4];
        for _ in 0..25 {
            writer.write_video(&picture).unwrap();
            writer.write_audio(&[0.25; 1920 * STEREO]).unwrap();
        }
        writer.finish().unwrap();

        let reader = MediaReader::open(&path, Timebase::HZ_90K).unwrap();
        let duration = reader.info().duration.unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        // One second, give or take the AAC priming and frame padding.
        assert!((duration - Ticks(90_000)).0.abs() < 4_500, "{duration:?}");
    }
}
