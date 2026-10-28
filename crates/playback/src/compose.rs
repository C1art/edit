use timeline::{Sequence, SourceId, Ticks, TrackRef};

use crate::track_video::TrackVideo;
use crate::{Layer, PlaybackError};

/// Decodes the picture of every visible video track at `pts`, bottom layer first.
/// Media that can't be read is reported once in `warnings` and left out.
pub(crate) fn layers_at(
    sequence: &Sequence,
    pts: Ticks,
    tracks: &mut Vec<TrackVideo>,
    warned: &mut Vec<SourceId>,
    warnings: &mut Vec<PlaybackError>,
) -> Vec<Layer> {
    tracks.resize_with(sequence.video.len(), || TrackVideo::new(sequence.timebase));
    let mut layers = Vec::new();
    for ((index, track), decoder) in sequence.video.iter().enumerate().zip(tracks) {
        if track.muted {
            continue;
        }
        let Some(clip) = track.clip_at(pts) else {
            continue;
        };
        let Some(media) = sequence.media_item(clip.source) else {
            continue;
        };
        match decoder.frame_at(media, clip.source_time(pts)) {
            Ok(Some(frame)) => layers.push(Layer {
                track: TrackRef::video(index),
                frame,
            }),
            Ok(None) => {}
            Err(error) => {
                decoder.reset();
                if !warned.contains(&media.id) {
                    warned.push(media.id);
                    warnings.push(PlaybackError::Media(error));
                }
            }
        }
    }
    layers
}
