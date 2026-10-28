use crate::{Side, TransitionKind};

/// How a transition shows one picture, on top of the clip's own look. Distances are
/// fractions of the canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reveal {
    /// Multiplies the clip's opacity.
    pub fade: f32,
    /// +x right, +y down.
    pub shift: [f32; 2],
    /// Scales the picture about the canvas center.
    pub zoom: f32,
    /// Hidden from the canvas's left, top, right and bottom.
    pub mask: [f32; 4],
    /// How far the picture is washed out to white.
    pub wash: f32,
}

impl Reveal {
    pub const FULL: Self = Self {
        fade: 1.0,
        shift: [0.0; 2],
        zoom: 1.0,
        mask: [0.0; 4],
        wash: 0.0,
    };

    const HIDDEN: Self = Self {
        fade: 0.0,
        ..Self::FULL
    };

    fn faded(fade: f32) -> Self {
        Self { fade, ..Self::FULL }
    }

    pub fn is_visible(&self) -> bool {
        let [left, top, right, bottom] = self.mask;
        self.fade > 0.0 && left + right < 1.0 && top + bottom < 1.0
    }

    /// Both reveals at once, as when a short clip's head and tail transitions overlap.
    pub fn and(self, other: Self) -> Self {
        Self {
            fade: self.fade * other.fade,
            shift: [0, 1].map(|axis| self.shift[axis] + other.shift[axis]),
            zoom: self.zoom * other.zoom,
            mask: [0, 1, 2, 3].map(|side| self.mask[side].max(other.mask[side])),
            wash: self.wash.max(other.wash),
        }
    }
}

impl TransitionKind {
    /// The outgoing and incoming pictures `progress` of the way through, where the cut
    /// falls at `cut` and the incoming picture enters `from` a side. The incoming
    /// picture is drawn over the outgoing one.
    pub fn reveals(self, progress: f32, cut: f32, from: Side) -> (Reveal, Reveal) {
        let before = phase(progress, 0.0, cut);
        let after = phase(progress, cut, 1.0);
        let crossed = progress >= cut;
        let unless = |hidden: bool, reveal: Reveal| if hidden { Reveal::HIDDEN } else { reveal };
        let incoming = |reveal| (Reveal::FULL, reveal);
        match self {
            Self::CrossDissolve => incoming(Reveal::faded(progress)),
            Self::DipToBlack => (Reveal::faded(1.0 - before), Reveal::faded(after)),
            Self::DipToWhite => (
                unless(crossed, washed(before)),
                unless(!crossed, washed(1.0 - after)),
            ),
            Self::CrossZoom => (
                unless(crossed, zoomed(1.0 + 4.0 * before)),
                unless(!crossed, zoomed(5.0 - 4.0 * after)),
            ),
            Self::Wipe => {
                let mut mask = [0.0; 4];
                mask[opposite(from)] = 1.0 - progress;
                incoming(masked(mask))
            }
            Self::BarnDoors => {
                let side = (1.0 - progress) / 2.0;
                incoming(masked(match from {
                    Side::Left | Side::Right => [side, 0.0, side, 0.0],
                    Side::Top | Side::Bottom => [0.0, side, 0.0, side],
                }))
            }
            Self::IrisBox => incoming(masked([(1.0 - progress) / 2.0; 4])),
            Self::Slide => incoming(shifted(toward(from, 1.0 - progress))),
            Self::Push => (
                shifted(toward(from, -progress)),
                shifted(toward(from, 1.0 - progress)),
            ),
        }
    }
}

fn washed(wash: f32) -> Reveal {
    Reveal {
        wash,
        ..Reveal::FULL
    }
}

fn zoomed(zoom: f32) -> Reveal {
    Reveal {
        zoom,
        ..Reveal::FULL
    }
}

fn masked(mask: [f32; 4]) -> Reveal {
    Reveal {
        mask,
        ..Reveal::FULL
    }
}

fn shifted(shift: [f32; 2]) -> Reveal {
    Reveal {
        shift,
        ..Reveal::FULL
    }
}

/// The index in a mask of the edge across the canvas from `side`.
fn opposite(side: Side) -> usize {
    match side {
        Side::Left => 2,
        Side::Top => 3,
        Side::Right => 0,
        Side::Bottom => 1,
    }
}

/// A shift of `amount` canvases out toward `side`.
fn toward(side: Side, amount: f32) -> [f32; 2] {
    match side {
        Side::Left => [-amount, 0.0],
        Side::Top => [0.0, -amount],
        Side::Right => [amount, 0.0],
        Side::Bottom => [0.0, amount],
    }
}

/// How far `progress` is from `from` to `to`, clamped to 0..=1; an empty span is a step.
fn phase(progress: f32, from: f32, to: f32) -> f32 {
    if to <= from {
        return if progress >= to { 1.0 } else { 0.0 };
    }
    ((progress - from) / (to - from)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_incoming_picture_enters_from_the_chosen_side() {
        let (_, wipe) = TransitionKind::Wipe.reveals(0.25, 0.5, Side::Top);
        assert_eq!(wipe.mask, [0.0, 0.0, 0.0, 0.75]);
        let (outgoing, incoming) = TransitionKind::Push.reveals(0.25, 0.5, Side::Left);
        assert_eq!(
            (outgoing.shift, incoming.shift),
            ([0.25, 0.0], [-0.75, 0.0])
        );
        let (_, doors) = TransitionKind::BarnDoors.reveals(0.5, 0.5, Side::Bottom);
        assert_eq!(doors.mask, [0.0, 0.25, 0.0, 0.25]);
    }
}
