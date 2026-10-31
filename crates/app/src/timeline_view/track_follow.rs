use timeline::TrackRef;

use super::TimelineView;
use crate::track_follow::TrackMoves;

impl TimelineView {
    pub(crate) fn follow_tracks(&mut self, moves: &TrackMoves) {
        self.heights.follow_tracks(moves);
        let shown = std::mem::take(&mut self.automation_shown).into_iter();
        self.automation_shown = shown.filter_map(|track| moves.of(track)).collect();
        let [video, audio] = [
            self.targets.video.map(TrackRef::video),
            self.targets.audio.map(TrackRef::audio),
        ]
        .map(|target| target.and_then(|track| moves.or_in_place(track)));
        self.targets.video = video.map(|track| track.index);
        self.targets.audio = audio.map(|track| track.index);
        self.gap = self
            .gap
            .and_then(|(track, at)| Some((moves.of(track)?, at)));
        self.edit_point = self.edit_point.take().and_then(|mut point| {
            point
                .tracks
                .retain_mut(|track| moves.of(*track).map(|moved| *track = moved).is_some());
            (!point.tracks.is_empty()).then_some(point)
        });
        self.renaming = None;
        self.context_empty = None;
    }
}
