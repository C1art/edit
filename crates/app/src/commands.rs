use timeline::{
    AddFrameHold, AddTrack, Clipboard, Lift, PasteClips, PlaceMode, Razor, RazorTarget,
    RippleDelete, SetClipsEnabled, Timecode, TrackKind,
};

use crate::editor::Editor;
use crate::gain_dialog::GainDialog;

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

    pub(crate) fn go_to_history(&mut self, step: usize) {
        self.view.cancel_drag();
        if let Err(error) = self.timeline.go_to(step) {
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

    pub(crate) fn copy(&mut self) {
        let sequence = self.timeline.sequence();
        let Some(clipboard) = Clipboard::copy(sequence, &self.view.selection) else {
            return;
        };
        // Paste only arrives as an event while the system clipboard holds text.
        let timecode = Timecode::new(
            clipboard.duration(),
            sequence.timebase,
            sequence.format.frame_rate,
        );
        self.ctx
            .copy_text(format!("{} clips, {timecode}", clipboard.clip_count()));
        self.clipboard = Some(clipboard);
    }

    pub(crate) fn cut(&mut self) {
        self.copy();
        self.lift();
    }

    /// Pastes at the playhead, onto the targeted tracks where they are set, and parks
    /// the playhead after the pasted clips.
    pub(crate) fn paste(&mut self, mode: PlaceMode) {
        let Some(clipboard) = self.clipboard.clone() else {
            return;
        };
        let offset = |kind, target: Option<usize>| match (clipboard.first_track(kind), target) {
            (Some(first), Some(target)) => target as isize - first as isize,
            _ => 0,
        };
        let at = self.playhead();
        let end = at + clipboard.duration();
        let paste = PasteClips {
            video_offset: offset(TrackKind::Video, self.view.targets.video),
            audio_offset: offset(TrackKind::Audio, self.view.targets.audio),
            clipboard,
            at,
            mode,
        };
        if self.apply(paste) {
            self.seek_program(end);
        }
    }

    pub(crate) fn add_track(&mut self, kind: TrackKind) {
        self.apply(AddTrack { kind });
    }
}

impl Editor {
    /// Opens the gain dialog for the selected clips, starting from the first audio
    /// clip's gain.
    pub(crate) fn prompt_clip_gain(&mut self) {
        let sequence = self.timeline.sequence();
        let audio_gain = sequence
            .with_links(&self.view.selection)
            .into_iter()
            .filter_map(|id| sequence.clip(id))
            .find(|(track, _)| track.kind == TrackKind::Audio)
            .map(|(_, clip)| clip.gain);
        match audio_gain {
            Some(gain) => self.gain_dialog = Some(GainDialog::new(gain.db())),
            None => self.notice("Select a clip with audio first"),
        }
    }

    /// Disables the selected clips, or enables them when all are already disabled.
    /// Freezes the selected video clips, or else the unlocked ones under the playhead,
    /// on the frame at the playhead.
    pub(crate) fn add_frame_hold(&mut self) {
        let at = self.playhead();
        let mut clips = self.view.selection.clone();
        if clips.is_empty() {
            clips = self
                .timeline
                .sequence()
                .tracks()
                .filter(|(track, contents)| track.kind == TrackKind::Video && !contents.locked)
                .filter_map(|(_, contents)| contents.clip_at(at))
                .map(|clip| clip.id)
                .collect();
        }
        if !self.apply(AddFrameHold { clips, at }) {
            self.notice("Put the playhead over a video clip to hold its frame");
        }
    }

    pub(crate) fn toggle_enabled(&mut self) {
        let sequence = self.timeline.sequence();
        let selected: Vec<_> = self
            .view
            .selection
            .iter()
            .filter_map(|&id| sequence.clip(id))
            .collect();
        if selected.is_empty() {
            self.notice("Select clips to enable or disable");
            return;
        }
        let enabled = selected.iter().all(|(_, clip)| clip.disabled);
        self.apply(SetClipsEnabled {
            clips: self.view.selection.clone(),
            enabled,
        });
    }
}
