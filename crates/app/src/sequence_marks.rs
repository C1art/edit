use timeline::{ExtractMarked, LiftMarked, Marks, SetMarks, Ticks};

use crate::editor::Editor;
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
}
