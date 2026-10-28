use crate::{Clip, Sequence, Ticks, Track, Transition, TransitionKind};

/// One picture to draw for a track, before the clip's own look is applied.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedLayer<'a> {
    pub clip: &'a Clip,
    /// Clamped to the media, so transitions past the edge of the media hold its first
    /// or last picture.
    pub source_time: Ticks,
    /// Opacity from transitions, multiplying the clip's own.
    pub fade: f32,
}

/// The pictures `track` shows at `position`, bottom first: two during a cross dissolve,
/// otherwise at most one.
pub fn track_layers_at<'a>(
    sequence: &'a Sequence,
    track: &'a Track,
    position: Ticks,
) -> Vec<PlannedLayer<'a>> {
    let Some(index) = track.clips.iter().position(|clip| clip.covers(position)) else {
        return Vec::new();
    };
    let clips = &track.clips;
    let current = &clips[index];
    let previous = index
        .checked_sub(1)
        .map(|i| &clips[i])
        .filter(|previous| previous.end() == current.start);
    let next = clips
        .get(index + 1)
        .filter(|next| next.start == current.end());
    let layer = |clip: &'a Clip, fade: f32| PlannedLayer {
        clip,
        source_time: source_time(sequence, clip, position),
        fade,
    };

    let mut fade = 1.0;
    match (current.transition_in, previous) {
        (Some(transition), Some(previous)) => {
            let (before, after) = cut_window(previous, current, transition);
            if position < current.start + after {
                let start = current.start - before;
                match transition.kind {
                    TransitionKind::CrossDissolve => {
                        let progress = fraction(position - start, before + after);
                        return vec![layer(previous, 1.0), layer(current, progress)];
                    }
                    TransitionKind::DipToBlack => {
                        fade *= fraction(position - current.start, after);
                    }
                }
            }
        }
        (Some(transition), None) => {
            let length = transition.duration.min(current.duration);
            fade *= fraction(position - current.start, length);
        }
        (None, _) => {}
    }
    match (next.and_then(|next| next.transition_in), next) {
        (Some(transition), Some(next)) => {
            let (before, after) = cut_window(current, next, transition);
            let start = current.end() - before;
            if position >= start {
                match transition.kind {
                    TransitionKind::CrossDissolve => {
                        let progress = fraction(position - start, before + after);
                        return vec![layer(current, fade), layer(next, progress)];
                    }
                    TransitionKind::DipToBlack => {
                        fade *= 1.0 - fraction(position - start, before);
                    }
                }
            }
        }
        _ => {
            if let (Some(transition), None) = (current.transition_out, next) {
                let length = transition.duration.min(current.duration);
                fade *= fraction(current.end() - position, length).min(1.0);
            }
        }
    }
    vec![layer(current, fade)]
}

/// How far a transition at the cut between `outgoing` and `incoming` reaches before and
/// after the cut: centered, but never further than either clip's own length.
pub(crate) fn cut_window(
    outgoing: &Clip,
    incoming: &Clip,
    transition: Transition,
) -> (Ticks, Ticks) {
    let half = Ticks(transition.duration.0 / 2);
    let before = half.min(outgoing.duration);
    let after = (transition.duration - half).min(incoming.duration);
    (before, after)
}

/// `part / whole` clamped to 0..=1, with an empty whole counting as complete.
pub(crate) fn fraction(part: Ticks, whole: Ticks) -> f32 {
    if whole <= Ticks::ZERO {
        return 1.0;
    }
    (part.0 as f64 / whole.0 as f64).clamp(0.0, 1.0) as f32
}

fn source_time(sequence: &Sequence, clip: &Clip, position: Ticks) -> Ticks {
    let time = clip.source_time(position).max(Ticks::ZERO);
    match sequence.media_item(clip.source) {
        Some(media) => time.min(media.duration - Ticks(1)),
        None => time,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipId, SourceId, Timebase, VideoFormat};

    fn clip(id: u64, start: i64, duration: i64, source_in: i64) -> Clip {
        Clip::new(
            ClipId(id),
            SourceId(1),
            Ticks(start),
            Ticks(duration),
            Ticks(source_in),
        )
    }

    fn dissolve(kind: TransitionKind, duration: i64) -> Option<Transition> {
        Some(Transition {
            kind,
            duration: Ticks(duration),
        })
    }

    fn summary(layers: &[PlannedLayer<'_>]) -> Vec<(u64, i64, f32)> {
        layers
            .iter()
            .map(|layer| (layer.clip.id.0, layer.source_time.0, layer.fade))
            .collect()
    }

    fn sequence() -> Sequence {
        Sequence::new(Timebase::HZ_90K, VideoFormat::default())
    }

    #[test]
    fn cross_dissolve_plays_both_clips_past_the_cut() {
        let mut incoming = clip(2, 100, 100, 500);
        incoming.transition_in = dissolve(TransitionKind::CrossDissolve, 40);
        let track = Track {
            clips: vec![clip(1, 0, 100, 0), incoming],
            ..Track::default()
        };
        let sequence = sequence();
        let at = |time| summary(&track_layers_at(&sequence, &track, Ticks(time)));
        assert_eq!(at(79), vec![(1, 79, 1.0)]);
        assert_eq!(at(90), vec![(1, 90, 1.0), (2, 490, 0.25)]);
        // Past its out point the outgoing clip keeps reading its media.
        assert_eq!(at(110), vec![(1, 110, 1.0), (2, 510, 0.75)]);
        assert_eq!(at(120), vec![(2, 520, 1.0)]);
    }

    #[test]
    fn dip_to_black_fades_out_then_in_around_the_cut() {
        let mut incoming = clip(2, 100, 100, 0);
        incoming.transition_in = dissolve(TransitionKind::DipToBlack, 40);
        let track = Track {
            clips: vec![clip(1, 0, 100, 0), incoming],
            ..Track::default()
        };
        let sequence = sequence();
        let at = |time| summary(&track_layers_at(&sequence, &track, Ticks(time)));
        assert_eq!(at(90), vec![(1, 90, 0.5)]);
        assert_eq!(at(100), vec![(2, 0, 0.0)]);
        assert_eq!(at(115), vec![(2, 15, 0.75)]);
    }

    #[test]
    fn transitions_without_a_neighbor_fade_from_and_to_nothing() {
        let mut lone = clip(1, 100, 100, 0);
        lone.transition_in = dissolve(TransitionKind::CrossDissolve, 20);
        lone.transition_out = dissolve(TransitionKind::DipToBlack, 50);
        let track = Track {
            clips: vec![clip(2, 0, 50, 0), lone],
            ..Track::default()
        };
        let sequence = sequence();
        let at = |time| summary(&track_layers_at(&sequence, &track, Ticks(time)));
        assert_eq!(at(105), vec![(1, 5, 0.25)]);
        assert_eq!(at(175), vec![(1, 75, 0.5)]);
    }
}
