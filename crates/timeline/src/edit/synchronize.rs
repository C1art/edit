use std::collections::HashSet;

use crate::{ClipId, Edit, MoveClips, MoveMode, Sequence, Ticks, TimelineError, TrackKind};

/// Moves clips, each with the clips linked to it, so each starts where it's paired with,
/// as Premiere's Synchronize lines up clips recorded together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynchronizeClips {
    pub starts: Vec<(ClipId, Ticks)>,
}

impl Edit for SynchronizeClips {
    fn name(&self) -> &'static str {
        "Synchronize"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let mut moved = false;
        for &(clip, start) in &self.starts {
            let (_, placed) = sequence
                .clip(clip)
                .ok_or(TimelineError::ClipNotFound(clip))?;
            let delta = start - placed.start;
            if delta == Ticks::ZERO {
                continue;
            }
            let move_clip = MoveClips {
                clips: vec![clip],
                delta,
                video_tracks: 0,
                audio_tracks: 0,
                mode: MoveMode::Overwrite,
            };
            move_clip.edit(sequence)?;
            moved = true;
        }
        if moved {
            Ok(())
        } else {
            Err(TimelineError::NothingToEdit)
        }
    }
}

/// One clip for each linked set among `selection`, its sound where it has one, earliest
/// first: what Synchronize lines up.
pub fn synchronized_clips(sequence: &Sequence, selection: &[ClipId]) -> Vec<ClipId> {
    let mut seen = HashSet::new();
    let mut picked: Vec<_> = sequence
        .clips_with_links(selection)
        .into_iter()
        .filter(|(_, clip)| selection.contains(&clip.id))
        .filter_map(|(_, clip)| {
            let set = sequence.links_of(&[clip.id]);
            if !seen.insert(set.clone()) {
                return None;
            }
            let sound = set.iter().find_map(|&id| {
                let (track, linked) = sequence.clip(id)?;
                (track.kind == TrackKind::Audio).then_some(linked)
            });
            let chosen = sound.unwrap_or(clip);
            Some((chosen.start, chosen.id))
        })
        .collect();
    picked.sort();
    picked.into_iter().map(|(_, id)| id).collect()
}
