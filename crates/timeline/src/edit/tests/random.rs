//! Long runs of random edits, checking after each one what every edit must keep: a
//! valid sequence, unique clip ids, a failed edit changing nothing, and undo and redo
//! landing on exactly the states before and after.

use std::collections::HashSet;
use std::sync::Arc;

use super::*;
use crate::{LinkedSelection, Marks, Speed, TrackKind, Transition, TransitionKind};

const SEEDS: u64 = 24;
const STEPS: usize = 400;

/// A small deterministic generator, so a failing seed replays exactly.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }

    fn ticks(&mut self, range: std::ops::Range<i64>) -> Ticks {
        let span = (range.end - range.start).max(1) as u64;
        Ticks(range.start + (self.next() % span) as i64)
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> Option<T> {
        (!items.is_empty()).then(|| items[self.below(items.len())])
    }
}

/// Media with sound and pictures, pictures only and sound only, on two tracks of each.
fn fixture() -> (Timeline, Vec<SourceId>) {
    let (mut timeline, both) = timeline();
    let format = timeline.sequence().format;
    for (name, video, has_audio) in [("v", Some(format), false), ("s", None, true)] {
        timeline
            .apply(ImportMedia {
                name: name.into(),
                path: format!("{name}.mp4").into(),
                duration: Ticks(700),
                video,
                has_audio,
                still: false,
                interpreted_rate: None,
                placed: None,
            })
            .unwrap();
    }
    let sources = timeline
        .sequence()
        .media
        .iter()
        .map(|item| item.id)
        .collect();
    let _ = both;
    (timeline, sources)
}

fn clip_ids(sequence: &Sequence) -> Vec<ClipId> {
    sequence
        .tracks()
        .flat_map(|(_, track)| track.clips.iter().map(Arc::as_ref))
        .map(|clip| clip.id)
        .collect()
}

fn some_clips(rng: &mut Rng, sequence: &Sequence) -> Vec<ClipId> {
    let all = clip_ids(sequence);
    let count = 1 + rng.below(3);
    (0..count).filter_map(|_| rng.pick(&all)).collect()
}

fn some_track(rng: &mut Rng, sequence: &Sequence) -> Option<TrackRef> {
    let tracks: Vec<TrackRef> = sequence.tracks().map(|(track, _)| track).collect();
    rng.pick(&tracks)
}

/// Some time on the sequence: often an edit point or inside a clip, so edits meet clips.
fn some_time(rng: &mut Rng, sequence: &Sequence) -> Ticks {
    let clips: Vec<(Ticks, Ticks)> = sequence
        .tracks()
        .flat_map(|(_, track)| track.clips.iter().map(Arc::as_ref))
        .map(|clip| (clip.start, clip.end()))
        .collect();
    match (rng.pick(&clips), rng.below(5)) {
        (Some((start, end)), 0 | 1) => rng.pick(&[start, end]).unwrap_or(start) + rng.ticks(-2..3),
        (Some((start, end)), 2 | 3) => rng.ticks(start.0..end.0),
        _ => rng.ticks(-20..sequence.duration().0 + 200),
    }
}

fn some_delta(rng: &mut Rng) -> Ticks {
    match rng.below(3) {
        0 => rng.ticks(-3..4),
        1 => rng.ticks(-60..61),
        _ => rng.ticks(-600..601),
    }
}

fn some_edge(rng: &mut Rng) -> Edge {
    if rng.chance(50) {
        Edge::Head
    } else {
        Edge::Tail
    }
}

/// Applies one random edit, returning a description of it to report on failure.
fn random_edit(
    rng: &mut Rng,
    timeline: &mut Timeline,
    sources: &[SourceId],
) -> (String, Result<(), TimelineError>) {
    let sequence = timeline.sequence().clone();
    let source = rng.pick(sources).unwrap_or(sources[0]);
    let mode = if rng.chance(50) {
        PlaceMode::Insert
    } else {
        PlaceMode::Overwrite
    };
    macro_rules! run {
        ($edit:expr) => {{
            let edit = $edit;
            let described = format!("{edit:?}");
            (described, timeline.apply(edit))
        }};
    }
    let sparse = clip_ids(&sequence).len() < 8;
    match if sparse && rng.chance(50) {
        0
    } else {
        rng.below(26)
    } {
        0..=3 => run!(PlaceClip {
            source,
            source_in: if rng.chance(90) {
                rng.ticks(0..400)
            } else {
                rng.ticks(-5..700)
            },
            duration: if rng.chance(90) {
                rng.ticks(1..300)
            } else {
                rng.ticks(-5..400)
            },
            at: some_time(rng, &sequence),
            video_track: rng
                .pick(&[None, Some(0), Some(0), Some(1), Some(5)])
                .flatten(),
            audio_track: rng
                .pick(&[None, Some(0), Some(0), Some(1), Some(5)])
                .flatten(),
            mode,
            fill: None,
        }),
        4 => run!(Razor {
            at: some_time(rng, &sequence),
            target: if rng.chance(50) {
                RazorTarget::Clips(some_clips(rng, &sequence))
            } else {
                RazorTarget::Tracks(some_track(rng, &sequence).into_iter().collect())
            },
        }),
        5 | 6 => run!(Trim {
            clips: some_clips(rng, &sequence),
            edge: some_edge(rng),
            delta: some_delta(rng),
            mode: if rng.chance(50) {
                TrimMode::Normal
            } else {
                TrimMode::Ripple
            },
        }),
        7 => run!(Roll {
            at: some_time(rng, &sequence),
            tracks: some_track(rng, &sequence).into_iter().collect(),
            delta: some_delta(rng),
        }),
        8 => run!(Slip {
            clips: some_clips(rng, &sequence),
            delta: some_delta(rng),
        }),
        9 => run!(Slide {
            clips: some_clips(rng, &sequence),
            delta: some_delta(rng),
        }),
        10 | 11 => run!(MoveClips {
            clips: some_clips(rng, &sequence),
            delta: some_delta(rng),
            video_tracks: rng.ticks(-1..2).0 as isize,
            audio_tracks: rng.ticks(-1..2).0 as isize,
            mode: rng
                .pick(&[MoveMode::Overwrite, MoveMode::Insert, MoveMode::Rearrange])
                .unwrap_or_default(),
        }),
        12 => run!(Lift {
            clips: some_clips(rng, &sequence),
        }),
        13 => run!(RippleDelete {
            clips: some_clips(rng, &sequence),
        }),
        14 => match some_track(rng, &sequence) {
            Some(track) => run!(CloseGap {
                track,
                at: some_time(rng, &sequence),
            }),
            None => run!(CloseGaps),
        },
        15 => {
            let (a, b) = (some_time(rng, &sequence), some_time(rng, &sequence));
            let marks = Marks {
                mark_in: Some(a.min(b)),
                mark_out: Some(a.max(b)),
            };
            let _ = timeline.apply(SetMarks { marks });
            if rng.chance(50) {
                run!(ExtractMarked)
            } else {
                run!(LiftMarked)
            }
        }
        16 => {
            let at = some_time(rng, &sequence);
            let edge = some_edge(rng);
            if rng.chance(50) {
                run!(RippleTrimToPlayhead { at, edge })
            } else {
                run!(TrimToPlayhead { at, edge })
            }
        }
        17 => match Clipboard::copy(&sequence, &some_clips(rng, &sequence)) {
            Some(clipboard) => run!(PasteClips {
                clipboard,
                at: some_time(rng, &sequence),
                mode,
                video_offset: rng.ticks(-1..2).0 as isize,
                audio_offset: rng.ticks(-1..2).0 as isize,
            }),
            None => run!(CloseGaps),
        },
        18 => {
            let speeds = [(1, 2), (2, 1), (3, 2), (1, 3), (1, 1), (7, 5)];
            let (num, den) = rng.pick(&speeds).unwrap_or((1, 1));
            run!(SetClipSpeed {
                clips: some_clips(rng, &sequence),
                speed: Speed::new(num, den).unwrap_or(Speed::NORMAL),
                ripple: rng.chance(50),
                reverse: rng.chance(30),
                maintain_pitch: false,
            })
        }
        19 => run!(RateStretch {
            clips: some_clips(rng, &sequence),
            edge: some_edge(rng),
            delta: some_delta(rng),
        }),
        20 => run!(AddFrameHold {
            clips: some_clips(rng, &sequence),
            at: some_time(rng, &sequence),
        }),
        21 => {
            let clips = some_clips(rng, &sequence);
            match rng.below(3) {
                0 => run!(SetLinked {
                    clips,
                    linked: rng.chance(50),
                }),
                1 => run!(SetGrouped {
                    clips,
                    grouped: rng.chance(50),
                }),
                _ => run!(JoinThroughEdits { clips }),
            }
        }
        22 => match rng.pick(&clip_ids(&sequence)) {
            Some(clip) => run!(SetTransition {
                clip,
                edge: some_edge(rng),
                transition: rng
                    .chance(80)
                    .then(|| Transition::new(TransitionKind::CrossDissolve, rng.ticks(-5..300))),
            }),
            None => run!(CloseGaps),
        },
        23 => {
            let kind = if rng.chance(50) {
                TrackKind::Video
            } else {
                TrackKind::Audio
            };
            match some_track(rng, &sequence) {
                Some(track) if sequence.tracks().count() > 6 || rng.chance(20) => {
                    run!(RemoveTrack { track })
                }
                _ => run!(AddTrack { kind }),
            }
        }
        24 => match some_track(rng, &sequence) {
            Some(track) => run!(SetTrackSwitch {
                track,
                switch: TrackSwitch::Lock,
                on: rng.chance(15),
            }),
            None => run!(CloseGaps),
        },
        25 => {
            let (clips, name) = (some_clips(rng, &sequence), "n".to_string());
            if rng.chance(50) {
                run!(NestClips { clips, name })
            } else {
                run!(MakeSubsequence { clips, name })
            }
        }
        _ => run!(SetLinkedSelection(if rng.chance(50) {
            LinkedSelection::Together
        } else {
            LinkedSelection::Apart
        })),
    }
}

/// Clip ids are unique across the sequence's tracks, which validation leaves to the edits.
fn check_ids(sequence: &Sequence) -> Result<(), String> {
    let mut seen = HashSet::new();
    match clip_ids(sequence).into_iter().find(|id| !seen.insert(*id)) {
        Some(id) => Err(format!("clip id {id:?} is used twice")),
        None => Ok(()),
    }
}

fn run_seed(seed: u64) {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let (mut timeline, sources) = fixture();
    for step in 0..STEPS {
        let before = timeline.sequence().clone();
        let (edit, result) = random_edit(&mut rng, &mut timeline, &sources);
        let context = || format!("seed {seed}, step {step}: {edit}");
        let after = timeline.sequence().clone();
        if result.is_err() {
            // Marks may have been set just before the edit that then failed.
            let mut unmarked = after.clone();
            unmarked.marks = before.marks;
            assert_eq!(
                unmarked,
                before,
                "failed edit changed the sequence, {}",
                context()
            );
            continue;
        }
        if let Err(problem) = after
            .validate()
            .map_err(|e| e.to_string())
            .and_then(|()| check_ids(&after))
        {
            panic!("{problem} after {}", context());
        }
        if rng.chance(25) {
            assert!(timeline.undo(), "nothing to undo after {}", context());
            let undone = timeline.sequence();
            let mut expected = before.clone();
            expected.marks = undone.marks;
            assert_eq!(
                undone,
                &expected,
                "undo missed the state before {}",
                context()
            );
            assert_eq!(timeline.redo(), Ok(true), "redo failed for {}", context());
            assert_eq!(
                timeline.sequence(),
                &after,
                "redo differs for {}",
                context()
            );
        }
    }
    // Undoing every kept step and redoing them all passes through the same states.
    let mut states = vec![timeline.sequence().clone()];
    while timeline.undo() {
        states.push(timeline.sequence().clone());
    }
    states.pop();
    while let Some(expected) = states.pop() {
        assert_eq!(timeline.redo(), Ok(true), "seed {seed}: redo all");
        assert_eq!(timeline.sequence(), &expected, "seed {seed}: redo all");
    }
}

#[test]
fn random_edits_keep_the_sequence_valid_and_undo_exact() {
    for seed in 0..SEEDS {
        run_seed(seed);
    }
}
