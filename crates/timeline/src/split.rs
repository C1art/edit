use crate::{Border, Reveal, Side, Sweep, Wipe, WipePattern};

/// A picture cut into pieces that move apart, each drawn where it has moved to.
/// Distances are fractions of the canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Split {
    /// The halves either side of a line through the middle, down it when `vertical`,
    /// each moved `apart` away from it.
    Halves { vertical: bool, apart: f32 },
    /// The quarters, each moved `apart` toward its corner both ways.
    Quarters { apart: f32 },
    /// `count` bands, running down the picture when `vertical`, moved along their
    /// length by `shift`, alternate ones the opposite way.
    Bands {
        count: u8,
        vertical: bool,
        shift: f32,
    },
}

impl Split {
    fn is_closed(&self) -> bool {
        match *self {
            Self::Halves { apart, .. } | Self::Quarters { apart } => apart == 0.0,
            Self::Bands { shift, count, .. } => shift == 0.0 || count == 0,
        }
    }
}

impl Reveal {
    /// The pieces the picture is drawn as, each a whole reveal of its own; one when it
    /// isn't split.
    pub fn pieces(self) -> Vec<Reveal> {
        let whole = Self {
            split: None,
            ..self
        };
        let Some(split) = self.split.filter(|split| !split.is_closed()) else {
            return vec![whole];
        };
        match split {
            Split::Halves {
                vertical: true,
                apart,
            } => vec![
                whole.piece([0.0, 0.0, 0.5, 1.0], [-apart, 0.0]),
                whole.piece([0.5, 0.0, 1.0, 1.0], [apart, 0.0]),
            ],
            Split::Halves {
                vertical: false,
                apart,
            } => vec![
                whole.piece([0.0, 0.0, 1.0, 0.5], [0.0, -apart]),
                whole.piece([0.0, 0.5, 1.0, 1.0], [0.0, apart]),
            ],
            Split::Quarters { apart } => [(0.0, -1.0), (0.5, 1.0)]
                .into_iter()
                .flat_map(|across| [(0.0, -1.0), (0.5, 1.0)].map(|down| (across, down)))
                .map(|((left, x), (top, y))| {
                    whole.piece([left, top, left + 0.5, top + 0.5], [x * apart, y * apart])
                })
                .collect(),
            Split::Bands {
                count,
                vertical,
                shift,
            } => (0..count)
                .map(|band| {
                    let (start, end) = (
                        f32::from(band) / f32::from(count),
                        f32::from(band + 1) / f32::from(count),
                    );
                    let along = if band % 2 == 0 { shift } else { -shift };
                    match vertical {
                        true => whole.piece([start, 0.0, end, 1.0], [0.0, along]),
                        false => whole.piece([0.0, start, 1.0, end], [along, 0.0]),
                    }
                })
                .collect(),
        }
    }

    /// The part of the picture inside `[left, top, right, bottom]`, moved by `shift`.
    fn piece(self, [left, top, right, bottom]: [f32; 4], [x, y]: [f32; 2]) -> Self {
        let edge = |from, progress| Wipe {
            pattern: WipePattern::Edge,
            from,
            progress,
            sweep: Sweep::Forward,
            border: Border::NONE,
            softness: 0.0,
        };
        // Only cuts inside the picture need hiding; its own edges stay as they are.
        let cuts = [
            (left > 0.0).then(|| edge(Side::Right, 1.0 - (left + x))),
            (top > 0.0).then(|| edge(Side::Bottom, 1.0 - (top + y))),
            (right < 1.0).then(|| edge(Side::Left, right + x)),
            (bottom < 1.0).then(|| edge(Side::Top, bottom + y)),
        ];
        let mut wipes = cuts
            .into_iter()
            .flatten()
            .chain(self.wipes.into_iter().flatten());
        Self {
            shift: [self.shift[0] + x, self.shift[1] + y],
            wipes: [wipes.next(), wipes.next()],
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Ticks, Transition, TransitionKind};

    /// What a piece's cuts show of the canvas across: from where to where.
    fn shown_across(piece: &Reveal) -> (f32, f32) {
        piece
            .wipes
            .iter()
            .flatten()
            .fold((0.0, 1.0), |(low, high), wipe| match wipe.from {
                Side::Left => (low, wipe.progress.min(high)),
                Side::Right => ((1.0 - wipe.progress).max(low), high),
                _ => (low, high),
            })
    }

    #[test]
    fn split_halves_part_to_either_side_of_the_incoming_picture() {
        let split = Transition::new(TransitionKind::Split, Ticks(10));
        let (outgoing, incoming) = split.reveals(0.5, 0.5);
        let pieces = outgoing.pieces();
        let shifts: Vec<_> = pieces.iter().map(|piece| piece.shift).collect();
        assert_eq!(shifts, [[-0.25, 0.0], [0.25, 0.0]]);
        assert_eq!(shown_across(&pieces[0]), (0.0, 0.25));
        assert_eq!(shown_across(&pieces[1]), (0.75, 1.0));
        // The incoming picture fills the gap between them.
        assert_eq!(incoming.wipes[0].map(|wipe| wipe.progress), Some(0.5));
    }

    #[test]
    fn a_picture_that_has_not_parted_is_drawn_whole() {
        let split = Transition::new(TransitionKind::CenterSplit, Ticks(10));
        let (outgoing, _) = split.reveals(0.0, 0.5);
        assert_eq!(outgoing.pieces(), [Reveal::FULL]);
        let (outgoing, _) = split.reveals(0.5, 0.5);
        assert_eq!(outgoing.pieces().len(), 4);
    }
}
