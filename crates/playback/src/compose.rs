use render::LayerLook;
use timeline::{track_layers_at, ClipId, Look, Sequence, SourceId, Ticks, Timebase, TrackRef};

use crate::track_video::TrackVideo;
use crate::{Layer, PlaybackError};

/// A cross dissolve shows two clips of one track at once.
const DECODERS_PER_TRACK: usize = 2;

/// Gaussian blur's "blurriness" reaches about two standard deviations.
const BLURRINESS_PER_SIGMA: f32 = 2.0;

/// The decoders of one video track, each following one clip so both sides of a
/// dissolve keep reading forward.
pub(crate) struct TrackDecoders {
    decoders: Vec<(Option<ClipId>, TrackVideo)>,
}

impl TrackDecoders {
    fn new(timebase: Timebase) -> Self {
        Self {
            decoders: (0..DECODERS_PER_TRACK)
                .map(|_| (None, TrackVideo::new(timebase)))
                .collect(),
        }
    }

    /// The decoder following `clip`, or else one no clip in `keep` is using.
    fn for_clip(&mut self, clip: ClipId, keep: &[ClipId]) -> &mut TrackVideo {
        let index = self
            .decoders
            .iter()
            .position(|(owner, _)| *owner == Some(clip))
            .or_else(|| {
                self.decoders
                    .iter()
                    .position(|(owner, _)| owner.is_none_or(|owner| !keep.contains(&owner)))
            })
            .unwrap_or(0);
        let (owner, decoder) = &mut self.decoders[index];
        *owner = Some(clip);
        decoder
    }
}

/// Decodes the pictures of every visible video track at `pts`, bottom layer first.
/// Media that can't be read is reported once in `warnings` and left out.
pub(crate) fn layers_at(
    sequence: &Sequence,
    pts: Ticks,
    tracks: &mut Vec<TrackDecoders>,
    warned: &mut Vec<SourceId>,
    warnings: &mut Vec<PlaybackError>,
) -> Vec<Layer> {
    tracks.resize_with(sequence.video.len(), || {
        TrackDecoders::new(sequence.timebase)
    });
    let mut layers = Vec::new();
    for ((index, track), decoders) in sequence.video.iter().enumerate().zip(tracks) {
        if track.muted {
            continue;
        }
        let planned = track_layers_at(sequence, track, pts);
        let showing: Vec<ClipId> = planned.iter().map(|layer| layer.clip.id).collect();
        for plan in planned {
            let Some(media) = sequence.media_item(plan.clip.source) else {
                continue;
            };
            let decoder = decoders.for_clip(plan.clip.id, &showing);
            match decoder.frame_at(media, plan.source_time) {
                Ok(Some(frame)) => layers.push(Layer {
                    track: TrackRef::video(index),
                    frame,
                    look: layer_look(&plan.clip.look, plan.fade),
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
    }
    layers
}

fn layer_look(look: &Look, fade: f32) -> LayerLook {
    let percent = |param: &timeline::Param| param.value() / 100.0;
    let crop = &look.crop;
    LayerLook {
        offset: look.motion.position.each_ref().map(|axis| axis.value()),
        scale: percent(&look.motion.scale),
        rotation_degrees: look.motion.rotation.value(),
        crop: [&crop.left, &crop.top, &crop.right, &crop.bottom].map(percent),
        opacity: percent(&look.opacity) * fade,
        blur: look.blurriness() / BLURRINESS_PER_SIGMA,
    }
}
