use timeline::{
    clamp_delta, preview, ClipId, Edge, MoveClips, Roll, Sequence, Slide, Slip, Ticks, Timeline,
    TimelineError, TrackKind, TrackRef, Trim, TrimMode,
};

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
    Slip {
        clips: Vec<ClipId>,
    },
    Slide {
        clips: Vec<ClipId>,
    },
}

/// An edit a gesture would make, previewed while dragging and applied on release.
#[derive(Debug, Clone)]
pub enum PendingEdit {
    Move(MoveClips),
    Trim(Trim),
    Roll(Roll),
    Slip(Slip),
    Slide(Slide),
}

impl PendingEdit {
    pub fn preview(&self, sequence: &Sequence) -> Result<Sequence, TimelineError> {
        match self {
            Self::Move(edit) => preview(sequence, edit),
            Self::Trim(edit) => preview(sequence, edit),
            Self::Roll(edit) => preview(sequence, edit),
            Self::Slip(edit) => preview(sequence, edit),
            Self::Slide(edit) => preview(sequence, edit),
        }
    }

    pub fn apply(self, timeline: &mut Timeline) -> Result<(), TimelineError> {
        match self {
            Self::Move(edit) => timeline.apply(edit),
            Self::Trim(edit) => timeline.apply(edit),
            Self::Roll(edit) => timeline.apply(edit),
            Self::Slip(edit) => timeline.apply(edit),
            Self::Slide(edit) => timeline.apply(edit),
        }
    }
}

impl Gesture {
    /// The sequence times that should snap while this gesture moves them.
    pub fn moving_edges(&self, sequence: &Sequence) -> Vec<Ticks> {
        match self {
            Self::Move { clips, .. } | Self::Slide { clips } => sequence
                .with_links(clips)
                .into_iter()
                .filter_map(|id| sequence.clip(id))
                .flat_map(|(_, clip)| [clip.start, clip.end()])
                .collect(),
            Self::Trim { clips, edge, .. } => sequence
                .with_links(clips)
                .into_iter()
                .filter_map(|id| sequence.clip(id))
                .map(|(_, clip)| match edge {
                    Edge::Head => clip.start,
                    Edge::Tail => clip.end(),
                })
                .collect(),
            Self::Roll { at, .. } => vec![*at],
            Self::Slip { .. } => Vec::new(),
        }
    }

    /// Clips whose own edges must not be snap targets.
    pub fn moving_clips(&self, sequence: &Sequence) -> Vec<ClipId> {
        match self {
            Self::Move { clips, .. }
            | Self::Trim { clips, .. }
            | Self::Slip { clips }
            | Self::Slide { clips } => sequence.with_links(clips),
            Self::Roll { .. } => Vec::new(),
        }
    }

    /// The edit for a drag of `delta` (already snapped and frame-aligned) that ended over
    /// `track`, clamped to what the sequence allows.
    pub fn edit(&self, sequence: &Sequence, delta: Ticks, track: Option<TrackRef>) -> PendingEdit {
        match self {
            Self::Move { clips, from } => {
                PendingEdit::Move(clamped_move(sequence, clips, *from, delta, track))
            }
            Self::Trim { clips, edge, mode } => {
                let make = |delta| Trim {
                    clips: clips.clone(),
                    edge: *edge,
                    delta,
                    mode: *mode,
                };
                PendingEdit::Trim(make(clamp_delta(sequence, delta, make)))
            }
            Self::Roll { at, tracks } => {
                let make = |delta| Roll {
                    at: *at,
                    tracks: tracks.clone(),
                    delta,
                };
                PendingEdit::Roll(make(clamp_delta(sequence, delta, make)))
            }
            Self::Slip { clips } => {
                // Dragging right reveals earlier source material, as in other editors.
                let make = |delta| Slip {
                    clips: clips.clone(),
                    delta,
                };
                PendingEdit::Slip(make(clamp_delta(sequence, Ticks(-delta.0), make)))
            }
            Self::Slide { clips } => {
                let make = |delta| Slide {
                    clips: clips.clone(),
                    delta,
                };
                PendingEdit::Slide(make(clamp_delta(sequence, delta, make)))
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
) -> MoveClips {
    let moved: Vec<(TrackRef, Ticks)> = sequence
        .with_links(clips)
        .into_iter()
        .filter_map(|id| sequence.clip(id))
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
    }
}
