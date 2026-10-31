use timeline::{TrackKind, TrackRef};

/// The tracks that Insert and Overwrite place source video and audio on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Targets {
    pub video: Option<usize>,
    pub audio: Option<usize>,
}

impl Default for Targets {
    fn default() -> Self {
        Self {
            video: Some(0),
            audio: Some(0),
        }
    }
}

impl Targets {
    pub(super) fn contains(&self, track: TrackRef) -> bool {
        let target = match track.kind {
            TrackKind::Video => self.video,
            TrackKind::Audio => self.audio,
        };
        target == Some(track.index)
    }

    pub(super) fn toggle(&mut self, track: TrackRef) {
        let target = match track.kind {
            TrackKind::Video => &mut self.video,
            TrackKind::Audio => &mut self.audio,
        };
        *target = if *target == Some(track.index) {
            None
        } else {
            Some(track.index)
        };
    }
}
