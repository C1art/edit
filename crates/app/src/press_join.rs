use crate::editor::Editor;

impl Editor {
    /// Runs a panel's UI so that the edits it makes over one held mouse press, as while
    /// dragging in a color picker or on a number, undo as one step. A press is cut short
    /// by any other change to the timeline, such as an undo or a finished import.
    pub(crate) fn joining_press<T>(&mut self, held: bool, act: impl FnOnce(&mut Self) -> T) -> T {
        let revision = self.timeline.revision();
        let mark = match self.press_mark {
            Some((mark, seen)) if held && seen == revision => mark,
            _ => self.timeline.edit_mark(),
        };
        let result = act(self);
        self.press_mark = held.then(|| {
            self.timeline.join_since(mark);
            (mark, self.timeline.revision())
        });
        result
    }
}
