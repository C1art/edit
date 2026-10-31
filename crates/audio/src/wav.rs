use std::io::{self, Seek, SeekFrom, Write};

use crate::SampleRate;

const HEADER_BYTES: u32 = 44;
const BYTES_PER_SAMPLE: u32 = 3;

/// Writes mono 24-bit PCM WAV as samples arrive, so a long recording never has to
/// be held whole. The sizes in the header are filled in by [`finish`](Self::finish).
pub struct WavWriter<W: Write + Seek> {
    out: W,
    data_bytes: u32,
}

impl<W: Write + Seek> WavWriter<W> {
    pub fn new(mut out: W, sample_rate: SampleRate) -> io::Result<Self> {
        let block_align = BYTES_PER_SAMPLE as u16;
        let mut header = Vec::with_capacity(HEADER_BYTES as usize);
        header.extend_from_slice(b"RIFF");
        header.extend_from_slice(&0u32.to_le_bytes());
        header.extend_from_slice(b"WAVEfmt ");
        header.extend_from_slice(&16u32.to_le_bytes());
        // PCM, one channel.
        header.extend_from_slice(&1u16.to_le_bytes());
        header.extend_from_slice(&1u16.to_le_bytes());
        header.extend_from_slice(&sample_rate.0.to_le_bytes());
        header.extend_from_slice(&(sample_rate.0 * BYTES_PER_SAMPLE).to_le_bytes());
        header.extend_from_slice(&block_align.to_le_bytes());
        header.extend_from_slice(&(BYTES_PER_SAMPLE as u16 * 8).to_le_bytes());
        header.extend_from_slice(b"data");
        header.extend_from_slice(&0u32.to_le_bytes());
        out.write_all(&header)?;
        Ok(Self { out, data_bytes: 0 })
    }

    pub fn write(&mut self, samples: &[f32]) -> io::Result<()> {
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|&sample| {
                let [low, middle, high, _] =
                    ((sample.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32).to_le_bytes();
                [low, middle, high]
            })
            .collect();
        self.out.write_all(&bytes)?;
        let written = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        // A WAV file can't hold more than 4 GiB; past that the sizes stop growing.
        self.data_bytes = self.data_bytes.saturating_add(written);
        Ok(())
    }

    /// Fills in the sizes and hands back the output.
    pub fn finish(mut self) -> io::Result<W> {
        // RIFF chunks take an even number of bytes; the pad isn't counted as data.
        let pad = self.data_bytes % 2;
        if pad == 1 {
            self.out.write_all(&[0])?;
        }
        let riff_bytes = self
            .data_bytes
            .saturating_add(pad)
            .saturating_add(HEADER_BYTES - 8);
        self.out.seek(SeekFrom::Start(4))?;
        self.out.write_all(&riff_bytes.to_le_bytes())?;
        self.out
            .seek(SeekFrom::Start(u64::from(HEADER_BYTES) - 4))?;
        self.out.write_all(&self.data_bytes.to_le_bytes())?;
        self.out.seek(SeekFrom::End(0))?;
        self.out.flush()?;
        Ok(self.out)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn header_sizes_cover_what_was_written_and_samples_are_24_bit() {
        let mut writer = WavWriter::new(Cursor::new(Vec::new()), SampleRate(48_000)).unwrap();
        writer.write(&[1.0, -1.0]).unwrap();
        writer.write(&[0.5, 2.0]).unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(bytes.len(), 44 + 4 * 3);
        assert_eq!(word(4), bytes.len() as u32 - 8);
        assert_eq!(word(28), 48_000 * 3);
        assert_eq!(word(40), 4 * 3);
        assert_eq!(&bytes[44..50], &[0xff, 0xff, 0x7f, 0x01, 0x00, 0x80]);
        // Over full scale is clipped rather than wrapped.
        assert_eq!(&bytes[53..56], &[0xff, 0xff, 0x7f]);
    }

    #[test]
    fn odd_data_is_padded_to_an_even_chunk() {
        let mut writer = WavWriter::new(Cursor::new(Vec::new()), SampleRate(48_000)).unwrap();
        writer.write(&[0.0]).unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(bytes.len(), 44 + 4);
        assert_eq!(word(4), bytes.len() as u32 - 8);
        assert_eq!(word(40), 3);
    }
}
