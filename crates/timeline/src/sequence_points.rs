//! Points in a sequence to jump and snap to.

use std::sync::Arc;

use crate::{Marker, Sequence, Ticks};

impl Sequence {
    /// The first marker strictly after `position`.
    pub fn marker_after(&self, position: Ticks) -> Option<&Marker> {
        self.markers.iter().find(|marker| marker.at > position)
    }

    /// The last marker strictly before `position`.
    pub fn marker_before(&self, position: Ticks) -> Option<&Marker> {
        self.markers
            .iter()
            .rev()
            .find(|marker| marker.at < position)
    }

    /// Clip starts and ends on all tracks, sorted and deduplicated, for snapping and
    /// jumping between edits.
    pub fn edit_points(&self) -> Vec<Ticks> {
        let mut points: Vec<Ticks> = self
            .tracks()
            .flat_map(|(_, track)| track.clips.iter().map(Arc::as_ref))
            .flat_map(|clip| [clip.start, clip.end()])
            .collect();
        points.sort_unstable();
        points.dedup();
        points
    }
}
