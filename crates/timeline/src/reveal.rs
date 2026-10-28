use crate::{Side, Transition, TransitionKind, Wipe};

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
    /// What of the picture wipes show, in canvas space; a picture under both a head and
    /// a tail transition can be in two at once.
    pub wipes: [Option<Wipe>; 2],
    /// How far the picture is washed out to white.
    pub wash: f32,
}

impl Reveal {
    pub const FULL: Self = Self {
        fade: 1.0,
        shift: [0.0; 2],
        zoom: 1.0,
        wipes: [None; 2],
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
        self.fade > 0.0 && self.wipes.iter().flatten().all(Wipe::is_visible)
    }

    /// Both reveals at once, as when a short clip's head and tail transitions overlap.
    pub fn and(self, other: Self) -> Self {
        let mut wipes = self.wipes.into_iter().chain(other.wipes).flatten();
        Self {
            fade: self.fade * other.fade,
            shift: [0, 1].map(|axis| self.shift[axis] + other.shift[axis]),
            zoom: self.zoom * other.zoom,
            wipes: [wipes.next(), wipes.next()],
            wash: self.wash.max(other.wash),
        }
    }
}

impl Transition {
    /// The outgoing and incoming pictures `progress` of the way through, where the cut
    /// falls at `cut`. The incoming picture is drawn over the outgoing one.
    pub fn reveals(self, progress: f32, cut: f32) -> (Reveal, Reveal) {
        let from = self.side();
        if let Some(pattern) = self.kind.pattern() {
            let wipe = Wipe {
                pattern,
                from,
                progress,
                sweep: self.sweep,
                border: self.border,
            };
            return (
                Reveal::FULL,
                Reveal {
                    wipes: [Some(wipe), None],
                    ..Reveal::FULL
                },
            );
        }
        let before = phase(progress, 0.0, cut);
        let after = phase(progress, cut, 1.0);
        let crossed = progress >= cut;
        let unless = |hidden: bool, reveal: Reveal| if hidden { Reveal::HIDDEN } else { reveal };
        let incoming = |reveal| (Reveal::FULL, reveal);
        match self.kind {
            TransitionKind::DipToBlack => (Reveal::faded(1.0 - before), Reveal::faded(after)),
            TransitionKind::DipToWhite => (
                unless(crossed, washed(before)),
                unless(!crossed, washed(1.0 - after)),
            ),
            TransitionKind::CrossZoom => (
                unless(crossed, zoomed(1.0 + 4.0 * before)),
                unless(!crossed, zoomed(5.0 - 4.0 * after)),
            ),
            TransitionKind::Slide => incoming(shifted(toward(from, 1.0 - progress))),
            TransitionKind::Push => (
                shifted(toward(from, -progress)),
                shifted(toward(from, 1.0 - progress)),
            ),
            _ => incoming(Reveal::faded(progress)),
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

fn shifted(shift: [f32; 2]) -> Reveal {
    Reveal {
        shift,
        ..Reveal::FULL
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
    fn the_incoming_picture_pushes_in_from_the_chosen_side() {
        let push = Transition {
            from: Some(Side::Left),
            ..Transition::new(TransitionKind::Push, crate::Ticks(10))
        };
        let (outgoing, incoming) = push.reveals(0.25, 0.5);
        assert_eq!(
            (outgoing.shift, incoming.shift),
            ([0.25, 0.0], [-0.75, 0.0])
        );
    }
}
