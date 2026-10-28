use timeline::{
    Edge, ExtractMarked, LiftMarked, Marks, RippleTrimToPlayhead, SetMarks, Ticks, TrackKind,
};

use crate::editor::Editor;
use crate::monitors::Focus;
use crate::transport;

impl Editor {
    pub(crate) fn mark_sequence_in(&mut self) {
        let marks = self.timeline.sequence().marks.with_in(self.playhead());
        self.apply_levels(SetMarks { marks });
    }

    pub(crate) fn mark_sequence_out(&mut self) {
        let out = self.program.as_ref().map_or(self.playhead(), |program| {
            transport::out_point(&program.player)
        });
        let marks = self.timeline.sequence().marks.with_out(out);
        self.apply_levels(SetMarks { marks });
    }

    pub(crate) fn clear_sequence_marks(&mut self) {
        self.apply_levels(SetMarks {
            marks: Marks::default(),
        });
    }

    pub(crate) fn lift_marked(&mut self) {
        if !self.apply(LiftMarked) {
            self.notice("Mark in and out around what to lift");
        }
    }

    /// Leaves the playhead where the extracted range closed up.
    pub(crate) fn extract_marked(&mut self) {
        let marks = self.timeline.sequence().marks;
        if self.apply(ExtractMarked) {
            self.seek_program(marks.mark_in.unwrap_or(Ticks::ZERO));
        } else {
            self.notice("Mark in and out around what to extract");
        }
    }

    pub(crate) fn ripple_trim_to_playhead(&mut self, edge: Edge) {
        let trim = RippleTrimToPlayhead {
            at: self.playhead(),
            edge,
        };
        let Some(span) = trim.span(self.timeline.sequence()) else {
            self.notice("Put the playhead over a clip to trim it");
            return;
        };
        // What was under the playhead closes up to the cut before it.
        if self.apply(trim) {
            self.seek_program(span.start);
        }
    }

    /// Moves the focused monitor's playhead to its in or out mark.
    pub(crate) fn go_to_mark(&mut self, edge: Edge) {
        let pick = |marks: Marks| match edge {
            Edge::Head => marks.mark_in,
            Edge::Tail => marks.mark_out,
        };
        match (self.focus, &mut self.source) {
            (Focus::Source, Some(source)) => {
                if let Some(at) = pick(source.marks) {
                    source.viewer.player.seek(at);
                }
            }
            (Focus::Source, None) => {}
            (Focus::Program, _) => {
                if let Some(at) = pick(self.timeline.sequence().marks) {
                    self.seek_program(at);
                }
            }
        }
    }

    /// Opens the media of the clip under the playhead in the source monitor on the same
    /// frame, marked with the clip's range: a selected clip first, then the top video.
    pub(crate) fn match_frame(&mut self) {
        let at = self.playhead();
        let sequence = self.timeline.sequence();
        let under: Vec<_> = sequence
            .tracks()
            .filter(|(_, track)| !track.muted)
            .filter_map(|(track, contents)| Some((track, contents.clip_at(at)?)))
            .filter(|(_, clip)| !clip.disabled)
            .collect();
        let clip = under
            .iter()
            .find(|(_, clip)| self.view.selection.contains(&clip.id))
            .or_else(|| {
                under
                    .iter()
                    .filter(|(track, _)| track.kind == TrackKind::Video)
                    .max_by_key(|(track, _)| track.index)
            })
            .or(under.first())
            .map(|(_, clip)| (*clip).clone());
        let Some(clip) = clip else {
            self.notice("Put the playhead over a clip to match its frame");
            return;
        };
        if sequence
            .media_item(clip.source)
            .is_some_and(|item| item.nest.is_some())
        {
            self.open_clip(clip.id);
            return;
        }
        self.open_source(clip.source);
        if let Some(source) = self
            .source
            .as_mut()
            .filter(|source| source.source == clip.source)
        {
            source.show_media(clip.source_in..clip.source_out(), clip.source_time(at));
        }
    }
}
