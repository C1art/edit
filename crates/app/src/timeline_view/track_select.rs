use timeline::{ClipId, Sequence, Ticks, TrackRef};

use super::tool::Direction;

/// The clips a track select tool picks from `at`: every clip reaching past it in
/// `direction`, on unlocked tracks, or only on `only`.
pub fn pick(
    sequence: &Sequence,
    at: Ticks,
    direction: Direction,
    only: Option<TrackRef>,
) -> Vec<ClipId> {
    let picked: Vec<ClipId> = sequence
        .tracks()
        .filter(|(track_ref, track)| !track.locked && only.is_none_or(|only| only == *track_ref))
        .flat_map(|(_, track)| &track.clips)
        .filter(|clip| match direction {
            Direction::Forward => clip.end() > at,
            Direction::Backward => clip.start < at,
        })
        .map(|clip| clip.id)
        .collect();
    sequence.with_links(&picked)
}
