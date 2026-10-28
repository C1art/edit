use render::{ColorAdjust, LayerLook};
use timeline::{
    track_layers_at, ClipId, ColorCorrection, Generator, Look, Param, Reveal, Sequence, SourceId,
    Ticks, Track, TrackRef,
};

use crate::clip_decoders::ClipDecoders;
use crate::pixel_effects::pixel_effects;
use crate::track_video::TrackVideo;
use crate::{Layer, LayerPicture, PlaybackError};

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
    let mut reported = Reported { warned, warnings };
    video_layers(sequence, &sequence.video, pts, tracks, &mut reported)
}

/// Media already reported as unreadable, and the reports to send.
struct Reported<'a> {
    warned: &'a mut Vec<SourceId>,
    warnings: &'a mut Vec<PlaybackError>,
}

/// The layers of `video` tracks, which are the sequence's own or a nest's; nests
/// recurse with the decoders of the clip they're placed in.
fn video_layers(
    sequence: &Sequence,
    video: &[Track],
    pts: Ticks,
    tracks: &mut Vec<TrackDecoders>,
    reported: &mut Reported<'_>,
) -> Vec<Layer> {
    tracks.resize_with(video.len(), || {
        ClipDecoders::new(|| TrackVideo::new(sequence.timebase))
    });
    let size = [sequence.format.width, sequence.format.height];
    let canvas = size.map(|side| side as f32);
    let mut layers = Vec::new();
    for ((index, track), decoders) in video.iter().enumerate().zip(tracks) {
        if track.muted {
            continue;
        }
        let planned = track_layers_at(sequence, track, pts);
        let showing: Vec<ClipId> = planned.iter().map(|layer| layer.clip.id).collect();
        for plan in planned.into_iter().filter(|plan| !plan.clip.disabled) {
            let Some(media) = sequence.media_item(plan.clip.source) else {
                continue;
            };
            let look = layer_look(
                &plan.clip.look,
                plan.clip.source_time(pts),
                plan.reveal,
                canvas,
            );
            if media.generator == Some(Generator::AdjustmentLayer) {
                adjust(&mut layers, TrackRef::video(index), look);
                continue;
            }
            let decoder = decoders.for_clip(plan.clip.id, &showing);
            let picture = match &media.nest {
                Some(nest) => {
                    let (time, nested) = (plan.source_time, &mut decoder.nested);
                    let inner = video_layers(sequence, &nest.video, time, nested, reported);
                    (!inner.is_empty()).then_some(LayerPicture::Nest(inner))
                }
                None => match decoder.frame_at(media, plan.source_time, size) {
                    Ok(frame) => frame.map(LayerPicture::Frame),
                    Err(error) => {
                        decoder.reset();
                        if !reported.warned.contains(&media.id) {
                            reported.warned.push(media.id);
                            reported.warnings.push(PlaybackError::Media(error));
                        }
                        None
                    }
                },
            };
            if let Some(picture) = picture {
                layers.push(Layer {
                    track: TrackRef::video(index),
                    picture,
                    look,
                });
            }
        }
    }
    layers
}

/// Applies an adjustment layer's look to the picture of `layers` as one. Below full
/// opacity the adjusted picture blends over the untouched one, as its opacity is meant to.
fn adjust(layers: &mut Vec<Layer>, track: TrackRef, look: LayerLook) {
    if layers.is_empty() {
        return;
    }
    let below = if look.opacity < 1.0 {
        layers.clone()
    } else {
        std::mem::take(layers)
    };
    layers.push(Layer {
        track,
        picture: LayerPicture::Nest(below),
        look,
    });
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use media::VideoFrame;
    use timeline::Param;

    use super::*;

    fn layer(track: usize) -> Layer {
        let frame = VideoFrame {
            pts: Ticks::ZERO,
            width: 1,
            height: 1,
            pixels: vec![0; 4],
        };
        Layer {
            track: TrackRef::video(track),
            picture: LayerPicture::Frame(Arc::new(frame)),
            look: layer_look(&Look::default(), Ticks::ZERO, Reveal::FULL, [1.0, 1.0]),
        }
    }

    #[test]
    fn adjustment_groups_the_picture_below_and_blends_over_it_when_faded() {
        let mut look = Look::default();
        let mut layers = vec![layer(0), layer(1)];
        let full = layer_look(&look, Ticks::ZERO, Reveal::FULL, [1.0, 1.0]);
        adjust(&mut layers, TrackRef::video(2), full);
        assert!(
            matches!(&layers[..], [Layer { picture: LayerPicture::Nest(below), .. }] if below.len() == 2)
        );

        look.opacity = Param::from_value(50.0);
        let faded = layer_look(&look, Ticks::ZERO, Reveal::FULL, [1.0, 1.0]);
        let mut layers = vec![layer(0)];
        adjust(&mut layers, TrackRef::video(1), faded);
        assert!(matches!(
            &layers[..],
            [
                Layer {
                    picture: LayerPicture::Frame(_),
                    ..
                },
                Layer {
                    picture: LayerPicture::Nest(_),
                    ..
                }
            ]
        ));
    }
}
