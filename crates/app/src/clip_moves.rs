use timeline::{clamp_delta, Edge, MoveClips, MoveMode, Ticks, TrackKind, Trim, TrimMode};

use crate::editor::Editor;

/// Keyboard moves of the selected clips, as Premiere's nudge and extend commands.
impl Editor {
    /// Moves the selection by whole frames, keeping the first selected clip on the frame
    /// grid, as far as it can go without leaving the sequence.
    pub(crate) fn nudge(&mut self, frames: i64) {
        let sequence = self.timeline.sequence();
        let clips = self.view.selection.clone();
        let Some((_, first)) = clips.first().and_then(|&id| sequence.clip(id)) else {
            return;
        };
        let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
        let target = rate.frame_start(rate.frame_at(first.start, timebase) + frames, timebase);
        let make = |delta| MoveClips {
            clips: clips.clone(),
            delta,
            video_tracks: 0,
            audio_tracks: 0,
            mode: MoveMode::Overwrite,
        };
        let delta = clamp_delta(sequence, target - first.start, make);
        if delta != Ticks::ZERO {
            self.apply(make(delta));
        }
    }

    /// Moves the selection to the track above, or below when `up` is false; video tracks
    /// count up from V1 and audio tracks down from A1.
    pub(crate) fn nudge_track(&mut self, up: bool) {
        if self.view.selection.is_empty() {
            return;
        }
        let step = if up { 1 } else { -1 };
        self.apply(MoveClips {
            clips: self.view.selection.clone(),
            delta: Ticks::ZERO,
            video_tracks: step,
            audio_tracks: -step,
            mode: MoveMode::Overwrite,
        });
    }

    /// Trims the first selected clip's nearer end to the playhead, as far as its media
    /// and neighbours allow.
    pub(crate) fn extend_edit(&mut self) {
        let sequence = self.timeline.sequence();
        let playhead = self.playhead();
        let Some((_, clip)) = self
            .view
            .selection
            .first()
            .and_then(|&id| sequence.clip(id))
        else {
            self.notice("Select a clip to extend its edit to the playhead");
            return;
        };
        let (to_head, to_tail) = (playhead - clip.start, playhead - clip.end());
        let (edge, desired) = if to_head.0.abs() <= to_tail.0.abs() {
            (Edge::Head, to_head)
        } else {
            (Edge::Tail, to_tail)
        };
        let id = clip.id;
        let make = |delta| Trim {
            clips: vec![id],
            edge,
            delta,
            mode: TrimMode::Normal,
        };
        let delta = clamp_delta(sequence, desired, make);
        if delta != Ticks::ZERO {
            self.apply(make(delta));
        }
    }

    /// Selects what's under the playhead on the targeted tracks, or on every track when
    /// none is targeted.
    pub(crate) fn select_at_playhead(&mut self) {
        let sequence = self.timeline.sequence();
        let playhead = self.playhead();
        let targets = self.view.targets;
        let any_target = targets.video.is_some() || targets.audio.is_some();
        let targeted = |kind, index| match kind {
            TrackKind::Video => targets.video == Some(index),
            TrackKind::Audio => targets.audio == Some(index),
        };
        self.view.selection = sequence
            .tracks()
            .filter(|(track, _)| !any_target || targeted(track.kind, track.index))
            .filter_map(|(_, track)| track.clip_at(playhead))
            .map(|clip| clip.id)
            .collect();
    }

    pub(crate) fn select_all(&mut self) {
        self.view.selection = self
            .timeline
            .sequence()
            .tracks()
            .flat_map(|(_, track)| track.clips.iter().map(|clip| clip.id))
            .collect();
    }

    pub(crate) fn deselect_all(&mut self) {
        self.view.selection.clear();
        self.view.edit_point = None;
        self.view.gap = None;
    }
}
