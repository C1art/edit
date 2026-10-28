use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use timeline::{Clip, ClipId, MediaItem, Sequence, SourceId, Ticks, Track};

use crate::compose::{self, TrackDecoders};
use crate::{Layer, Resolution};

/// A rendered file standing in for the sequence's pictures over `range`, as a render
/// preview does in Premiere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub range: Range<Ticks>,
    pub path: PathBuf,
}

/// The previews laid out as a sequence of their own, a clip each, so they decode as
/// any clip does.
pub(crate) struct PreviewPictures {
    sequence: Sequence,
    tracks: Vec<TrackDecoders>,
}

impl PreviewPictures {
    pub(crate) fn new(previews: &[Preview], like: &Sequence) -> Option<Self> {
        if previews.is_empty() {
            return None;
        }
        let mut sequence = Sequence::new(like.timebase, like.format);
        let mut track = Track::default();
        for (index, preview) in (1..).zip(previews) {
            let duration = preview.range.end - preview.range.start;
            sequence.media.push(MediaItem {
                id: SourceId(index),
                name: String::new(),
                path: preview.path.clone(),
                duration,
                video: Some(like.format),
                has_audio: false,
                proxy: None,
                generator: None,
                nest: None,
                bin: None,
                subclip: None,
            });
            let clip = Clip::new(
                ClipId(index),
                SourceId(index),
                preview.range.start,
                duration,
                Ticks::ZERO,
            );
            track.clips.push(clip);
        }
        track.clips.sort_by_key(|clip| clip.start);
        sequence.video.push(track);
        Some(Self {
            sequence,
            tracks: Vec::new(),
        })
    }

    pub(crate) fn covers(&self, pts: Ticks) -> bool {
        self.sequence
            .video
            .first()
            .is_some_and(|track| track.clip_at(pts).is_some())
    }

    /// The preview's picture at `pts`, or `None` when its file can't be read, so the
    /// sequence is composed instead.
    pub(crate) fn layers_at(&mut self, pts: Ticks, resolution: Resolution) -> Option<Vec<Layer>> {
        let mut warned = Vec::new();
        let mut warnings = Vec::new();
        let layers = compose::layers_at(
            &self.sequence,
            pts,
            &mut self.tracks,
            &mut warned,
            &mut warnings,
            resolution,
        );
        warnings.is_empty().then_some(layers)
    }
}

/// The previews to play, shared between the player and its video worker.
pub(crate) type PreviewList = Arc<Vec<Preview>>;
