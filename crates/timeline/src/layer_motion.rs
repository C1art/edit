//! How a graphic's layers come on at the start of their clip and go off at its end, as
//! animated title templates do. Timed from the clip's own edges, so trimming a clip
//! keeps its exit at the end.

use serde::{Deserialize, Serialize};

/// How far a slide travels, in frame heights.
const SLIDE: f32 = 0.06;
/// How small a pop starts, as a share of the layer's size.
const POP_FROM: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Appear {
    #[default]
    None,
    Fade,
    /// Moves in from, or out to, a side while fading.
    Slide(FrameSide),
    /// Uncovered from, or covered back toward, a side.
    Wipe(FrameSide),
    /// Grows from small with a little overshoot.
    Pop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FrameSide {
    Left,
    Right,
    Top,
    Bottom,
}

impl FrameSide {
    pub const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Top, Self::Bottom];

    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Top => "Top",
            Self::Bottom => "Bottom",
        }
    }

    /// The way toward this side, +y down.
    fn toward(self) -> [f32; 2] {
        match self {
            Self::Left => [-1.0, 0.0],
            Self::Right => [1.0, 0.0],
            Self::Top => [0.0, -1.0],
            Self::Bottom => [0.0, 1.0],
        }
    }
}

impl Appear {
    pub fn name(self) -> String {
        match self {
            Self::None => "None".to_owned(),
            Self::Fade => "Fade".to_owned(),
            Self::Slide(side) => format!("Slide {}", side.name()),
            Self::Wipe(side) => format!("Wipe {}", side.name()),
            Self::Pop => "Pop".to_owned(),
        }
    }

    /// Every choice, for menus.
    pub fn all() -> impl Iterator<Item = Self> {
        [Self::None, Self::Fade, Self::Pop]
            .into_iter()
            .chain(FrameSide::ALL.map(Self::Slide))
            .chain(FrameSide::ALL.map(Self::Wipe))
    }

    /// Changes `pose` for a layer `shown` of the way on, from 0 (gone) to 1 (fully on).
    fn apply(self, shown: f32, pose: &mut LayerPose) {
        let eased = ease_out(shown);
        match self {
            Self::None => {}
            Self::Fade => pose.opacity *= eased,
            Self::Slide(side) => {
                pose.opacity *= eased;
                let [x, y] = side.toward();
                let away = SLIDE * (1.0 - eased);
                pose.shift = [pose.shift[0] + x * away, pose.shift[1] + y * away];
            }
            Self::Wipe(side) => {
                let covered = pose.reveal.is_some_and(|(_, shown)| shown <= eased);
                if eased < 1.0 && !covered {
                    pose.reveal = Some((side, eased));
                }
            }
            Self::Pop => {
                pose.scale *= POP_FROM + (1.0 - POP_FROM) * overshoot(shown);
                // Fades in over the first quarter, so it never shows as a dot.
                pose.opacity *= (shown * 4.0).min(1.0);
            }
        }
    }
}

/// Quick to start and gentle to land; mirrored in time it leaves gently, then quickly.
fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Passes 1 a little before settling on it.
fn overshoot(t: f32) -> f32 {
    const BACK: f32 = 1.70158;
    let u = t - 1.0;
    1.0 + (BACK + 1.0) * u.powi(3) + BACK * u.powi(2)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LayerAnimation {
    pub enter: Appear,
    pub exit: Appear,
    /// Frames after the clip starts before the layer comes on, and before it ends that
    /// the layer is gone, so layers can follow one another.
    pub delay: u32,
    /// Frames each of coming on and going off takes.
    pub length: u32,
}

impl Default for LayerAnimation {
    fn default() -> Self {
        Self {
            enter: Appear::None,
            exit: Appear::None,
            delay: 0,
            length: 15,
        }
    }
}

/// How a layer draws at one frame, besides how it's set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerPose {
    pub opacity: f32,
    /// Added to its position, in frame heights.
    pub shift: [f32; 2],
    /// Times its scale.
    pub scale: f32,
    /// The share of it uncovered, from a side.
    pub reveal: Option<(FrameSide, f32)>,
}

impl LayerPose {
    pub const AS_SET: Self = Self {
        opacity: 1.0,
        shift: [0.0, 0.0],
        scale: 1.0,
        reveal: None,
    };

    pub fn is_as_set(&self) -> bool {
        *self == Self::AS_SET
    }
}

impl LayerAnimation {
    pub fn is_still(&self) -> bool {
        self.enter == Appear::None && self.exit == Appear::None
    }

    /// The pose at `frame` of a clip `frames` long, counted from its start.
    pub fn pose(&self, frame: i64, frames: i64) -> LayerPose {
        let length = self.length.max(1) as f32;
        let delay = i64::from(self.delay);
        let entered = ((frame - delay) as f32 / length).clamp(0.0, 1.0);
        let remaining = ((frames - 1 - delay - frame) as f32 / length).clamp(0.0, 1.0);
        let mut pose = LayerPose::AS_SET;
        self.enter.apply(entered, &mut pose);
        self.exit.apply(remaining, &mut pose);
        pose
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn animation(enter: Appear, exit: Appear) -> LayerAnimation {
        LayerAnimation {
            enter,
            exit,
            delay: 5,
            length: 10,
        }
    }

    #[test]
    fn layers_come_on_after_their_delay_and_are_gone_as_long_before_the_end() {
        let fade = animation(Appear::Fade, Appear::Fade);
        let opacity = |frame| fade.pose(frame, 100).opacity;
        assert_eq!(opacity(0), 0.0);
        assert_eq!(opacity(5), 0.0);
        assert!(opacity(10) > 0.5 && opacity(10) < 1.0);
        assert!(fade.pose(15, 100).is_as_set());
        assert!(fade.pose(84, 100).is_as_set());
        assert_eq!(opacity(94), 0.0);
        assert_eq!(opacity(99), 0.0);
    }

    #[test]
    fn slides_travel_from_their_side_and_back_to_the_other() {
        let slide = animation(
            Appear::Slide(FrameSide::Left),
            Appear::Slide(FrameSide::Bottom),
        );
        let start = slide.pose(5, 100);
        assert!((start.shift[0] + SLIDE).abs() < 1e-6);
        let end = slide.pose(94, 100);
        assert!((end.shift[1] - SLIDE).abs() < 1e-6);
        assert_eq!(end.shift[0], 0.0);
    }

    #[test]
    fn a_pop_overshoots_before_settling() {
        let pop = animation(Appear::Pop, Appear::None);
        let scales: Vec<f32> = (5..=15).map(|frame| pop.pose(frame, 100).scale).collect();
        assert_eq!(scales[0], POP_FROM);
        assert!(scales.iter().any(|&scale| scale > 1.0));
        assert_eq!(scales[10], 1.0);
    }

    #[test]
    fn a_short_clip_overlaps_coming_on_and_going_off() {
        let wipe = animation(
            Appear::Wipe(FrameSide::Left),
            Appear::Wipe(FrameSide::Right),
        );
        // Ten frames long: never fully on, and the smaller share of the two shows.
        let middle = wipe.pose(5, 10);
        assert_eq!(middle.reveal.map(|(side, _)| side), Some(FrameSide::Left));
        assert!((0..10).all(|frame| !wipe.pose(frame, 10).is_as_set()));
    }
}
