use timeline::{AddTrack, Lift, Razor, RazorTarget, RippleDelete, TrackKind};

use crate::editor::Editor;

/// Editor operations shared by the menus and keyboard shortcuts.
impl Editor {
    pub(crate) fn undo(&mut self) {
        self.view.cancel_drag();
        self.timeline.undo();
    }

    pub(crate) fn redo(&mut self) {
        self.view.cancel_drag();
        if let Err(error) = self.timeline.redo() {
            self.notice(format!("Cannot redo: {error}"));
        }
    }

    /// Cuts every unlocked track at the playhead.
    pub(crate) fn add_edit(&mut self) {
        let tracks = self
            .timeline
            .sequence()
            .tracks()
            .filter(|(_, track)| !track.locked)
            .map(|(track, _)| track)
            .collect();
        self.apply(Razor {
            at: self.playhead(),
            target: RazorTarget::Tracks(tracks),
        });
    }

    pub(crate) fn lift(&mut self) {
        let clips = std::mem::take(&mut self.view.selection);
        if !clips.is_empty() {
            self.apply(Lift { clips });
        }
    }

    pub(crate) fn ripple_delete(&mut self) {
        let clips = std::mem::take(&mut self.view.selection);
        if !clips.is_empty() {
            self.apply(RippleDelete { clips });
        }
    }

    pub(crate) fn add_track(&mut self, kind: TrackKind) {
        self.apply(AddTrack { kind });
    }
}
