use timeline::{clamp_delta, ClipId, Edge, Roll, Sequence, TrackRef, Trim, TrimMode};

use crate::editor::Editor;
use crate::keymap::Command;
use crate::timeline_view::{EditPoint, TrimType};

/// Premiere's keyboard trimming: select a cut, then move it a frame or five at a time.
impl Editor {
    /// Selects the cut nearest the playhead on every unlocked track with a clip edge
    /// there, keeping the trim type last chosen, and moves the playhead onto it.
    pub(crate) fn select_nearest_edit_point(&mut self) {
        let sequence = self.timeline.sequence();
        let playhead = self.playhead();
        let nearest = sequence
            .edit_points()
            .into_iter()
            .min_by_key(|&point| (point - playhead).0.abs());
        let Some(at) = nearest else {
            self.notice("There are no edit points to select");
            return;
        };
        let tracks: Vec<TrackRef> = sequence
            .tracks()
            .filter(|(_, track)| {
                !track.locked
                    && track
                        .clips
                        .iter()
                        .any(|clip| clip.start == at || clip.end() == at)
            })
            .map(|(track, _)| track)
            .collect();
        let trim = self
            .view
            .edit_point
            .as_ref()
            .map_or(TrimType::default(), |point| point.trim);
        self.view.edit_point = Some(EditPoint { at, tracks, trim });
        self.seek_program(at);
    }

    pub(crate) fn toggle_trim_type(&mut self) {
        if let Some(point) = &mut self.view.edit_point {
            point.trim = point.trim.next();
        }
    }

    /// Moves the selected edit point `frames` later, or earlier when negative, as far as
    /// the media allows.
    pub(crate) fn trim_edit_point(&mut self, frames: i64) {
        let sequence = self.timeline.sequence();
        let Some(point) = self
            .view
            .edit_point
            .clone()
            .filter(|point| point.is_on(sequence))
        else {
            let hint = self.keymap.tip(
                "Select an edit point to trim first",
                Command::SelectNearestEditPoint,
            );
            self.notice(hint);
            return;
        };
        let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
        let target = rate.frame_start(rate.frame_at(point.at, timebase) + frames, timebase);
        let desired = target - point.at;
        let applied = match point.trim {
            TrimType::Roll => {
                let make = |delta| Roll {
                    at: point.at,
                    tracks: point.tracks.clone(),
                    delta,
                };
                let delta = clamp_delta(sequence, desired, make);
                self.apply(make(delta)).then_some(point.at + delta)
            }
            TrimType::RippleOut | TrimType::RippleIn => {
                let edge = match point.trim {
                    TrimType::RippleIn => Edge::Head,
                    _ => Edge::Tail,
                };
                let make = |delta| Trim {
                    clips: clips_at(sequence, &point, edge),
                    edge,
                    delta,
                    mode: TrimMode::Ripple,
                };
                let delta = clamp_delta(sequence, desired, make);
                // A ripple keeps the incoming clip's start where it was.
                let moved = match edge {
                    Edge::Head => point.at,
                    Edge::Tail => point.at + delta,
                };
                self.apply(make(delta)).then_some(moved)
            }
        };
        if let Some(at) = applied {
            if let Some(point) = &mut self.view.edit_point {
                point.at = at;
            }
            self.seek_program(at);
        }
    }
}

/// The clips on the point's tracks whose `edge` lies on it.
fn clips_at(sequence: &Sequence, point: &EditPoint, edge: Edge) -> Vec<ClipId> {
    point
        .tracks
        .iter()
        .filter_map(|&track| sequence.track(track))
        .flat_map(|track| &track.clips)
        .filter(|clip| match edge {
            Edge::Head => clip.start == point.at,
            Edge::Tail => clip.end() == point.at,
        })
        .map(|clip| clip.id)
        .collect()
}
