use super::ops::{insert, remove};
use crate::{
    Clip, ClipId, Edit, LinkId, MediaItem, Nest, Sequence, SourceId, Ticks, TimelineError,
    TrackKind, TrackRef,
};

/// Runs `edit` on the tracks, mix and markers of nest `nest` instead of the sequence's
/// own; media edits still act on the shared pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InNest<E> {
    pub nest: SourceId,
    pub edit: E,
}

impl<E: Edit> Edit for InNest<E> {
    fn name(&self) -> &'static str {
        self.edit.name()
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        sequence.swap_nest(self.nest)?;
        let result = self.edit.edit(sequence);
        // While swapped the nest holds the sequence's own tracks, so an edit that
        // removed it fails here and the working copy is thrown away.
        sequence.swap_nest(self.nest)?;
        result?;
        sequence.fit_nest(self.nest);
        Ok(())
    }
}

/// Moves clips and the clips linked to them into a new nested sequence, keeping their
/// tracks and spacing, and puts one clip of the nest where they were: on the lowest
/// video and audio tracks they used, linked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NestClips {
    pub clips: Vec<ClipId>,
    pub name: String,
}

impl Edit for NestClips {
    fn name(&self) -> &'static str {
        "Nest"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let ids = sequence.with_links(&self.clips);
        let moved = remove(sequence, &ids);
        let (Some(start), Some(end)) = (
            moved.iter().map(|(_, clip)| clip.start).min(),
            moved.iter().map(|(_, clip)| clip.end()).max(),
        ) else {
            return Err(TimelineError::NothingToEdit);
        };
        let lowest = |kind: TrackKind| {
            moved
                .iter()
                .filter(|(track, _)| track.kind == kind)
                .map(|(track, _)| track.index)
                .min()
        };
        let (video, audio) = (lowest(TrackKind::Video), lowest(TrackKind::Audio));
        let mut nest = Nest::default();
        for (track, mut clip) in moved {
            let tracks = match track.kind {
                TrackKind::Video => &mut nest.video,
                TrackKind::Audio => &mut nest.audio,
            };
            if tracks.len() <= track.index {
                tracks.resize_with(track.index + 1, Default::default);
            }
            clip.start = clip.start - start;
            tracks[track.index].clips.push(clip);
        }
        for track in nest.video.iter_mut().chain(&mut nest.audio) {
            track.clips.sort_by_key(|clip| clip.start);
        }

        let id = SourceId(sequence.ids.next());
        sequence.media.push(MediaItem {
            id,
            name: self.name.clone(),
            path: Default::default(),
            duration: end - start,
            video: video.map(|_| sequence.format),
            has_audio: audio.is_some(),
            proxy: None,
            bin: None,
            generator: None,
            nest: Some(nest),
            subclip: None,
        });
        let link = (video.is_some() && audio.is_some()).then(|| LinkId(sequence.ids.next()));
        let placements = [video.map(TrackRef::video), audio.map(TrackRef::audio)];
        for track in placements.into_iter().flatten() {
            let mut clip = Clip::new(
                ClipId(sequence.ids.next()),
                id,
                start,
                end - start,
                Ticks::ZERO,
            );
            clip.link = link;
            insert(sequence, track, clip)?;
        }
        Ok(())
    }
}

/// Adds an empty nested sequence to the pool, to edit on its own and place later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddNest {
    pub name: String,
}

impl Edit for AddNest {
    fn name(&self) -> &'static str {
        "New Sequence"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let id = SourceId(sequence.ids.next());
        sequence.media.push(MediaItem {
            id,
            name: self.name.clone(),
            path: Default::default(),
            duration: Ticks::ZERO,
            video: Some(sequence.format),
            has_audio: true,
            proxy: None,
            bin: None,
            generator: None,
            nest: Some(Nest::with_default_tracks()),
            subclip: None,
        });
        sequence.fit_nest(id);
        Ok(())
    }
}
