use std::collections::HashMap;
use std::iter;

use super::ops::{self, Relinks};
use crate::{
    Clip, ClipId, Edit, LinkId, MediaItem, Nest, Sequence, SourceId, Ticks, TimelineError, Track,
    TrackKind, TrackRef,
};

/// Makes a multi-camera source of media items: each one with a picture becomes a camera
/// on its own video track. Every camera's sound is kept on a track of its own, but only
/// the first is heard until another track is unmuted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateMulticam {
    pub sources: Vec<SourceId>,
    /// Where each source starts relative to the others, so they play in sync; missing
    /// ones start with the earliest.
    pub starts: Vec<Ticks>,
    pub name: String,
}

impl Edit for CreateMulticam {
    fn name(&self) -> &'static str {
        "Create Multi-Camera Source Sequence"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let mut nest = Nest {
            multicam: true,
            ..Nest::default()
        };
        let starts = self.starts.iter().copied().chain(iter::repeat(Ticks::ZERO));
        let mut cameras = Vec::new();
        for (&source, start) in self.sources.iter().zip(starts) {
            let item = sequence
                .media_item(source)
                .ok_or(TimelineError::MediaNotFound(source))?;
            if item.video.is_some() && item.is_file() {
                cameras.push((item.clone(), start));
            }
        }
        let earliest = cameras.iter().map(|&(_, start)| start).min();
        let bin = cameras.iter().find_map(|(item, _)| item.bin);
        for (item, start) in cameras {
            let start = start - earliest.unwrap_or(Ticks::ZERO);
            let link = item.has_audio.then(|| LinkId(sequence.ids.next()));
            let mut camera = || {
                let mut clip = Clip::new(
                    ClipId(sequence.ids.next()),
                    item.id,
                    start,
                    item.duration,
                    Ticks::ZERO,
                );
                clip.link = link;
                Track {
                    clips: vec![clip],
                    ..Track::default()
                }
            };
            nest.video.push(camera());
            if item.has_audio {
                let mut sound = camera();
                sound.muted = !nest.audio.is_empty();
                nest.audio.push(sound);
            }
        }
        if nest.video.len() < 2 {
            return Err(TimelineError::NothingToEdit);
        }
        let id = SourceId(sequence.ids.next());
        sequence.media.push(MediaItem {
            id,
            name: self.name.clone(),
            path: Default::default(),
            duration: Ticks::ZERO,
            video: Some(sequence.format),
            has_audio: false,
            proxy: None,
            generator: None,
            nest: Some(nest),
            bin,
            subclip: None,
            interpreted_rate: None,
            log: Default::default(),
        });
        sequence.fit_nest(id);
        Ok(())
    }
}

/// Shows another camera of a multi-camera clip. With `at` inside the clip, the clip and
/// those linked to it are cut there first and only the part after switches, as when
/// cutting between cameras while the sequence plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchAngle {
    pub clip: ClipId,
    pub angle: usize,
    pub at: Option<Ticks>,
}

impl Edit for SwitchAngle {
    fn name(&self) -> &'static str {
        "Switch Camera"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let (track, clip) = sequence
            .clip(self.clip)
            .ok_or(TimelineError::ClipNotFound(self.clip))?;
        let angles = sequence
            .multicam_of(clip)
            .map_or(0, |nest| nest.video.len());
        if self.angle >= angles {
            return Err(TimelineError::NothingToEdit);
        }
        let mut switched = self.clip;
        if let Some(at) = self.at.filter(|&at| clip.start < at && at < clip.end()) {
            if clip.angle == self.angle {
                return Err(TimelineError::NothingToEdit);
            }
            let tracks: Vec<_> = sequence
                .links_of(&[self.clip])
                .into_iter()
                .filter_map(|id| sequence.clip(id).map(|(track, _)| track))
                .collect();
            ops::split(sequence, &tracks, at, &mut Relinks::default());
            switched = sequence
                .track(track)
                .and_then(|track| track.clip_at(at))
                .map(|clip| clip.id)
                .ok_or(TimelineError::ClipNotFound(self.clip))?;
        }
        let clip = ops::clip_mut(sequence, switched)?;
        if clip.angle == self.angle {
            return Err(TimelineError::NothingToEdit);
        }
        clip.angle = self.angle;
        Ok(())
    }
}

/// Replaces multi-camera clips with the clips of the cameras they show, and their sound
/// with that of the one camera heard, keeping their effects, so the cut no longer needs
/// the multi-camera source. Clips at another speed, and sound mixed from several
/// cameras, stay as they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlattenMulticam {
    pub clips: Vec<ClipId>,
}

impl Edit for FlattenMulticam {
    fn name(&self) -> &'static str {
        "Flatten Multi-Camera"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let mut relinks = HashMap::new();
        let mut flattened = false;
        for id in sequence.links_of(&self.clips) {
            let Some((track, pieces)) = flat_pieces(sequence, id) else {
                continue;
            };
            ops::remove(sequence, &[id]);
            for mut piece in pieces {
                piece.id = ClipId(sequence.ids.next());
                piece.link = piece.link.map(|old| {
                    *relinks
                        .entry(old)
                        .or_insert_with(|| LinkId(sequence.ids.next()))
                });
                ops::insert(sequence, track, piece)?;
            }
            flattened = true;
        }
        if flattened {
            Ok(())
        } else {
            Err(TimelineError::NothingToEdit)
        }
    }
}

/// The clips of the camera multi-camera clip `id` shows over its span, each with the
/// clip's settings; None when it isn't a multi-camera clip that can be flattened.
fn flat_pieces(sequence: &Sequence, id: ClipId) -> Option<(TrackRef, Vec<Clip>)> {
    let (track, clip) = sequence.clip(id)?;
    let nest = sequence.multicam_of(clip)?;
    if !plays_straight(clip) {
        return None;
    }
    let camera = match track.kind {
        TrackKind::Video => nest.video.get(clip.angle)?,
        TrackKind::Audio => {
            let mut heard = nest.audio.iter().filter(|track| !track.muted);
            match (heard.next(), heard.next()) {
                (Some(track), None) => track,
                _ => return None,
            }
        }
    };
    let (from, to) = (clip.source_in, clip.source_in + clip.duration);
    let pieces = camera
        .clips
        .iter()
        .filter(|inner| plays_straight(inner) && inner.start < to && from < inner.end())
        .map(|inner| {
            let (start, end) = (inner.start.max(from), inner.end().min(to));
            let mut piece = Clip {
                source: inner.source,
                start: clip.start + (start - from),
                duration: end - start,
                source_in: inner.source_in + (start - inner.start),
                transition_in: clip.transition_in.filter(|_| start == from),
                transition_out: clip.transition_out.filter(|_| end == to),
                angle: 0,
                ..clip.clone()
            };
            // Keyframes are kept in source time, which is the camera's now.
            let shift = inner.source_in - inner.start;
            let params = piece
                .look
                .params_mut()
                .chain([&mut piece.volume, &mut piece.pan])
                .chain(
                    piece
                        .audio_effects
                        .iter_mut()
                        .flat_map(|effect| effect.params_mut()),
                );
            for param in params {
                for key in &mut param.keys {
                    key.time = key.time + shift;
                }
            }
            piece
        })
        .collect();
    Some((track, pieces))
}

fn plays_straight(clip: &Clip) -> bool {
    clip.rate().is_normal() && !clip.reverse && !clip.hold && clip.remap.is_none()
}
