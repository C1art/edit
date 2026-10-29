use std::f32::consts::FRAC_PI_2;

use crate::{BlendMode, Side, Split, Transition, TransitionKind, Wipe, WipePattern};

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
    /// Turns the picture in depth.
    pub turn: Option<Turn>,
    /// Replaces the clip's blend mode.
    pub blend: Option<BlendMode>,
    pub split: Option<Split>,
    /// Motion blur as far as this shift, each way.
    pub smear: [f32; 2],
    pub fold: Option<Fold>,
}

/// A picture folding over a line that sweeps from one corner to the opposite one, the
/// lifted part laid back over the rest, as a page turns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fold {
    /// The corner it lifts from: -1 or 1 across, then down, from the picture's center.
    pub corner: [f32; 2],
    /// 0 flat, 1 lifted clear of the picture.
    pub progress: f32,
    /// Whether the page curls, its back pale and shaded, rather than folding flat.
    pub curl: bool,
}

/// A picture turned in depth by a transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Turn {
    /// Radians: about the vertical axis with the right side coming nearer, or about the
    /// horizontal one with the bottom coming nearer when `vertical`.
    pub angle: f32,
    pub vertical: bool,
    /// Whether it turns as a face of a cube, about an axis behind its middle by half
    /// its breadth, rather than about its middle.
    pub cube: bool,
}

impl Reveal {
    pub const FULL: Self = Self {
        fade: 1.0,
        shift: [0.0; 2],
        zoom: 1.0,
        wipes: [None; 2],
        wash: 0.0,
        turn: None,
        blend: None,
        split: None,
        smear: [0.0; 2],
        fold: None,
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
            turn: self.turn.or(other.turn),
            blend: self.blend.or(other.blend),
            split: self.split.or(other.split),
            smear: [0, 1].map(|axis| self.smear[axis] + other.smear[axis]),
            fold: self.fold.or(other.fold),
        }
    }
}

impl Transition {
    /// The outgoing and incoming pictures `progress` of the way through, where the cut
    /// falls at `cut`. The incoming picture is drawn over the outgoing one, unless the
    /// kind moves the outgoing one away.
    pub fn reveals(self, progress: f32, cut: f32) -> (Reveal, Reveal) {
        let from = self.side();
        if let Some(pattern) = self.kind.pattern() {
            let wipe = Wipe {
                pattern,
                from,
                progress,
                sweep: self.sweep,
                border: self.border,
                softness: self.softness(),
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
            TransitionKind::AdditiveDissolve => mingled(progress, BlendMode::LinearDodge),
            TransitionKind::NonAdditiveDissolve => mingled(progress, BlendMode::Lighten),
            TransitionKind::FlipOver => (
                unless(crossed, turned(from, before, false)),
                unless(!crossed, turned(from, after - 1.0, false)),
            ),
            TransitionKind::Split => (
                Reveal {
                    split: Some(Split::Halves {
                        vertical: across(from),
                        apart: progress / 2.0,
                    }),
                    ..Reveal::FULL
                },
                uncovered(WipePattern::Split, from, progress),
            ),
            TransitionKind::CenterSplit => (
                Reveal {
                    split: Some(Split::Quarters {
                        apart: progress / 2.0,
                    }),
                    ..Reveal::FULL
                },
                uncovered(WipePattern::Cross, from, progress),
            ),
            TransitionKind::BandSlide => incoming(Reveal {
                split: Some(Split::Bands {
                    count: SLIDING_BANDS,
                    vertical: !across(from),
                    shift: if matches!(from, Side::Left | Side::Top) {
                        progress - 1.0
                    } else {
                        1.0 - progress
                    },
                }),
                ..Reveal::FULL
            }),
            TransitionKind::Whip => {
                // Eased, so the pictures whip through the middle and settle at the ends.
                let eased = progress * progress * (3.0 - 2.0 * progress);
                let speed = 6.0 * progress * (1.0 - progress);
                let smear = toward(from, WHIP_SMEAR * speed).map(f32::abs);
                let whipped = |shift| Reveal {
                    smear,
                    ..shifted(shift)
                };
                (
                    whipped(toward(from, -eased)),
                    whipped(toward(from, 1.0 - eased)),
                )
            }
            TransitionKind::CubeSpin => (
                turned(from, progress, true),
                turned(from, progress - 1.0, true),
            ),
            TransitionKind::PageTurn | TransitionKind::PagePeel => {
                let corner = match from {
                    Side::Left => [-1.0, -1.0],
                    Side::Top => [1.0, -1.0],
                    Side::Right => [1.0, 1.0],
                    Side::Bottom => [-1.0, 1.0],
                };
                let fold = Fold {
                    corner,
                    progress,
                    curl: self.kind == TransitionKind::PagePeel,
                };
                (
                    Reveal {
                        fold: Some(fold),
                        ..Reveal::FULL
                    },
                    Reveal::FULL,
                )
            }
            _ => incoming(Reveal::faded(progress)),
        }
    }
}

/// Bands a band slide cuts the incoming picture into.
const SLIDING_BANDS: u8 = 7;
/// How far, in canvases, a whip blurs the pictures each way at its fastest.
const WHIP_SMEAR: f32 = 0.06;

/// Whether `side` is the picture's left or right edge.
fn across(side: Side) -> bool {
    matches!(side, Side::Left | Side::Right)
}

/// The outgoing picture as it is, the incoming one shown through `pattern`.
fn uncovered(pattern: WipePattern, from: Side, progress: f32) -> Reveal {
    let wipe = Wipe {
        pattern,
        from,
        progress,
        sweep: crate::Sweep::Forward,
        border: crate::Border::NONE,
        softness: 0.0,
    };
    Reveal {
        wipes: [Some(wipe), None],
        ..Reveal::FULL
    }
}

/// Each picture at full strength through the middle, the incoming one laid on by
/// `blend`.
fn mingled(progress: f32, blend: BlendMode) -> (Reveal, Reveal) {
    let incoming = Reveal {
        blend: Some(blend),
        ..Reveal::faded((2.0 * progress).min(1.0))
    };
    (Reveal::faded((2.0 * (1.0 - progress)).min(1.0)), incoming)
}

/// Turned `quarters` of a right angle away from the side the incoming picture enters
/// from: positive toward the far side, negative toward that one.
fn turned(from: Side, quarters: f32, cube: bool) -> Reveal {
    let toward_far = match from {
        Side::Right | Side::Bottom => 1.0,
        Side::Left | Side::Top => -1.0,
    };
    Reveal {
        turn: Some(Turn {
            angle: quarters * toward_far * FRAC_PI_2,
            vertical: matches!(from, Side::Top | Side::Bottom),
            cube,
        }),
        ..Reveal::FULL
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

    #[test]
    fn a_cube_turns_both_faces_away_from_the_side_the_incoming_one_enters() {
        let spin = Transition {
            from: Some(Side::Left),
            ..Transition::new(TransitionKind::CubeSpin, crate::Ticks(10))
        };
        let angle = |reveal: Reveal| reveal.turn.map(|turn| turn.angle);
        let (outgoing, incoming) = spin.reveals(0.5, 0.5);
        let quarter = FRAC_PI_2 / 2.0;
        assert_eq!(
            (angle(outgoing), angle(incoming)),
            (Some(-quarter), Some(quarter))
        );
        let (outgoing, incoming) = spin.reveals(1.0, 0.5);
        assert_eq!(
            (angle(outgoing), angle(incoming)),
            (Some(-FRAC_PI_2), Some(0.0))
        );
    }
}
