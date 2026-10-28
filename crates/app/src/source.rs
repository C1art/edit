use std::sync::Arc;

use audio::AudioOutput;
use eframe::egui_wgpu::RenderState;
use playback::PlaybackError;
use timeline::{Clip, ClipId, Marks, MediaItem, Sequence, SourceId, Ticks, Track};

use crate::transport;
use crate::viewer::Viewer;

/// The source monitor: plays one media item so a range of it can be marked and edited
/// into the sequence.
pub struct SourceMonitor {
    pub source: SourceId,
    pub name: String,
    pub viewer: Viewer,
    pub marks: Marks,
}

impl SourceMonitor {
    /// `pool` holds the media a nested sequence's clips use.
    pub fn open(
        item: &MediaItem,
        pool: &Sequence,
        audio: Option<&AudioOutput>,
        render_state: Option<&RenderState>,
    ) -> Result<Self, PlaybackError> {
        let sequence = Arc::new(source_sequence(item, pool));
        Ok(Self {
            source: item.id,
            name: item.name.clone(),
            viewer: Viewer::new(sequence, audio, render_state)?,
            marks: Marks::default(),
        })
    }

    /// Picks up changes to the item shown, such as a proxy being attached.
    pub fn refresh(&mut self, item: &MediaItem, pool: &Sequence) {
        let player = &mut self.viewer.player;
        if player.sequence().media_item(item.id) != Some(item) {
            player.set_sequence(Arc::new(source_sequence(item, pool)));
        }
    }

    pub fn mark_in(&mut self) {
        self.marks = self.marks.with_in(self.viewer.player.position());
    }

    pub fn mark_out(&mut self) {
        self.marks = self
            .marks
            .with_out(transport::out_point(&self.viewer.player));
    }

    /// The marked source range as `(source_in, duration)`, defaulting to the whole item.
    pub fn range(&self) -> (Ticks, Ticks) {
        let marked = self.marks.range(self.viewer.player.duration());
        (marked.start, marked.end - marked.start)
    }
}

/// A sequence that shows `item` from start to end in its own format; generated media,
/// which has no end, shows for as long as an edit places by default.
fn source_sequence(item: &MediaItem, pool: &Sequence) -> Sequence {
    let timebase = pool.timebase;
    let mut sequence = Sequence::new(timebase, item.video.unwrap_or(pool.format));
    sequence.media.clone_from(&pool.media);
    let track = |id| Track {
        clips: vec![Clip::new(
            ClipId(id),
            item.id,
            Ticks::ZERO,
            item.default_length(timebase),
            Ticks::ZERO,
        )],
        ..Track::default()
    };
    if item.video.is_some() {
        sequence.video.push(track(1));
    }
    if item.has_audio {
        sequence.audio.push(track(2));
    }
    sequence
}
