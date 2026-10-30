//! Transitions where the timeline shows them, and their outer ends, which drag to change
//! how long they last.

use std::ops::Range;

use egui::Pos2;
use timeline::{cut_window, Alignment, Clip, ClipId, Edge, Sequence, Ticks, Track, Transition};

use super::layout::{row_at, Row};
use super::scale::TimeScale;

/// How near a transition's outer end the pointer grabs it, in points; less than a clip
/// edge's reach so a short transition's end doesn't crowd out the trim beside it.
const GRIP_REACH: f32 = 4.0;

/// A transition and the part of the sequence it plays over.
pub(super) struct Shown {
    pub clip: ClipId,
    pub edge: Edge,
    pub transition: Transition,
    pub span: Range<Ticks>,
    /// For the start and end of the span, how many times a drag's distance the length
    /// changes by when that end is dragged; `None` at a cut or a clip's edge, which trims.
    pub stretch: [Option<i64>; 2],
    /// The longest it can play over the clips it joins.
    pub longest: Ticks,
}

/// A grabbed outer end of a transition.
#[derive(Debug, Clone)]
pub struct Grip {
    pub clip: ClipId,
    pub edge: Edge,
    pub transition: Transition,
    pub stretch: i64,
    pub longest: Ticks,
    /// Where the grabbed end is, for snapping.
    pub at: Ticks,
}

impl Grip {
    /// The transition after its end is dragged by `delta`, at least `shortest` long.
    pub fn dragged(&self, delta: Ticks, shortest: Ticks) -> Transition {
        let duration = self.transition.duration + Ticks(self.stretch * delta.0);
        Transition {
            duration: duration.clamp(shortest, self.longest.max(shortest)),
            ..self.transition
        }
    }
}

pub(super) fn shown(track: &Track, clip: &Clip) -> Vec<Shown> {
    let mut shown = Vec::new();
    if let Some(transition) = clip.transition_in {
        let previous = track.clips.iter().find(|other| other.end() == clip.start);
        let (span, stretch, longest) = match previous {
            Some(previous) => {
                let (before, after) = cut_window(previous, clip, transition);
                let shorter = previous.duration.min(clip.duration);
                let (stretch, longest) = match transition.alignment {
                    Alignment::Center => ([Some(-2), Some(2)], shorter + shorter),
                    Alignment::StartAtCut => ([None, Some(1)], clip.duration),
                    Alignment::EndAtCut => ([Some(-1), None], previous.duration),
                };
                (clip.start - before..clip.start + after, stretch, longest)
            }
            None => (
                clip.start..clip.start + transition.duration.min(clip.duration),
                [None, Some(1)],
                clip.duration,
            ),
        };
        shown.push(Shown {
            clip: clip.id,
            edge: Edge::Head,
            transition,
            span,
            stretch,
            longest,
        });
    }
    if let Some(transition) = clip.transition_out {
        if !track.clips.iter().any(|next| next.start == clip.end()) {
            let duration = transition.duration.min(clip.duration);
            shown.push(Shown {
                clip: clip.id,
                edge: Edge::Tail,
                transition,
                span: clip.end() - duration..clip.end(),
                stretch: [Some(-1), None],
                longest: clip.duration,
            });
        }
    }
    shown
}

/// The transition end under `pos`, if any.
pub(super) fn grip_at(
    sequence: &Sequence,
    rows: &[Row],
    scale: &TimeScale,
    pos: Pos2,
) -> Option<Grip> {
    let row = row_at(rows, pos.y)?;
    let track = sequence.track(row.track)?;
    let near = |time: Ticks| (scale.x(time) - pos.x).abs() <= GRIP_REACH;
    track
        .clips
        .iter()
        .filter(|clip| clip.transition_in.is_some() || clip.transition_out.is_some())
        .flat_map(|clip| shown(track, clip))
        .find_map(|shown| {
            let ends = [shown.span.start, shown.span.end];
            let (at, stretch) = ends
                .into_iter()
                .zip(shown.stretch)
                .find_map(|(at, stretch)| near(at).then_some((at, stretch?)))?;
            Some(Grip {
                clip: shown.clip,
                edge: shown.edge,
                transition: shown.transition,
                stretch,
                longest: shown.longest,
                at,
            })
        })
}
