use std::sync::Arc;

use crate::{
    Clip, ClipId, Edit, LinkId, MediaItem, Nest, Sequence, SourceId, Ticks, TimelineError, Track,
};

/// Makes a merged clip of a camera's picture and sound recorded apart: a sequence with
/// the picture on the first video track and each recording's sound on an audio track of
/// its own, all linked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeClips {
    /// The first item with a picture gives it; every other item gives only its sound.
    pub sources: Vec<SourceId>,
    /// Where each source starts relative to the others; missing ones start with the
    /// earliest.
    pub starts: Vec<Ticks>,
    /// Leaves out the sound recorded with the picture.
    pub remove_camera_audio: bool,
    pub name: String,
}

impl Edit for MergeClips {
    fn name(&self) -> &'static str {
        "Merge Clips"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let starts = self
            .starts
            .iter()
            .copied()
            .chain(std::iter::repeat(Ticks::ZERO));
        let mut items = Vec::new();
        for (&source, start) in self.sources.iter().zip(starts) {
            let item = sequence
                .media_item(source)
                .ok_or(TimelineError::MediaNotFound(source))?;
            items.push((item.clone(), start));
        }
        let camera = items
            .iter()
            .position(|(item, _)| item.video.is_some())
            .ok_or(TimelineError::NothingToEdit)?;
        let earliest = items.iter().map(|&(_, start)| start).min();
        let link = LinkId(sequence.ids.next());
        let mut nest = Nest::default();
        for (index, (item, start)) in items.iter().enumerate() {
            let mut clip = Clip::new(
                ClipId(sequence.ids.next()),
                item.id,
                *start - earliest.unwrap_or(Ticks::ZERO),
                item.duration,
                Ticks::ZERO,
            );
            clip.link = Some(link);
            let track = |clip: Clip| Track {
                clips: vec![clip.into()],
                ..Track::default()
            };
            let is_camera = index == camera;
            let keeps_sound = item.has_audio && !(is_camera && self.remove_camera_audio);
            if is_camera {
                nest.video.push(track(clip.clone()));
            }
            if keeps_sound {
                clip.id = ClipId(sequence.ids.next());
                nest.audio.push(track(clip));
            }
        }
        if nest.audio.is_empty() || items.len() < 2 {
            return Err(TimelineError::NothingToEdit);
        }
        let id = SourceId(sequence.ids.next());
        let bin = items[camera].0.bin;
        Arc::make_mut(&mut sequence.media).push(MediaItem {
            id,
            name: self.name.clone(),
            path: Default::default(),
            duration: Ticks::ZERO,
            video: Some(sequence.format),
            has_audio: true,
            proxy: None,
            generator: None,
            nest: Some(nest),
            bin,
            subclip: None,
            interpreted_rate: None,
            interpreted_aspect: None,
            interpreted_color: None,
            interpreted_fields: None,
            alpha: Default::default(),
            log: Default::default(),
            offline: false,
            still: false,
            channel_map: None,
        });
        sequence.fit_nest(id);
        Ok(())
    }
}
