use render::{ColorAdjust, LayerLook};
use timeline::{
    track_layers_at, ClipId, ColorCorrection, Look, Param, Reveal, Sequence, SourceId, Ticks,
    TrackRef,
};

use crate::clip_decoders::ClipDecoders;
use crate::pixel_effects::pixel_effects;
use crate::track_video::TrackVideo;
use crate::{Layer, PlaybackError};

/// Gaussian blur's "blurriness" reaches about two standard deviations.
const BLURRINESS_PER_SIGMA: f32 = 2.0;

/// The decoders of one video track.
pub(crate) type TrackDecoders = ClipDecoders<TrackVideo>;

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
        ClipDecoders::new(|| TrackVideo::new(sequence.timebase))
    });
    let canvas = [sequence.format.width, sequence.format.height].map(|side| side as f32);
    let mut layers = Vec::new();
    for ((index, track), decoders) in sequence.video.iter().enumerate().zip(tracks) {
        if track.muted {
            continue;
        }
        let planned = track_layers_at(sequence, track, pts);
        let showing: Vec<ClipId> = planned.iter().map(|layer| layer.clip.id).collect();
        for plan in planned.into_iter().filter(|plan| !plan.clip.disabled) {
            let Some(media) = sequence.media_item(plan.clip.source) else {
                continue;
            };
            let decoder = decoders.for_clip(plan.clip.id, &showing);
            match decoder.frame_at(media, plan.source_time) {
                Ok(Some(frame)) => layers.push(Layer {
                    track: TrackRef::video(index),
                    frame,
                    look: layer_look(
                        &plan.clip.look,
                        plan.clip.source_time(pts),
                        plan.reveal,
                        canvas,
                    ),
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

/// The look at source time `time`, where its keyframes are, as a transition reveals it
/// on a canvas of `canvas` pixels.
fn layer_look(look: &Look, time: Ticks, reveal: Reveal, canvas: [f32; 2]) -> LayerLook {
    let value = |param: &Param| param.value_at(time);
    let percent = |param: &Param| param.value_at(time) / 100.0;
    let crop = &look.crop;
    let position = look.motion.position.each_ref().map(value);
    // Zooming about the canvas center scales the picture's offset from it too.
    let offset =
        [0, 1].map(|axis| (position[axis] + reveal.shift[axis] * canvas[axis]) * reveal.zoom);
    LayerLook {
        offset,
        scale: percent(&look.motion.scale) * reveal.zoom,
        rotation_degrees: value(&look.motion.rotation),
        crop: [&crop.left, &crop.top, &crop.right, &crop.bottom].map(percent),
        opacity: percent(&look.opacity) * reveal.fade,
        blur: look.blurriness_at(time) / BLURRINESS_PER_SIGMA,
        color: color_adjust(&look.color, time),
        mask: reveal.mask,
        wash: reveal.wash,
        effects: pixel_effects(&look.effects, time),
    }
}

fn color_adjust(color: &ColorCorrection, time: Ticks) -> ColorAdjust {
    let unit = |param: &Param| param.value_at(time) / 100.0;
    ColorAdjust {
        exposure: color.exposure.value_at(time),
        contrast: unit(&color.contrast),
        highlights: unit(&color.highlights),
        shadows: unit(&color.shadows),
        temperature: unit(&color.temperature),
        tint: unit(&color.tint),
        saturation: unit(&color.saturation),
    }
}
