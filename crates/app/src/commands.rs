use std::collections::HashSet;

use timeline::{
    AddBlurredBackground, AddFrameHold, AddTrack, ClipId, Clipboard, CloseGap, CloseGaps,
    DeleteEmptyTracks, FrameFit, Gain, HoldFrame, InsertFrameHold, JoinThroughEdits, Lift,
    LinkedSelection, MakeSubsequence, NudgeClipGain, PasteClips, PlaceMode, Razor, RazorTarget,
    RippleDelete, Sequence, SetClipsEnabled, SetFrameHold, SetGrouped, SetLinked,
    SetLinkedSelection, SetToFrameSize, Ticks, Timebase, TrackKind, TrackRef,
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

    /// Cuts the selected clips under the playhead there, or with none of those, the
    /// targeted tracks.
    pub(crate) fn add_edit(&mut self) {
        let at = self.playhead();
        let sequence = self.timeline.sequence();
        let selected: Vec<_> = self
            .view
            .selection
            .iter()
            .copied()
            .filter(|&id| {
                sequence
                    .clip(id)
                    .is_some_and(|(_, clip)| clip.start < at && at < clip.end())
            })
            .collect();
        let target = if selected.is_empty() {
            let targets = &self.view.targets;
            let targeted = [
                targets.video.map(TrackRef::video),
                targets.audio.map(TrackRef::audio),
            ];
            let tracks: Vec<_> = targeted
                .into_iter()
                .flatten()
                .filter(|&track| sequence.track(track).is_some_and(|track| !track.locked))
                .collect();
            if tracks.is_empty() {
                self.notice("Target a track to add an edit to");
                return;
            }
            RazorTarget::Tracks(tracks)
        } else {
            RazorTarget::Clips(selected)
        };
        self.apply(Razor { at, target });
    }

    /// Cuts every unlocked track at the playhead.
    pub(crate) fn add_edit_to_all_tracks(&mut self) {
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

    /// Clears the selected clips, or with none selected closes the selected gap, which
    /// is all clearing a gap can mean.
    pub(crate) fn lift(&mut self) {
        let clips = std::mem::take(&mut self.view.selection);
        if clips.is_empty() {
            self.close_gap();
        } else {
            self.apply(Lift { clips });
        }
    }

    pub(crate) fn ripple_delete(&mut self) {
        let clips = std::mem::take(&mut self.view.selection);
        if clips.is_empty() {
            self.close_gap();
        } else {
            self.apply(RippleDelete { clips });
        }
    }

    /// A new sequence of the selected clips, or of the marked span with none selected,
    /// added to the project beside this one.
    pub(crate) fn make_subsequence(&mut self) {
        let name = format!("{} Sub", self.shown_sequence_name());
        let clips = self.view.selection.clone();
        if !self.apply(MakeSubsequence { clips, name }) {
            self.notice("Select clips or mark In and Out to make a subsequence");
        }
    }

    pub(crate) fn close_all_gaps(&mut self) {
        self.apply(CloseGaps);
    }

    fn close_gap(&mut self) {
        if let Some((track, at)) = self.view.gap.take() {
            self.apply(CloseGap { track, at });
        }
    }

    pub(crate) fn copy(&mut self) {
        let sequence = self.timeline.sequence();
        let Some(clipboard) = Clipboard::copy(sequence, &self.view.selection) else {
            return;
        };
        // Paste only arrives as an event while the system clipboard holds text.
        let timecode = sequence.timecode(clipboard.duration());
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
        let before: HashSet<ClipId> = clip_ids(self.timeline.sequence()).collect();
        if self.apply(paste) {
            self.view.selection = clip_ids(self.timeline.sequence())
                .filter(|id| !before.contains(id))
                .collect();
            self.seek_program(end);
        }
    }

    pub(crate) fn add_track(&mut self, kind: TrackKind) {
        self.apply(AddTrack { kind });
    }

    pub(crate) fn delete_empty_tracks(&mut self, kinds: &[TrackKind]) {
        let kinds = kinds.to_vec();
        if !self.apply(DeleteEmptyTracks { kinds }) {
            self.notice("No empty, unlocked tracks to delete");
        }
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

    /// Raises or lowers the selected clips' audio by `db` from where each one is.
    pub(crate) fn nudge_clip_gain(&mut self, db: f32) {
        let clips = self.view.selection.clone();
        if self.apply(NudgeClipGain {
            clips,
            by: Gain::from_db(db),
        }) {
            return;
        }
        let sequence = self.timeline.sequence();
        let selected_audio = sequence
            .with_links(&self.view.selection)
            .into_iter()
            .filter_map(|id| sequence.clip(id))
            .any(|(track, _)| track.kind == TrackKind::Audio);
        if !selected_audio {
            self.notice("Select a clip with audio first");
        }
    }

    /// Freezes the selected video clips, or else the unlocked ones under the playhead,
    /// on the frame at the playhead.
    pub(crate) fn add_frame_hold(&mut self) {
        let at = self.playhead();
        let clips = self.held_clips(at);
        if !self.apply(AddFrameHold { clips, at }) {
            self.notice("Put the playhead over a video clip to hold its frame");
        }
    }

    /// Like `add_frame_hold`, but puts two seconds of the frame in the clip and pushes
    /// the rest of it later.
    pub(crate) fn insert_frame_hold(&mut self) {
        let at = self.playhead();
        let clips = self.held_clips(at);
        let timebase = self.timeline.sequence().timebase;
        let Some(length) = Timebase::new(2, 1).map(|two| Ticks(1).rescale(two, timebase)) else {
            return;
        };
        if !self.apply(InsertFrameHold { clips, at, length }) {
            self.notice("Put the playhead over a video clip to hold its frame");
        }
    }

    /// Holds the selected video clips, or else those under the playhead, on one of their
    /// frames for their whole length, or with `None` lets them play again.
    pub(crate) fn set_frame_hold(&mut self, frame: Option<HoldFrame>) {
        let clips = self.held_clips(self.playhead());
        if !self.apply(SetFrameHold { clips, frame }) {
            self.notice(match frame {
                Some(_) => "Select video clips, or put the playhead over one, to hold",
                None => "None of those clips holds a frame",
            });
        }
    }

    fn held_clips(&self, at: Ticks) -> Vec<ClipId> {
        if !self.view.selection.is_empty() {
            return self.view.selection.clone();
        }
        self.timeline
            .sequence()
            .tracks()
            .filter(|(track, contents)| track.kind == TrackKind::Video && !contents.locked)
            .filter_map(|(_, contents)| contents.clip_at(at))
            .map(|clip| clip.id)
            .collect()
    }

    /// Disables the selected clips, or enables them when all are already disabled.
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

    /// Whether every selected clip is linked; `None` when none is selected.
    pub(crate) fn selection_linked(&self) -> Option<bool> {
        let sequence = self.timeline.sequence();
        let mut selected = self
            .view
            .selection
            .iter()
            .filter_map(|&id| sequence.clip(id))
            .peekable();
        selected.peek()?;
        Some(selected.all(|(_, clip)| clip.link.is_some()))
    }

    /// Unlinks the selected clips when they are all linked, or else links them.
    pub(crate) fn toggle_link(&mut self) {
        let Some(linked) = self.selection_linked() else {
            self.notice("Select clips to link or unlink");
            return;
        };
        let edit = SetLinked {
            clips: self.view.selection.clone(),
            linked: !linked,
        };
        if linked {
            self.apply(edit);
        } else if !self.apply(edit) {
            self.notice("Select two or more clips to link");
        }
    }

    pub(crate) fn toggle_linked_selection(&mut self) {
        let next = match self.timeline.sequence().linked_selection {
            LinkedSelection::Together => LinkedSelection::Apart,
            LinkedSelection::Apart => LinkedSelection::Together,
        };
        self.apply(SetLinkedSelection(next));
    }

    pub(crate) fn set_grouped(&mut self, grouped: bool) {
        let edit = SetGrouped {
            clips: self.view.selection.clone(),
            grouped,
        };
        if self.apply(edit) {
            self.view.selection = self.timeline.sequence().with_groups(&self.view.selection);
        } else if grouped {
            self.notice("Select two or more clips to group");
        } else {
            self.notice("Select a grouped clip to ungroup");
        }
    }

    pub(crate) fn join_through_edits(&mut self) {
        let edit = JoinThroughEdits {
            clips: self.view.selection.clone(),
        };
        if !self.apply(edit) {
            self.notice("No through edits at the selected clips");
        }
    }

    pub(crate) fn add_blurred_background(&mut self) {
        let edit = AddBlurredBackground {
            clips: self.view.selection.clone(),
        };
        if !self.apply(edit) {
            self.notice("Select video clips to put a blurred background under");
        }
    }

    pub(crate) fn set_to_frame_size(&mut self, fit: FrameFit) {
        if self.view.selection.is_empty() {
            self.notice("Select video clips to size to the frame");
            return;
        }
        let edit = SetToFrameSize {
            clips: self.view.selection.clone(),
            fit,
        };
        if !self.apply(edit) {
            self.notice("The selected video clips already match the frame");
        }
    }
}

fn clip_ids(sequence: &Sequence) -> impl Iterator<Item = ClipId> + '_ {
    sequence
        .tracks()
        .flat_map(|(_, track)| track.clips.iter().map(|clip| clip.id))
}
