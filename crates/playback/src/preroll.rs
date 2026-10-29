use timeline::{track_layers_at, ClipId, Sequence, Ticks, Timebase};

use crate::compose::{decodes_planes, TrackDecoders};
use crate::Resolution;

/// How long before a clip starts its first picture starts being decoded.
const PREROLL_SECONDS: i64 = 3;

/// Reads one packet toward the first picture of a clip starting on a video track soon
/// after `pts`, in a decoder no clip showing then follows, so a cut into the middle of
/// a long group of pictures plays without stalling. Returns whether there's more to do.
pub(crate) fn preroll(
    sequence: &Sequence,
    pts: Ticks,
    tracks: &mut [TrackDecoders],
    resolution: Resolution,
) -> bool {
    let lead = Timebase::new(1, 1).map_or(Ticks::ZERO, |second| {
        Ticks(PREROLL_SECONDS).rescale(second, sequence.timebase)
    });
    for (track, decoders) in sequence.video.iter().zip(tracks.iter_mut()) {
        if track.muted {
            continue;
        }
        let next = track
            .clips
            .iter()
            .filter(|clip| pts < clip.start && clip.start <= pts + lead)
            .min_by_key(|clip| clip.start);
        // A reversed clip starts from the end of its pictures and decodes backwards.
        let Some(clip) = next.filter(|clip| !clip.disabled && !clip.reverse) else {
            continue;
        };
        let Some(media) = sequence
            .media_item(clip.source)
            .filter(|media| media.nest.is_none())
        else {
            continue;
        };
        let showing: Vec<ClipId> = track_layers_at(sequence, track, pts)
            .iter()
            .map(|layer| layer.clip.id)
            .collect();
        let Some(decoder) = decoders.spare(clip.id, &showing) else {
            continue;
        };
        decoder.keep_planes(decodes_planes(clip));
        let first = clip.source_time(clip.start);
        if let Ok(false) = decoder.preroll(media, first, resolution, 1) {
            return true;
        }
    }
    false
}
