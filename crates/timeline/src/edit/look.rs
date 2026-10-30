use std::collections::HashSet;

use crate::edit::ops;
use crate::{ClipId, Edge, Edit, Look, Sequence, TimelineError, TrackKind, Transition};

/// Replaces a video clip's look: motion, opacity, crop and effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetClipLook {
    pub clip: ClipId,
    pub look: Look,
}

impl Edit for SetClipLook {
    fn name(&self) -> &'static str {
        "Effect Controls"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if sequence
            .clip(self.clip)
            .is_some_and(|(track, _)| track.kind != TrackKind::Video)
        {
            return Err(TimelineError::NothingToEdit);
        }
        let clip = ops::clip_mut(sequence, self.clip)?;
        if clip.look == self.look {
            return Err(TimelineError::NothingToEdit);
        }
        clip.look = self.look.clone();
        Ok(())
    }
}

/// Adds, replaces or removes the transition at one end of a clip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetTransition {
    pub clip: ClipId,
    pub edge: Edge,
    pub transition: Option<Transition>,
}

impl Edit for SetTransition {
    fn name(&self) -> &'static str {
        match self.transition {
            Some(_) => "Apply Transition",
            None => "Remove Transition",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let clip = ops::clip_mut(sequence, self.clip)?;
        let slot = match self.edge {
            Edge::Head => &mut clip.transition_in,
            Edge::Tail => &mut clip.transition_out,
        };
        if *slot == self.transition {
            return Err(TimelineError::NothingToEdit);
        }
        *slot = self.transition;
        Ok(())
    }
}

/// Puts `transition` at each edit point of `clips`: the cuts between two of them and
/// the ends that meet no other clip. Transitions already there stay; none runs longer
/// than the clips it joins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyTransitions {
    pub clips: Vec<ClipId>,
    pub transition: Transition,
}

impl Edit for ApplyTransitions {
    fn name(&self) -> &'static str {
        "Apply Default Transitions"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let chosen: HashSet<ClipId> = self.clips.iter().copied().collect();
        let mut edges = Vec::new();
        for (_, track) in sequence.tracks().filter(|(_, track)| !track.locked) {
            let clips = &track.clips;
            for (index, clip) in clips.iter().enumerate() {
                if !chosen.contains(&clip.id) {
                    continue;
                }
                let previous = index
                    .checked_sub(1)
                    .and_then(|previous| clips.get(previous))
                    .filter(|previous| previous.end() == clip.start);
                let next = clips.get(index + 1).filter(|next| next.start == clip.end());
                match previous {
                    _ if clip.transition_in.is_some() => {}
                    None => edges.push((clip.id, Edge::Head, clip.duration)),
                    Some(previous) if chosen.contains(&previous.id) => {
                        let shorter = clip.duration.min(previous.duration);
                        edges.push((clip.id, Edge::Head, shorter));
                    }
                    Some(_) => {}
                }
                if next.is_none() && clip.transition_out.is_none() {
                    edges.push((clip.id, Edge::Tail, clip.duration));
                }
            }
        }
        if edges.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        for (id, edge, longest) in edges {
            let transition = Some(Transition {
                duration: self.transition.duration.min(longest),
                ..self.transition
            });
            let clip = ops::clip_mut(sequence, id)?;
            match edge {
                Edge::Head => clip.transition_in = transition,
                Edge::Tail => clip.transition_out = transition,
            }
        }
        Ok(())
    }
}

/// Takes off every transition touching `clips`, including one held by the clip after a
/// chosen clip on the cut they share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveTransitions {
    pub clips: Vec<ClipId>,
}

impl Edit for RemoveTransitions {
    fn name(&self) -> &'static str {
        "Remove Transitions"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let chosen: HashSet<ClipId> = self.clips.iter().copied().collect();
        let mut cleared = Vec::new();
        for (_, track) in sequence.tracks().filter(|(_, track)| !track.locked) {
            let mut previous_chosen_end = None;
            for clip in &track.clips {
                let picked = chosen.contains(&clip.id);
                let clear_in = clip.transition_in.is_some()
                    && (picked || previous_chosen_end == Some(clip.start));
                let clear_out = picked && clip.transition_out.is_some();
                if clear_in || clear_out {
                    cleared.push((clip.id, clear_in, clear_out));
                }
                previous_chosen_end = picked.then(|| clip.end());
            }
        }
        if cleared.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        for (id, clear_in, clear_out) in cleared {
            let clip = ops::clip_mut(sequence, id)?;
            if clear_in {
                clip.transition_in = None;
            }
            if clear_out {
                clip.transition_out = None;
            }
        }
        Ok(())
    }
}
