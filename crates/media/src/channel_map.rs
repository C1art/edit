use timeline::ChannelMap;

/// Interleaved stereo taken from the `map`ped channels of `channels`-wide interleaved
/// `samples`; a channel the source doesn't have plays silence.
pub(crate) fn mapped(samples: &[f32], channels: u16, map: ChannelMap) -> Vec<f32> {
    let width = usize::from(channels.max(1));
    let pick =
        |frame: &[f32], channel: u16| frame.get(usize::from(channel)).copied().unwrap_or(0.0);
    samples
        .chunks_exact(width)
        .flat_map(|frame| [pick(frame, map.left), pick(frame, map.right)])
        .collect()
}

#[cfg(test)]
mod tests {
    use timeline::Timebase;

    use super::*;
    use crate::{DecodedFrame, MediaReader};

    /// A 16-bit PCM WAV file of `channels` channels, each holding its own steady level.
    fn wav(levels: &[i16], frames: u32) -> Vec<u8> {
        let channels = levels.len() as u16;
        let rate = 48_000_u32;
        let data = frames * u32::from(channels) * 2;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data.to_le_bytes());
        for _ in 0..frames {
            levels
                .iter()
                .for_each(|level| bytes.extend_from_slice(&level.to_le_bytes()));
        }
        bytes
    }

    #[test]
    #[ignore = "decodes a real file with FFmpeg"]
    fn a_mapped_reader_plays_the_chosen_channels_of_a_wide_file() {
        let path = std::env::temp_dir().join("edit-channel-map-test.wav");
        std::fs::write(&path, wav(&[1_000, 2_000, 3_000, 4_000], 4_800)).unwrap();
        let mut reader = MediaReader::open(&path, Timebase::HZ_90K).unwrap();
        reader.set_channel_map(Some(ChannelMap { left: 2, right: 0 }));
        let Some(DecodedFrame::Audio(buffer)) = reader.next_frame().unwrap() else {
            panic!("no sound");
        };
        std::fs::remove_file(&path).unwrap();
        assert_eq!(buffer.channels, 2);
        let level = |value: i16| f32::from(value) / 32_768.0;
        assert!(
            (buffer.samples[0] - level(3_000)).abs() < 1e-4,
            "{:?}",
            &buffer.samples[..2]
        );
        assert!(
            (buffer.samples[1] - level(1_000)).abs() < 1e-4,
            "{:?}",
            &buffer.samples[..2]
        );
    }

    #[test]
    fn mapping_picks_each_side_from_its_channel_or_silence() {
        // Four channels, each frame numbering its channels by tens.
        let samples = [1.0, 2.0, 3.0, 4.0, 11.0, 12.0, 13.0, 14.0];
        let swapped_mono = ChannelMap { left: 2, right: 2 };
        assert_eq!(mapped(&samples, 4, swapped_mono), [3.0, 3.0, 13.0, 13.0]);
        let beyond = ChannelMap { left: 1, right: 7 };
        assert_eq!(mapped(&samples, 4, beyond), [2.0, 0.0, 12.0, 0.0]);
    }
}
