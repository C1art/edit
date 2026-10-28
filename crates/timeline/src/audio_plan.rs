use std::f32::consts::FRAC_PI_2;

use crate::plan::{cut_window, fraction};
use crate::{Clip, Ticks, Track, TransitionKind};

/// Where clip `index` of `track` is heard: its own span, widened by the part of a
/// crossfade that plays it past either edge.
pub fn audio_span(track: &Track, index: usize) -> (Ticks, Ticks) {
    let clip = &track.clips[index];
    let (previous, next) = neighbors(track, index);
    let mut start = clip.start;
    if let (Some(transition), Some(previous)) = (clip.transition_in, previous) {
        if transition.kind == TransitionKind::CrossDissolve {
            start = start - cut_window(previous, clip, transition).0;
        }
    }
    let mut end = clip.end();
    if let Some((transition, next)) = next.and_then(|next| Some((next.transition_in?, next))) {
        if transition.kind == TransitionKind::CrossDissolve {
            end = end + cut_window(clip, next, transition).1;
        }
    }
    (start, end)
}

/// The amplitude transitions give clip `index` at `position`. Fades follow a
/// constant-power curve, so a crossfade between correlated-free sources holds its
/// loudness through the middle.
pub fn audio_fade(track: &Track, index: usize, position: Ticks) -> f32 {
    let clip = &track.clips[index];
    let (previous, next) = neighbors(track, index);
    let rise = |progress: f32| (progress * FRAC_PI_2).sin();
    let mut gain = 1.0;
    match (clip.transition_in, previous) {
        (Some(transition), Some(previous)) => {
            let (before, after) = cut_window(previous, clip, transition);
            gain *= match transition.kind {
                TransitionKind::CrossDissolve => {
                    rise(fraction(position - (clip.start - before), before + after))
                }
                TransitionKind::DipToBlack if position < clip.start => 0.0,
                TransitionKind::DipToBlack => rise(fraction(position - clip.start, after)),
            };
        }
        (Some(transition), None) => {
            let length = transition.duration.min(clip.duration);
            gain *= rise(fraction(position - clip.start, length));
        }
        (None, _) => {}
    }
    match (
        next.and_then(|next| Some((next.transition_in?, next))),
        next,
    ) {
        (Some((transition, next)), _) => {
            let (before, after) = cut_window(clip, next, transition);
            let start = clip.end() - before;
            gain *= match transition.kind {
                TransitionKind::CrossDissolve => {
                    rise(1.0 - fraction(position - start, before + after))
                }
                TransitionKind::DipToBlack if position >= clip.end() => 0.0,
                TransitionKind::DipToBlack => rise(1.0 - fraction(position - start, before)),
            };
        }
        (None, None) => {
            if let Some(transition) = clip.transition_out {
                let length = transition.duration.min(clip.duration);
                gain *= rise(fraction(clip.end() - position, length));
            }
        }
        (None, Some(_)) => {}
    }
    gain
}

fn neighbors(track: &Track, index: usize) -> (Option<&Clip>, Option<&Clip>) {
    let clips = &track.clips;
    let clip = &clips[index];
    let previous = index
        .checked_sub(1)
        .map(|i| &clips[i])
        .filter(|previous| previous.end() == clip.start);
    let next = clips.get(index + 1).filter(|next| next.start == clip.end());
    (previous, next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipId, SourceId, Transition};

    fn track_with_crossfade(duration: i64) -> Track {
        let mut incoming = Clip::new(ClipId(2), SourceId(1), Ticks(100), Ticks(100), Ticks(0));
        incoming.transition_in = Some(Transition {
            kind: TransitionKind::CrossDissolve,
            duration: Ticks(duration),
        });
        let outgoing = Clip::new(ClipId(1), SourceId(1), Ticks(0), Ticks(100), Ticks(0));
        Track {
            clips: vec![outgoing, incoming],
            ..Track::default()
        }
    }

    #[test]
    fn crossfade_holds_power_across_the_cut() {
        let track = track_with_crossfade(40);
        assert_eq!(audio_span(&track, 0), (Ticks(0), Ticks(120)));
        assert_eq!(audio_span(&track, 1), (Ticks(80), Ticks(200)));
        for time in [80, 90, 100, 110, 119] {
            let out = audio_fade(&track, 0, Ticks(time));
            let incoming = audio_fade(&track, 1, Ticks(time));
            assert!(
                (out * out + incoming * incoming - 1.0).abs() < 1e-5,
                "{time}"
            );
        }
        assert_eq!(audio_fade(&track, 1, Ticks(80)), 0.0);
        assert_eq!(audio_fade(&track, 0, Ticks(79)), 1.0);
    }
}
