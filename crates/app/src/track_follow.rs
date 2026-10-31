//! State the editor keeps per track outside the sequence is keyed by where the track
//! is, so it follows the track when tracks are added or deleted around it, by edits,
//! undo or redo alike.

use timeline::{Sequence, TrackId, TrackKind, TrackRef};

use crate::editor::Editor;

const KINDS: [TrackKind; 2] = [TrackKind::Video, TrackKind::Audio];

/// Each kind's tracks, bottom video and top audio first, as last seen.
#[derive(Debug, Default)]
pub(crate) struct TrackOrder([Vec<TrackId>; 2]);

/// Where each track last seen is now, by its old place.
pub(crate) struct TrackMoves {
    moved: [Vec<Option<usize>>; 2],
    counts: [usize; 2],
}

impl TrackOrder {
    fn of(sequence: &Sequence) -> Self {
        Self(KINDS.map(|kind| {
            sequence
                .tracks_of(kind)
                .iter()
                .map(|track| track.id)
                .collect()
        }))
    }

    /// Takes the sequence's tracks as the order from now on. None when no track moved,
    /// or for another sequence with no track in common, whose state stays by place.
    fn update(&mut self, sequence: &Sequence) -> Option<TrackMoves> {
        let old = std::mem::replace(self, Self::of(sequence));
        if old.0 == self.0 {
            return None;
        }
        let new = &self.0;
        let shared = old
            .0
            .iter()
            .flatten()
            .any(|id| new.iter().flatten().any(|other| other == id));
        if !shared {
            return None;
        }
        Some(TrackMoves {
            moved: std::array::from_fn(|kind| {
                let place = |id| new[kind].iter().position(|other| other == id);
                old.0[kind].iter().map(place).collect()
            }),
            counts: std::array::from_fn(|kind| new[kind].len()),
        })
    }
}

fn slot(kind: TrackKind) -> usize {
    match kind {
        TrackKind::Video => 0,
        TrackKind::Audio => 1,
    }
}

impl TrackMoves {
    /// Where `track` is now; None once it's deleted.
    pub(crate) fn of(&self, track: TrackRef) -> Option<TrackRef> {
        let index = (*self.moved[slot(track.kind)].get(track.index)?)?;
        Some(TrackRef { index, ..track })
    }

    /// Where `track` is now, or the track that took its place once it's deleted.
    pub(crate) fn or_in_place(&self, track: TrackRef) -> Option<TrackRef> {
        let count = self.counts[slot(track.kind)];
        let in_place = (count > 0).then(|| TrackRef {
            index: track.index.min(count - 1),
            ..track
        });
        self.of(track).or(in_place)
    }
}

impl Editor {
    pub(crate) fn follow_tracks(&mut self) {
        if let Some(moves) = self.track_order.update(self.timeline.sequence()) {
            self.view.follow_tracks(&moves);
            self.mixer.follow_tracks(&moves);
        }
    }
}

#[cfg(test)]
mod tests {
    use timeline::{Sequence, Timebase, Track};

    use super::*;

    #[test]
    fn state_follows_tracks_past_a_deleted_one() {
        let mut sequence = Sequence::new(Timebase::new(1, 25).unwrap(), Default::default());
        sequence.audio = vec![Track::default(), Track::default(), Track::default()];
        let mut order = TrackOrder::default();
        order.update(&sequence);
        sequence.audio.remove(1);
        let moves = order.update(&sequence).unwrap();
        let moved = [0, 1, 2].map(|index| moves.of(TrackRef::audio(index)));
        assert_eq!(
            moved,
            [Some(TrackRef::audio(0)), None, Some(TrackRef::audio(1))]
        );
        assert_eq!(
            moves.or_in_place(TrackRef::audio(1)),
            Some(TrackRef::audio(1))
        );
        assert!(order.update(&sequence).is_none());
    }
}
