use std::collections::VecDeque;
use std::path::PathBuf;

use audio::{Resampler, SampleRate};
use media::{DecodedFrame, MediaReader, Streams};
use timeline::{ChannelMap, Generator, MediaItem, SourceId, Ticks, Timebase};

use crate::PlaybackError;

/// Stereo, like everything the media reader decodes.
const CHANNELS: usize = 2;
/// Bars and Tone's reference tone: 1 kHz at -20 dBFS.
const TONE_HZ: f64 = 1_000.0;
const TONE_LEVEL: f32 = 0.1;

enum Source {
    None,
    Open(SourceId, PathBuf, Option<ChannelMap>, Box<MediaReader>),
    Failed(SourceId, PathBuf),
}

/// Decodes and resamples the audio one track needs, in frames at the output rate.
pub(crate) struct TrackAudio {
    timebase: Timebase,
    rate: SampleRate,
    source: Source,
    resampler: Option<(u32, Resampler)>,
    /// Resampled stereo frames, the first of which is source frame `next_frame`.
    buffered: VecDeque<f32>,
    next_frame: i64,
    ended: bool,
    scratch: Vec<f32>,
}

impl TrackAudio {
    pub(crate) fn new(timebase: Timebase, rate: SampleRate) -> Self {
        Self {
            timebase,
            rate,
            source: Source::None,
            resampler: None,
            buffered: VecDeque::new(),
            next_frame: i64::MIN,
            ended: false,
            scratch: Vec::new(),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.next_frame = i64::MIN;
    }

    /// Adds `out.len() / 2` stereo frames of `media`, starting at source frame `first`
    /// (counted at the output rate), into `out`. Missing audio is left silent.
    pub(crate) fn mix_into(
        &mut self,
        media: &MediaItem,
        first: i64,
        out: &mut [f32],
    ) -> Result<(), PlaybackError> {
        if media.offline {
            return Ok(());
        }
        match &media.generator {
            Some(Generator::BarsAndTone) => {
                add_reference_tone(self.rate, first, out, |_| true);
                return Ok(());
            }
            Some(Generator::CountingLeader(leader)) => {
                let rate = media.video.unwrap_or_default().frame_rate;
                let per_frame = |sample: i64| {
                    let frame = i128::from(sample) * i128::from(rate.num())
                        / (i128::from(self.rate.0) * i128::from(rate.den()));
                    leader.blips_on_frame(frame as i64, rate)
                };
                add_reference_tone(self.rate, first, out, per_frame);
                return Ok(());
            }
            _ => {}
        }
        let same_media = match &self.source {
            Source::Open(id, path, map, _) => {
                *id == media.id && *path == media.path && *map == media.channel_map
            }
            Source::Failed(id, path) => *id == media.id && *path == media.path,
            Source::None => false,
        };
        if !same_media {
            self.reset();
            self.source =
                match MediaReader::open_streams(&media.path, self.timebase, Streams::Audio) {
                    Ok(mut reader) => {
                        reader.set_channel_map(media.channel_map);
                        let path = media.path.clone();
                        Source::Open(media.id, path, media.channel_map, Box::new(reader))
                    }
                    Err(error) => {
                        self.source = Source::Failed(media.id, media.path.clone());
                        return Err(error.into());
                    }
                };
        }
        let Source::Open(_, _, _, reader) = &mut self.source else {
            return Ok(());
        };
        if first != self.next_frame {
            let output_rate =
                Timebase::new(1, self.rate.0).ok_or(audio::AudioError::InvalidFormat)?;
            reader.seek(Ticks(first).rescale(output_rate, self.timebase))?;
            if let Some((_, resampler)) = &mut self.resampler {
                resampler.reset();
            }
            self.buffered.clear();
            self.next_frame = first;
            self.ended = false;
        }
        let wanted = out.len();
        while self.buffered.len() < wanted && !self.ended {
            match reader.next_frame()? {
                Some(DecodedFrame::Audio(buffer)) => {
                    if self.resampler.as_ref().map(|(rate, _)| *rate) != Some(buffer.sample_rate) {
                        let resampler = Resampler::new(
                            SampleRate(buffer.sample_rate),
                            self.rate,
                            buffer.channels,
                        )?;
                        self.resampler = Some((buffer.sample_rate, resampler));
                    }
                    if let Some((_, resampler)) = &mut self.resampler {
                        resampler.process(&buffer.samples, &mut self.scratch)?;
                    }
                    self.buffered.extend(self.scratch.drain(..));
                }
                Some(DecodedFrame::Video(_) | DecodedFrame::Planar(_)) => {}
                None => {
                    self.ended = true;
                    if let Some((_, resampler)) = &mut self.resampler {
                        resampler.flush(&mut self.scratch)?;
                    }
                    self.buffered.extend(self.scratch.drain(..));
                }
            }
        }
        let available = self.buffered.len().min(wanted) / CHANNELS * CHANNELS;
        for (sample, value) in out.iter_mut().zip(self.buffered.drain(..available)) {
            *sample += value;
        }
        self.next_frame += i64::try_from(wanted / CHANNELS).unwrap_or(0);
        Ok(())
    }
}

/// Adds the tone to the sample frames from `first` that `sounding` picks.
fn add_reference_tone(
    rate: SampleRate,
    first: i64,
    out: &mut [f32],
    sounding: impl Fn(i64) -> bool,
) {
    let step = std::f64::consts::TAU * TONE_HZ / f64::from(rate.0);
    let frames = (first..).zip(out.as_chunks_mut::<2>().0);
    for (frame, pair) in frames.filter(|(frame, _)| sounding(*frame)) {
        let sample = TONE_LEVEL * (frame as f64 * step).sin() as f32;
        pair.iter_mut().for_each(|channel| *channel += sample);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use timeline::{CountingLeader, FrameRate, VideoFormat};

    fn fixture() -> MediaItem {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/testsrc_2s_25fps_44k.mp4");
        MediaItem {
            id: SourceId(1),
            name: "testsrc".into(),
            path,
            duration: Ticks(180_000),
            video: None,
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
            placed: None,
        }
    }

    fn read(track: &mut TrackAudio, first: i64, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * CHANNELS];
        track.mix_into(&fixture(), first, &mut out).unwrap();
        out
    }

    #[test]
    #[ignore = "decodes a real file with FFmpeg"]
    fn chunked_and_seeked_reads_match_one_continuous_read() {
        let new = || TrackAudio::new(Timebase::HZ_90K, SampleRate(48_000));
        let whole = read(&mut new(), 4800, 2048);
        assert!(whole.iter().any(|&sample| sample != 0.0));

        let mut chunked = new();
        let mut pieces = read(&mut chunked, 4800, 1024);
        pieces.extend(read(&mut chunked, 5824, 1024));
        assert_eq!(pieces, whole);

        // A seek restarts the decoder and resampler, so the first samples differ a little
        // while they settle; what matters is that the audio lines up to the frame.
        let mut seeked = new();
        read(&mut seeked, 40_000, 512);
        let tail = read(&mut seeked, 5824, 1024);
        let max_difference = |offset: usize| {
            tail[offset..]
                .iter()
                .zip(&whole[2048..])
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f32::max)
        };
        let one_frame_late = max_difference(CHANNELS);
        assert!(max_difference(0) < one_frame_late / 2.0);
    }

    #[test]
    fn bars_and_tone_plays_1_khz_at_minus_20_dbfs_from_any_frame() {
        let bars = MediaItem {
            generator: Some(Generator::BarsAndTone),
            ..fixture()
        };
        let mut track = TrackAudio::new(Timebase::HZ_90K, SampleRate(48_000));
        let mut out = vec![0.0; 48 * CHANNELS];
        // A whole number of cycles in, so a quarter cycle on is the crest.
        track.mix_into(&bars, 48_000, &mut out).unwrap();
        assert!((out[12 * CHANNELS] - 0.1).abs() < 1e-4);
        assert!((out[12 * CHANNELS + 1] - 0.1).abs() < 1e-4);
        assert!((out[36 * CHANNELS] + 0.1).abs() < 1e-4);
    }

    #[test]
    fn the_leader_pops_for_exactly_the_frame_its_two_shows() {
        let leader = MediaItem {
            generator: Some(Generator::CountingLeader(CountingLeader::default())),
            video: Some(VideoFormat {
                frame_rate: FrameRate::FPS_25,
                ..VideoFormat::default()
            }),
            ..fixture()
        };
        let mut track = TrackAudio::new(Timebase::HZ_90K, SampleRate(48_000));
        let sounding = |track: &mut TrackAudio, first| {
            let mut out = vec![0.0; CHANNELS];
            track.mix_into(&leader, first, &mut out).unwrap();
            out[0] != 0.0
        };
        // The 2 is frame 150, six seconds in; a frame is 1920 samples at 25 fps.
        let two = 6 * 48_000;
        assert!(!sounding(&mut track, two - 1));
        assert!(sounding(&mut track, two + 1));
        assert!(sounding(&mut track, two + 1919));
        assert!(!sounding(&mut track, two + 1920));
    }
}
