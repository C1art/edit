use egui::Rangef;
use timeline::{
    preview, ClipId, DeltaBounds, Edge, Edit, LinkedSelection, MoveClips, MoveMode, RateStretch,
    Roll, Sequence, SetClipOpacity, SetClipVolume, SetTrackAutomation, SetTransition, Slide, Slip,
    Ticks, Timeline, TimelineError, TrackKind, TrackRef, Trim, TrimMode, Unlinked,
};

use super::rubber_band::{self, BandHit, BandOwner, Envelope};
use super::transition_grip::Grip;

/// What a drag in the timeline is doing, fixed when the drag starts.
#[derive(Debug, Clone)]
pub enum Gesture {
    Move {
        clips: Vec<ClipId>,
        from: TrackRef,
    },
    Trim {
        clips: Vec<ClipId>,
        edge: Edge,
        mode: TrimMode,
    },
    Roll {
        at: Ticks,
        tracks: Vec<TrackRef>,
    },
    Stretch {
        clips: Vec<ClipId>,
        edge: Edge,
    },
    Slip {
        clips: Vec<ClipId>,
    },
    Slide {
        clips: Vec<ClipId>,
    },
    /// Drags a volume line or one of its keyframes.
    Volume {
        owner: BandOwner,
        grabbed: BandHit,
        band: Rangef,
        origin_y: f32,
    },
    /// Drags an outer end of a transition to change how long it lasts.
    TransitionLength(Grip),
}

/// An edit a gesture would make, previewed while dragging and applied on release.
#[derive(Debug, Clone, PartialEq)]
pub enum PendingEdit {
    Move(MoveClips),
    Trim(Trim),
    Roll(Roll),
    Stretch(RateStretch),
    Slip(Slip),
    Slide(Slide),
    Volume(SetClipVolume),
    Opacity(SetClipOpacity),
    Automation(SetTrackAutomation),
    Transition(SetTransition),
}

impl PendingEdit {
    pub fn preview(&self, sequence: &Sequence) -> Result<Sequence, TimelineError> {
        match self {
            Self::Move(edit) => preview(sequence, edit),
            Self::Trim(edit) => preview(sequence, edit),
            Self::Roll(edit) => preview(sequence, edit),
            Self::Stretch(edit) => preview(sequence, edit),
            Self::Slip(edit) => preview(sequence, edit),
            Self::Slide(edit) => preview(sequence, edit),
            Self::Volume(edit) => preview(sequence, edit),
            Self::Opacity(edit) => preview(sequence, edit),
            Self::Automation(edit) => preview(sequence, edit),
            Self::Transition(edit) => preview(sequence, edit),
        }
    }

    /// Applies the edit; `Apart` keeps it to the clips it names, linked or not.
    pub fn apply(
        self,
        timeline: &mut Timeline,
        links: LinkedSelection,
    ) -> Result<(), TimelineError> {
        fn run(
            timeline: &mut Timeline,
            edit: impl Edit,
            links: LinkedSelection,
        ) -> Result<(), TimelineError> {
            match links {
                LinkedSelection::Together => timeline.apply(edit),
                LinkedSelection::Apart => timeline.apply(Unlinked(edit)),
            }
        }
        match self {
            Self::Move(edit) => run(timeline, edit, links),
            Self::Trim(edit) => run(timeline, edit, links),
            Self::Roll(edit) => run(timeline, edit, links),
            Self::Stretch(edit) => run(timeline, edit, links),
            Self::Slip(edit) => run(timeline, edit, links),
            Self::Slide(edit) => run(timeline, edit, links),
            Self::Volume(edit) => run(timeline, edit, links),
            Self::Opacity(edit) => run(timeline, edit, links),
            Self::Automation(edit) => run(timeline, edit, links),
            Self::Transition(edit) => run(timeline, edit, links),
        }
    }
}

/// `sequence` with linked selection off, for working out an Alt-drag that moves or trims
/// one side of a linked pair.
pub fn apart(sequence: &Sequence) -> Sequence {
    let mut apart = sequence.clone();
    apart.linked_selection = LinkedSelection::Apart;
    apart
}

impl Gesture {
    /// The sequence times that should snap while this gesture moves them.
    pub fn moving_edges(&self, sequence: &Sequence) -> Vec<Ticks> {
        match self {
            Self::Move { clips, .. } | Self::Slide { clips } => sequence
                .clips_with_links(clips)
                .into_iter()
                .flat_map(|(_, clip)| [clip.start, clip.end()])
                .collect(),
            Self::Trim { clips, edge, .. } | Self::Stretch { clips, edge } => sequence
                .clips_with_links(clips)
                .into_iter()
                .map(|(_, clip)| match edge {
                    Edge::Head => clip.start,
                    Edge::Tail => clip.end(),
                })
                .collect(),
            Self::Roll { at, .. } => vec![*at],
            Self::Slip { .. } => Vec::new(),
            Self::Volume {
                owner,
                grabbed: BandHit::Key(index),
                ..
            } => Envelope::of(sequence, *owner, sequence.duration())
                .and_then(|envelope| envelope.key_position(*index))
                .into_iter()
                .collect(),
            Self::Volume { .. } => Vec::new(),
            Self::TransitionLength(grip) => vec![grip.at],
        }
    }

    /// Clips whose own edges must not be snap targets.
    pub fn moving_clips(&self, sequence: &Sequence) -> Vec<ClipId> {
        match self {
            Self::Move { clips, .. }
            | Self::Trim { clips, .. }
            | Self::Stretch { clips, .. }
            | Self::Slip { clips }
            | Self::Slide { clips } => sequence.with_links(clips),
            Self::Roll { .. } | Self::Volume { .. } | Self::TransitionLength(_) => Vec::new(),
        }
    }

    /// The edit for a drag of `delta` (already snapped and frame-aligned) that ended over
    /// `track` at height `y`, clamped to what the sequence allows; a move does `mode`.
    pub fn edit(
        &self,
        sequence: &Sequence,
        delta: Ticks,
        track: Option<TrackRef>,
        y: f32,
        mode: MoveMode,
        bounds: &mut DeltaBounds,
    ) -> PendingEdit {
        match self {
            Self::Move { clips, from } => {
                PendingEdit::Move(clamped_move(sequence, clips, *from, delta, track, mode))
            }
            Self::Trim { clips, edge, mode } => {
                let make = |delta| Trim {
                    clips: clips.clone(),
                    edge: *edge,
                    delta,
                    mode: *mode,
                };
                PendingEdit::Trim(make(bounds.clamp(sequence, delta, make)))
            }
            Self::Roll { at, tracks } => {
                let make = |delta| Roll {
                    at: *at,
                    tracks: tracks.clone(),
                    delta,
                };
                PendingEdit::Roll(make(bounds.clamp(sequence, delta, make)))
            }
            Self::Stretch { clips, edge } => {
                let make = |delta| RateStretch {
                    clips: clips.clone(),
                    edge: *edge,
                    delta,
                };
                PendingEdit::Stretch(make(bounds.clamp(sequence, delta, make)))
            }
            Self::Slip { clips } => {
                // Dragging right reveals earlier source material, as in other editors.
                let make = |delta| Slip {
                    clips: clips.clone(),
                    delta,
                };
                PendingEdit::Slip(make(bounds.clamp(sequence, Ticks(-delta.0), make)))
            }
            Self::Slide { clips } => {
                let make = |delta| Slide {
                    clips: clips.clone(),
                    delta,
                };
                PendingEdit::Slide(make(bounds.clamp(sequence, delta, make)))
            }
            Self::Volume {
                owner,
                grabbed,
                band,
                origin_y,
            } => {
                let param = Envelope::of(sequence, *owner, sequence.duration()).map_or_else(
                    Default::default,
                    |envelope| {
                        let change = envelope.change_for_drag(*band, y - origin_y);
                        rubber_band::dragged(&envelope, *grabbed, delta, change)
                    },
                );
                match *owner {
                    BandOwner::Clip(clip) => PendingEdit::Volume(SetClipVolume {
                        clip,
                        volume: param,
                    }),
                    BandOwner::Opacity(clip) => PendingEdit::Opacity(SetClipOpacity {
                        clip,
                        opacity: param,
                    }),
                    BandOwner::Track(track) => PendingEdit::Automation(SetTrackAutomation {
                        track,
                        automation: param,
                    }),
                }
            }
            Self::TransitionLength(grip) => {
                let frame = sequence.format.frame_rate.frame_start(1, sequence.timebase);
                PendingEdit::Transition(SetTransition {
                    clip: grip.clip,
                    edge: grip.edge,
                    transition: Some(grip.dragged(delta, frame)),
                })
            }
        }
    }
}

fn clamped_move(
    sequence: &Sequence,
    clips: &[ClipId],
    from: TrackRef,
    delta: Ticks,
    over: Option<TrackRef>,
    mode: MoveMode,
) -> MoveClips {
    let moved: Vec<(TrackRef, Ticks)> = sequence
        .clips_with_links(clips)
        .into_iter()
        .map(|(track, clip)| (track, clip.start))
        .collect();
    let earliest = moved.iter().map(|&(_, start)| start).min();
    let delta = earliest.map_or(delta, |earliest| delta.max(Ticks::ZERO - earliest));
    let lowest = |kind| {
        moved
            .iter()
            .filter(|(track, _)| track.kind == kind)
            .map(|(track, _)| track.index)
            .min()
    };
    let offset = over
        .filter(|over| over.kind == from.kind)
        .map_or(0, |over| over.index as isize - from.index as isize);
    let offset = lowest(from.kind).map_or(0, |lowest| offset.max(-(lowest as isize)));
    let (video_tracks, audio_tracks) = match from.kind {
        TrackKind::Video => (offset, 0),
        TrackKind::Audio => (0, offset),
    };
    MoveClips {
        clips: clips.to_vec(),
        delta,
        video_tracks,
        audio_tracks,
        mode,
    }
}
