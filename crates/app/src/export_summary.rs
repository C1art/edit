use media::{AudioCodec, RateControl};
use timeline::{FrameRate, Ticks, Timebase, Timecode};

use crate::export_format::ExportFormat;

pub fn span_length(range: std::ops::Range<Ticks>) -> Ticks {
    range.end - range.start
}

/// The export's length as timecode, and the file size it comes to when that follows
/// from the bitrates alone.
pub fn length_text(
    format: &ExportFormat,
    length: Ticks,
    (timebase, rate): (Timebase, FrameRate),
) -> String {
    let timecode = Timecode::new(length, timebase, rate);
    let video_bits = match (format.format.video_codec(), format.rate_control) {
        (None, _) => Some(0),
        (Some(codec), RateControl::Bitrate(bits)) if codec.is_rate_controlled() => {
            Some(u64::from(bits))
        }
        _ => None,
    };
    let audio_bits = match format.audio() {
        None => Some(0),
        Some(audio) if audio.codec.is_lossy() => Some(u64::from(format.audio_kbps) * 1_000),
        Some(_) => None,
    };
    let millis = Timebase::new(1, 1_000).map(|millis| length.rescale(timebase, millis).0);
    match (video_bits, audio_bits, millis) {
        (Some(video), Some(audio), Some(millis)) if video + audio > 0 => {
            let bytes = (video + audio) * millis.max(0) as u64 / 8_000;
            format!("{timecode}, about {}", crate::metadata_panel::size(bytes))
        }
        _ => timecode.to_string(),
    }
}

pub fn summary(format: &ExportFormat, sequence_size: (u32, u32)) -> String {
    let audio = format.audio().map(|audio| {
        let codec = audio_codec_name(audio.codec);
        if audio.codec.is_lossy() {
            format!("{codec} {} kbps", format.audio_kbps)
        } else {
            codec.to_owned()
        }
    });
    match (format.video(sequence_size), audio) {
        (Some(video), Some(audio)) => format!(
            "{} at {}×{}, with {audio} sound.",
            format.format.name(),
            video.size.0,
            video.size.1
        ),
        (Some(video), None) => format!(
            "{} at {}×{}, silent.",
            format.format.name(),
            video.size.0,
            video.size.1
        ),
        (None, Some(audio)) => format!("Sound only, {audio}."),
        (None, None) => String::new(),
    }
}

fn audio_codec_name(codec: AudioCodec) -> &'static str {
    match codec {
        AudioCodec::Aac => "AAC",
        AudioCodec::Mp3 => "MP3",
        AudioCodec::Flac => "FLAC",
        AudioCodec::Pcm16 => "16-bit PCM",
        AudioCodec::Pcm24 => "24-bit PCM",
    }
}
