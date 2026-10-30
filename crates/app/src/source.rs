use std::ops::Range;
use std::sync::Arc;

use audio::AudioOutput;
use eframe::egui_wgpu::RenderState;
use playback::PlaybackError;
use timeline::{Clip, ClipId, Marks, MediaItem, Sequence, SourceId, Speed, Ticks, Track};

use crate::editor::Editor;
use crate::monitors::Focus;
use crate::transport;
use crate::viewer::Viewer;

/// The source monitor: plays one media item so a range of it can be marked and edited
/// into the sequence.
pub struct SourceMonitor {
    pub source: SourceId,
    pub name: String,
    pub viewer: Viewer,
    /// Monitor time zero in the media's time: the start of a subclip, else zero.
    offset: Ticks,
    /// How much faster the media plays than it was recorded, when interpreted at
    /// another frame rate; monitor time runs at that speed through the media.
    conform: Speed,
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
            offset: item.default_range(pool.timebase).start,
            conform: item.conform(),
            marks: Marks::default(),
        })
    }

    /// Picks up changes to the item shown, such as a proxy being attached.
    pub fn refresh(&mut self, item: &MediaItem, pool: &Sequence) {
        let player = &mut self.viewer.player;
        if player.sequence().media_item(item.id) != Some(item) {
            player.set_sequence(Arc::new(source_sequence(item, pool)));
            self.conform = item.conform();
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

    /// The marked source range as `(source_in, duration)` in the media's time,
    /// defaulting to the whole item.
    pub fn range(&self) -> (Ticks, Ticks) {
        self.media_range(self.marks.range(self.duration()))
    }

    /// The media start and length of a range of the monitor's time.
    pub fn media_range(&self, monitor: Range<Ticks>) -> (Ticks, Ticks) {
        let to_media = |span| self.conform.source_span(span);
        (
            self.offset + to_media(monitor.start),
            to_media(monitor.end - monitor.start),
        )
    }

    pub fn duration(&self) -> Ticks {
        self.viewer.player.duration()
    }

    pub fn left_at(&self) -> LeftAt {
        LeftAt {
            marks: self.marks,
            position: self.viewer.player.position(),
            marked: self.marks.is_set().then(|| self.range()),
        }
    }

    fn resume(&mut self, left: LeftAt) {
        self.marks = left.marks;
        self.viewer.player.seek(left.position);
    }

    /// Marks the media range `marked` and parks on media time `at`.
    pub fn show_media(&mut self, marked: Range<Ticks>, at: Ticks) {
        let end = self.viewer.player.duration();
        let to_monitor = |time: Ticks| {
            let into = (time - self.offset).max(Ticks::ZERO);
            self.conform.sequence_span(into).min(end)
        };
        self.marks = Marks {
            mark_in: Some(to_monitor(marked.start)),
            mark_out: Some(to_monitor(marked.end)),
        };
        self.viewer.player.seek(to_monitor(at));
    }
}

/// How the source monitor left an item it moved on from, so that going back to it
/// brings back its marks and position, as in Premiere.
#[derive(Debug, Clone, Copy)]
pub struct LeftAt {
    marks: Marks,
    position: Ticks,
    /// The marked media as `(source_in, duration)`, which dragging the item from the
    /// project places.
    marked: Option<(Ticks, Ticks)>,
}

/// A sequence that shows `item` from start to end in its own format; generated media,
/// which has no end, shows for as long as an edit places by default.
fn source_sequence(item: &MediaItem, pool: &Sequence) -> Sequence {
    let timebase = pool.timebase;
    let mut sequence = Sequence::new(timebase, item.played_format().unwrap_or(pool.format));
    sequence.media.clone_from(&pool.media);
    let range = item.default_range(timebase);
    let conform = item.conform();
    let duration = conform.sequence_span(range.end - range.start);
    // The monitor reads the media's own timecode, from where a subclip starts.
    sequence.start_time = item.start_timecode(timebase) + conform.sequence_span(range.start);
    let track = |id| Track {
        clips: vec![Clip {
            conform,
            ..Clip::new(ClipId(id), item.id, Ticks::ZERO, duration, range.start)
        }
        .into()],
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

impl Editor {
    pub(crate) fn open_source(&mut self, id: SourceId) {
        let Some(item) = self.timeline.sequence().media_item(id).cloned() else {
            return;
        };
        // Drop the old player first so its workers stop feeding the audio output.
        if let Some(source) = self.source.take() {
            self.left_sources.insert(source.source, source.left_at());
        }
        match SourceMonitor::open(
            &item,
            self.timeline.project(),
            self.audio.as_ref(),
            self.render_state.as_ref(),
        ) {
            Ok(mut source) => {
                if let Some(&left) = self.left_sources.get(&id) {
                    source.resume(left);
                }
                self.source = Some(source);
                self.focus = Focus::Source;
            }
            Err(error) => self.notice(format!("Cannot open {}: {error}", item.name)),
        }
    }

    /// The part of `id` marked in the source monitor, now or when it was last shown.
    pub(crate) fn source_marked(&self, id: SourceId) -> Option<(Ticks, Ticks)> {
        match &self.source {
            Some(source) if source.source == id => source.left_at().marked,
            _ => self.left_sources.get(&id)?.marked,
        }
    }
}
