use std::sync::Arc;

use audio::AudioOutput;
use eframe::egui_wgpu::RenderState;
use playback::PlaybackError;
use timeline::{Clip, ClipId, MediaItem, Sequence, SourceId, Ticks, Timebase, Track};

use crate::transport::Marks;
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
    pub fn open(
        item: &MediaItem,
        timebase: Timebase,
        audio: Option<&AudioOutput>,
        render_state: Option<&RenderState>,
    ) -> Result<Self, PlaybackError> {
        let sequence = Arc::new(source_sequence(item, timebase));
        Ok(Self {
            source: item.id,
            name: item.name.clone(),
            viewer: Viewer::new(sequence, audio, render_state)?,
            marks: Marks::default(),
        })
    }

    /// Picks up changes to the item shown, such as a proxy being attached.
    pub fn refresh(&mut self, item: &MediaItem) {
        let player = &mut self.viewer.player;
        if player.sequence().media.first() != Some(item) {
            let timebase = player.sequence().timebase;
            player.set_sequence(Arc::new(source_sequence(item, timebase)));
        }
    }

    pub fn mark_in(&mut self) {
        let position = self.viewer.player.position();
        self.marks.mark_in = Some(position);
        if self.marks.mark_out.is_some_and(|out| out <= position) {
            self.marks.mark_out = None;
        }
    }

    pub fn mark_out(&mut self) {
        let player = &self.viewer.player;
        // The out mark includes the frame on screen, as in other editors.
        let rate = player.frame_rate();
        let timebase = player.sequence().timebase;
        let frame = rate.frame_at(player.position(), timebase);
        let out = rate.frame_start(frame + 1, timebase).min(player.duration());
        self.marks.mark_out = Some(out);
        if self.marks.mark_in.is_some_and(|mark_in| mark_in >= out) {
            self.marks.mark_in = None;
        }
    }

    /// The marked source range as `(source_in, duration)`, defaulting to the whole item.
    pub fn range(&self) -> (Ticks, Ticks) {
        let mark_in = self.marks.mark_in.unwrap_or(Ticks::ZERO);
        let mark_out = self.marks.mark_out.unwrap_or(self.viewer.player.duration());
        (mark_in, mark_out - mark_in)
    }
}

/// A sequence that shows `item` from start to end in its own format; generated media,
/// which has no end, shows for as long as an edit places by default.
fn source_sequence(item: &MediaItem, timebase: Timebase) -> Sequence {
    let mut sequence = Sequence::new(timebase, item.video.unwrap_or_default());
    sequence.media.push(item.clone());
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
