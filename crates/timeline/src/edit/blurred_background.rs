//! A blurred copy of a clip filling the frame beneath it, for pictures of another shape
//! than the sequence, as horizontal footage in a vertical sequence.

use std::collections::BTreeMap;
use std::ops::Range;

use super::add_tracks::renumber_layers;
use super::frame_size::scale_hundredths;
use crate::edit::ops;
use crate::{
    Clip, ClipId, Edit, FrameFit, GroupId, Look, MediaItem, Motion, Param, Sequence, Ticks,
    TimelineError, Track, TrackKind, TrackRef, VideoEffect,
};

/// Enough to leave only the colors and shapes of the picture.
const BLURRINESS: f32 = 60.0;

/// Puts under each video clip a copy scaled to fill the frame and blurred, grouped
/// with it, on the free track below or a new one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddBlurredBackground {
    pub clips: Vec<ClipId>,
}

impl Edit for AddBlurredBackground {
    fn name(&self) -> &'static str {
        "Add Blurred Background"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let mut by_track: BTreeMap<usize, Vec<ClipId>> = BTreeMap::new();
        for &id in &self.clips {
            let (track, clip) = sequence.clip(id).ok_or(TimelineError::ClipNotFound(id))?;
            let has_picture = sequence
                .media_item(clip.source)
                .and_then(MediaItem::played_format)
                .is_some();
            if track.kind == TrackKind::Video && has_picture {
                by_track.entry(track.index).or_default().push(id);
            }
        }
        if by_track.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        // From the top down, so a track added below one doesn't move those yet to do.
        for (index, ids) in by_track.into_iter().rev() {
            let spans: Vec<Range<Ticks>> = ids
                .iter()
                .filter_map(|&id| sequence.clip(id))
                .map(|(_, clip)| clip.start..clip.end())
                .collect();
            let below = match index.checked_sub(1) {
                Some(below) if is_free(&sequence.tracks_of(TrackKind::Video)[below], &spans) => {
                    below
                }
                _ => {
                    sequence
                        .tracks_of_mut(TrackKind::Video)
                        .insert(index, Track::default());
                    renumber_layers(sequence, |layer| {
                        Some(if layer >= index { layer + 1 } else { layer })
                    });
                    index
                }
            };
            for id in ids {
                let background = background_of(sequence, id)?;
                ops::insert(sequence, TrackRef::video(below), background)?;
            }
        }
        Ok(())
    }
}

fn is_free(track: &Track, spans: &[Range<Ticks>]) -> bool {
    !track.locked
        && spans.iter().all(|span| {
            track
                .clips
                .iter()
                .all(|clip| clip.end() <= span.start || clip.start >= span.end)
        })
}

/// The blurred copy of clip `id`, which joins it in a group.
fn background_of(sequence: &mut Sequence, id: ClipId) -> Result<Clip, TimelineError> {
    let frame = sequence.format;
    let (_, clip) = sequence.clip(id).ok_or(TimelineError::ClipNotFound(id))?;
    let item = sequence
        .media_item(clip.source)
        .ok_or(TimelineError::NothingToEdit)?;
    let picture = item.played_format().ok_or(TimelineError::NothingToEdit)?;
    let shown_name = clip.name.as_deref().unwrap_or(&item.name);
    let name = format!("{shown_name} (Blurred Background)");
    let scale = Param::new(scale_hundredths(picture, frame, FrameFit::Fill));
    let group = match clip.group {
        Some(group) => group,
        None => {
            let group = GroupId(sequence.ids.next());
            ops::clip_mut(sequence, id)?.group = Some(group);
            group
        }
    };
    let (_, clip) = sequence.clip(id).ok_or(TimelineError::ClipNotFound(id))?;
    let clip = clip.clone();
    let look = Look {
        motion: Motion {
            scale_width: scale.clone(),
            scale,
            ..Motion::default()
        },
        effects: vec![VideoEffect::GaussianBlur {
            blurriness: Param::from_value(BLURRINESS),
        }],
        color: clip.look.color.clone(),
        ..Look::default()
    };
    Ok(Clip {
        id: ClipId(sequence.ids.next()),
        name: Some(name),
        link: None,
        group: Some(group),
        look,
        ..clip
    })
}
