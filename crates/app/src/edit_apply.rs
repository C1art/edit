use timeline::{Edit, SetSequenceFormat, SourceId, Ticks, TimelineError};

use crate::editor::Editor;

impl Editor {
    pub(crate) fn apply(&mut self, edit: impl Edit) -> bool {
        match self.timeline.apply(edit) {
            Ok(()) => true,
            Err(TimelineError::NothingToEdit) => false,
            Err(error) => {
                self.notice(format!("Cannot edit: {error}"));
                false
            }
        }
    }

    /// Applies an edit that changes only levels, or nothing that plays at all, handing it
    /// to the program player without the seek a structural edit needs, so playback
    /// carries on uninterrupted.
    pub(crate) fn apply_levels(&mut self, edit: impl Edit) {
        if !self.apply(edit) {
            return;
        }
        self.shown_revision = self.timeline.revision();
        self.mix_program(self.timeline.sequence().clone());
    }

    /// Runs an action of several edits so that it undoes as one step.
    pub(crate) fn as_one_edit<T>(&mut self, act: impl FnOnce(&mut Self) -> T) -> T {
        let mark = self.timeline.edit_mark();
        let result = act(self);
        self.timeline.join_since(mark);
        result
    }

    /// An empty sequence takes on the format of the first video placed in it.
    pub(crate) fn match_format(&mut self, source: SourceId) {
        let sequence = self.timeline.sequence();
        // A nest shares the project's format, which its contents shouldn't decide.
        if sequence.duration() > Ticks::ZERO || self.timeline.open_nest().is_some() {
            return;
        }
        let Some(format) = sequence
            .media_item(source)
            .and_then(|item| item.played_format())
        else {
            return;
        };
        if format != sequence.format {
            self.apply(SetSequenceFormat { format });
        }
    }
}
