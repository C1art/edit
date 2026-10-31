use std::sync::Arc;

use audio::SampleRate;
use timeline::{Clip, ClipId, Sequence, SourceId, Ticks, Track};

use crate::sequence_mix::{SequenceMix, CHUNK_FRAMES};
use crate::{Piece, PlaybackError};

/// A piece of a sequence mixed to mono a stretch at a time, so a long piece never has
/// to be held whole.
pub struct MonoMix {
    piece: Piece,
    mix: SequenceMix,
    next: i64,
    span: std::ops::Range<i64>,
}

impl MonoMix {
    pub fn new(piece: Piece, rate: SampleRate) -> Self {
        let mix = SequenceMix::new(rate);
        let to_frames = |time: Ticks| time.rescale(piece.0.timebase, mix.rate()).0;
        let span = to_frames(piece.1.start)..to_frames(piece.1.end);
        Self {
            piece,
            mix,
            next: span.start,
            span,
        }
    }

    /// Appends at least `frames` more samples to `into`, fewer only at the end, and
    /// returns how many. Media that can't be read is reported and left silent.
    pub fn read(
        &mut self,
        frames: usize,
        into: &mut Vec<f32>,
        warnings: &mut Vec<PlaybackError>,
    ) -> usize {
        let before = into.len();
        while into.len() - before < frames && self.next < self.span.end {
            self.mix.mix_chunk(&self.piece.0, self.next, warnings);
            let length = (self.span.end - self.next).min(CHUNK_FRAMES as i64);
            let stereo = &self.mix.mix[..usize::try_from(length).unwrap_or(0) * 2];
            into.extend(
                stereo
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|[left, right]| (left + right) * 0.5),
            );
            self.next += length;
        }
        into.len() - before
    }

    /// Samples in the whole piece.
    pub fn len(&self) -> usize {
        usize::try_from(self.span.end - self.span.start).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.span.is_empty()
    }
}

/// The sound of media item `source` alone from its first tick to its last, at unity
/// and its own rate, so the piece's time is the file's.
pub fn media_alone(sequence: &Sequence, source: SourceId) -> Option<Piece> {
    let item = sequence.media_item(source)?;
    if !item.has_audio || !item.is_file() {
        return None;
    }
    let mut alone = Sequence::new(sequence.timebase, sequence.format);
    alone.media.clone_from(&sequence.media);
    alone.audio.push(Track {
        clips: vec![Arc::new(Clip::new(
            ClipId(1),
            source,
            Ticks::ZERO,
            item.duration,
            Ticks::ZERO,
        ))],
        ..Track::default()
    });
    Some((Arc::new(alone), Ticks::ZERO..item.duration))
}
