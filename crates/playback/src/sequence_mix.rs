use audio::SampleRate;
use timeline::{Sequence, Ticks, Timebase};

use crate::tracks_mix::TracksMix;
use crate::{Levels, PlaybackError};

/// Stereo frames mixed per step: small enough to start quickly after a seek, large
/// enough to keep per-chunk overhead low.
pub(crate) const CHUNK_FRAMES: usize = 1024;

/// Mixes every audio track of a sequence, nested sequences included, into interleaved
/// stereo at one output rate.
pub(crate) struct SequenceMix {
    tracks: TracksMix,
    /// The last chunk mixed.
    pub(crate) mix: Vec<f32>,
}

impl SequenceMix {
    pub(crate) fn new(sample_rate: SampleRate) -> Self {
        Self {
            tracks: TracksMix::new(sample_rate),
            mix: vec![0.0; CHUNK_FRAMES * 2],
        }
    }

    /// One tick per output frame.
    pub(crate) fn rate(&self) -> Timebase {
        self.tracks.rate()
    }

    /// Forgets decode positions, so the next chunk seeks.
    pub(crate) fn reset(&mut self) {
        self.tracks.reset();
    }

    /// Mixes the chunk starting at output frame `start` into `self.mix` and returns its
    /// levels. Media that can't be read is reported once and left silent.
    pub(crate) fn mix_chunk(
        &mut self,
        sequence: &Sequence,
        start: i64,
        warnings: &mut Vec<PlaybackError>,
    ) -> Levels {
        let tracks = (sequence.audio.as_slice(), sequence.master);
        let (tracks, master) = self
            .tracks
            .mix(sequence, tracks, start, &mut self.mix, warnings);
        Levels {
            pts: Ticks(start).rescale(self.rate(), sequence.timebase),
            tracks,
            master,
        }
    }
}
