//! Sequence › Go to Gap: the playhead to where the next or previous empty stretch
//! begins, across the whole sequence or on the targeted tracks.

use std::sync::Arc;

use timeline::{unused_time, Ticks, TrackRef};

use crate::editor::Editor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GapScope {
    /// Time no track uses.
    Sequence,
    /// Time any one targeted track leaves empty.
    Track,
}

impl Editor {
    pub(crate) fn go_to_gap(&mut self, scope: GapScope, forward: bool) {
        let sequence = self.timeline.sequence();
        let starts: Vec<Ticks> = match scope {
            GapScope::Sequence => unused_time(
                sequence
                    .tracks()
                    .flat_map(|(_, track)| track.clips.iter().map(Arc::as_ref)),
            )
            .into_iter()
            .map(|gap| gap.start)
            .collect(),
            GapScope::Track => {
                let targets = self.view.targets;
                let tracks: Vec<_> = [
                    targets.video.map(TrackRef::video),
                    targets.audio.map(TrackRef::audio),
                ]
                .into_iter()
                .flatten()
                .filter_map(|track| sequence.track(track))
                .collect();
                if tracks.is_empty() {
                    self.notice("Target a track first (click a track name)");
                    return;
                }
                let mut starts: Vec<Ticks> = tracks
                    .into_iter()
                    .flat_map(|track| unused_time(track.clips.iter().map(Arc::as_ref)))
                    .map(|gap| gap.start)
                    .collect();
                starts.sort_unstable();
                starts
            }
        };
        let at = self.playhead();
        let target = if forward {
            starts.into_iter().find(|&start| start > at)
        } else {
            starts.into_iter().rev().find(|&start| start < at)
        };
        match target {
            Some(start) => self.seek_program(start),
            None => self.notice(if forward {
                "No gap after the playhead"
            } else {
                "No gap before the playhead"
            }),
        }
    }
}
