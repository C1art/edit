use std::sync::Arc;

use render::{
    ColorAdjust, Grade, LayerLook, LayerMask, LayerWipe, Outline, PixelEffects, SecondaryAdjust,
    Warp, MAX_MASKS, MAX_OUTLINE_POINTS,
};
use timeline::{
    track_layers_at, BlendMode, ClipId, ColorCorrection, Generator, HslSecondary, Look, Mask,
    MaskShape, MatteChannel, MediaItem, Param, PlannedLayer, Reveal, Sequence, SourceId, Ticks,
    TimeInterpolation, Timebase, Track, TrackRef, VideoEffect,
};

use crate::clip_decoders::ClipDecoders;
use crate::generated::{caption_frame, scrolled};
use crate::grades::grade_for;
use crate::pixel_effects::{linear, pixel_effects, transform_opacity};
use crate::time_effects::{posterized, with_timecode};
use crate::track_video::TrackVideo;
use crate::{Layer, LayerPicture, PlaybackError, Resolution};

/// Gaussian blur's "blurriness" reaches about two standard deviations.
const BLURRINESS_PER_SIGMA: f32 = 2.0;

/// The decoders of one video track.
pub(crate) type TrackDecoders = ClipDecoders<TrackVideo>;

/// Decodes the pictures of every visible video track at `pts`, bottom layer first,
/// with the caption showing then on top. Media that can't be read is reported once in
/// `warnings` and left out.
pub(crate) fn layers_at(
    sequence: &Sequence,
    pts: Ticks,
    tracks: &mut Vec<TrackDecoders>,
    warned: &mut Vec<SourceId>,
    warnings: &mut Vec<PlaybackError>,
    resolution: Resolution,
) -> Vec<Layer> {
    let mut decoding = Decoding {
        warned,
        warnings,
        resolution,
    };
    let mut layers = video_layers(sequence, &sequence.video, pts, tracks, &mut decoding);
    let captions = &sequence.captions;
    if let Some(caption) = captions.at(pts).filter(|_| !captions.hidden) {
        let format = sequence.format;
        let frame = caption_frame(&caption.text, captions.style, format.width, format.height);
        layers.push(Layer {
            // Above every track, as a caption track sits in Premiere.
            track: TrackRef::video(sequence.video.len()),
            picture: LayerPicture::Frame(frame),
            look: LayerLook::IDENTITY,
            grade: None,
        });
    }
    layers
}

/// Media already reported as unreadable, the reports to send, and how finely to decode.
struct Decoding<'a> {
    warned: &'a mut Vec<SourceId>,
    warnings: &'a mut Vec<PlaybackError>,
    resolution: Resolution,
}

/// The layers of `video` tracks, which are the sequence's own or a nest's; nests
/// recurse with the decoders of the clip they're placed in.
fn video_layers(
    sequence: &Sequence,
    video: &[Track],
    pts: Ticks,
    tracks: &mut Vec<TrackDecoders>,
    decoding: &mut Decoding<'_>,
) -> Vec<Layer> {
    tracks.resize_with(video.len(), || {
        ClipDecoders::new(|| TrackVideo::new(sequence.timebase))
    });
    let canvas = [sequence.format.width, sequence.format.height].map(|side| side as f32);
    let mut layers = Vec::new();
    for (index, track) in video.iter().enumerate() {
        if track.muted {
            continue;
        }
        let planned = track_layers_at(sequence, track, pts);
        let showing: Vec<ClipId> = planned.iter().map(|layer| layer.clip.id).collect();
        for plan in planned.into_iter().filter(|plan| !plan.clip.disabled) {
            let Some(media) = sequence.media_item(plan.clip.source) else {
                continue;
            };
            let time = plan.clip.source_time(pts);
            let look = layer_look(
                &plan.clip.look,
                time,
                plan.reveal,
                canvas,
                sequence.timebase,
            );
            let grade = grade_for(&plan.clip.look.color, decoding.warnings);
            if media.generator == Some(Generator::AdjustmentLayer) {
                adjust(&mut layers, TrackRef::video(index), look, grade);
                continue;
            }
            // Decoded first, as the clip's own track decoders are borrowed below.
            let matte = track_matte(&plan.clip.look.effects)
                .filter(|matte| matte.track != index && matte.track < video.len())
                .map(|matte| {
                    let pictures =
                        matte_layers(sequence, video, matte.track, pts, tracks, decoding);
                    (matte, pictures)
                });
            let decoders = &mut tracks[index];
            let Some(picture) = clip_picture(sequence, &plan, media, decoders, &showing, decoding)
            else {
                continue;
            };
            let layer = Layer {
                track: TrackRef::video(index),
                picture,
                look,
                grade,
            };
            layers.push(match matte {
                Some((matte, pictures)) => matted(layer, matte, pictures),
                None => layer,
            });
        }
    }
    layers
}

/// The picture `plan` shows, decoded by the decoders of its track, which also has the
/// clips in `showing` on screen.
fn clip_picture(
    sequence: &Sequence,
    plan: &PlannedLayer<'_>,
    media: &MediaItem,
    decoders: &mut TrackDecoders,
    showing: &[ClipId],
    decoding: &mut Decoding<'_>,
) -> Option<LayerPicture> {
    let decoder = decoders.for_clip(plan.clip.id, showing);
    match &media.nest {
        Some(nest) => {
            let (time, nested) = (plan.source_time, &mut decoder.nested);
            let tracks = if nest.multicam {
                nest.video.get(plan.clip.angle..=plan.clip.angle)?
            } else {
                &nest.video[..]
            };
            let inner = video_layers(sequence, tracks, time, nested, decoding);
            (!inner.is_empty()).then_some(LayerPicture::Nest(inner))
        }
        None => {
            let size = [sequence.format.width, sequence.format.height];
            let effects = &plan.clip.look.effects;
            let time = posterized(effects, plan.source_time, sequence.timebase);
            let resolution = decoding.resolution;
            let frame = match plan.clip.interpolation {
                TimeInterpolation::FrameSampling => decoder.frame_at(media, time, size, resolution),
                TimeInterpolation::FrameBlending => {
                    decoder.blended_frame_at(media, time, size, resolution)
                }
            };
            match frame {
                Ok(frame) => frame
                    .map(|frame| scrolled(frame, sequence, plan.clip, media, time))
                    .map(|frame| with_timecode(frame, sequence, plan.clip, media, time))
                    .map(LayerPicture::Frame),
                Err(error) => {
                    decoder.reset();
                    if !decoding.warned.contains(&media.id) {
                        decoding.warned.push(media.id);
                        decoding.warnings.push(PlaybackError::Media(error));
                    }
                    decoder.offline(size).map(LayerPicture::Frame)
                }
            }
        }
    }
}

/// The track matte a clip's effects ask for; the last Track Matte Key with one wins.
#[derive(Debug, Clone, Copy)]
struct TrackMatte {
    track: usize,
    channel: MatteChannel,
    reverse: bool,
}

fn track_matte(effects: &[VideoEffect]) -> Option<TrackMatte> {
    effects.iter().rev().find_map(|effect| match *effect {
        VideoEffect::TrackMatteKey {
            matte: Some(track),
            channel,
            reverse,
        } => Some(TrackMatte {
            track,
            channel,
            reverse,
        }),
        _ => None,
    })
}

/// What video track `index` shows at `pts` for use as a matte, hidden or not. Its
/// adjustment layers and its own track mattes are left out.
fn matte_layers(
    sequence: &Sequence,
    video: &[Track],
    index: usize,
    pts: Ticks,
    tracks: &mut [TrackDecoders],
    decoding: &mut Decoding<'_>,
) -> Vec<Layer> {
    let canvas = [sequence.format.width, sequence.format.height].map(|side| side as f32);
    let planned = track_layers_at(sequence, &video[index], pts);
    let showing: Vec<ClipId> = planned.iter().map(|layer| layer.clip.id).collect();
    let mut layers = Vec::new();
    for plan in planned.iter().filter(|plan| !plan.clip.disabled) {
        let Some(media) = sequence
            .media_item(plan.clip.source)
            .filter(|media| media.generator != Some(Generator::AdjustmentLayer))
        else {
            continue;
        };
        let decoders = &mut tracks[index];
        if let Some(picture) = clip_picture(sequence, plan, media, decoders, &showing, decoding) {
            let time = plan.clip.source_time(pts);
            layers.push(Layer {
                track: TrackRef::video(index),
                picture,
                look: layer_look(
                    &plan.clip.look,
                    time,
                    plan.reveal,
                    canvas,
                    sequence.timebase,
                ),
                grade: grade_for(&plan.clip.look.color, decoding.warnings),
            });
        }
    }
    layers
}

/// `fill` shown through `pictures`, its blend mode applied once matted.
fn matted(fill: Layer, matte: TrackMatte, pictures: Vec<Layer>) -> Layer {
    let look = LayerLook {
        blend: fill.look.blend,
        ..LayerLook::IDENTITY
    };
    let track = fill.track;
    let fill = Layer {
        look: LayerLook {
            blend: BlendMode::Normal,
            ..fill.look
        },
        ..fill
    };
    Layer {
        track,
        picture: LayerPicture::Matted {
            fill: Box::new(fill),
            matte: pictures,
            channel: matte.channel,
            reverse: matte.reverse,
        },
        look,
        grade: None,
    }
}

/// Applies an adjustment layer's look to the picture of `layers` as one. Below full
/// opacity, or outside its masks, the adjusted picture blends over the untouched one,
/// as its opacity is meant to.
fn adjust(layers: &mut Vec<Layer>, track: TrackRef, look: LayerLook, grade: Option<Arc<Grade>>) {
    if layers.is_empty() {
        return;
    }
    let below = if look.opacity < 1.0 || look.opacity_masks.iter().any(Option::is_some) {
        layers.clone()
    } else {
        std::mem::take(layers)
    };
    layers.push(Layer {
        track,
        picture: LayerPicture::Nest(below),
        look,
        grade,
    });
}

/// The look at source time `time`, where its keyframes are, as a transition reveals it
/// on a canvas of `canvas` pixels.
fn layer_look(
    look: &Look,
    time: Ticks,
    reveal: Reveal,
    canvas: [f32; 2],
    timebase: Timebase,
) -> LayerLook {
    let value = |param: &Param| param.value_at(time);
    let percent = |param: &Param| param.value_at(time) / 100.0;
    let crop = &look.crop;
    let position = look.motion.position.each_ref().map(value);
    // Zooming about the canvas center scales the picture's offset from it too.
    let offset =
        [0, 1].map(|axis| (position[axis] + reveal.shift[axis] * canvas[axis]) * reveal.zoom);
    LayerLook {
        offset,
        scale: look
            .motion
            .scale_at(time)
            .map(|percent| percent / 100.0 * reveal.zoom),
        anchor: look.motion.anchor.each_ref().map(value),
        rotation_degrees: value(&look.motion.rotation),
        crop: [&crop.left, &crop.top, &crop.right, &crop.bottom].map(percent),
        opacity: percent(&look.opacity) * transform_opacity(&look.effects, time) * reveal.fade,
        opacity_masks: opacity_masks(&look.masks, time),
        blur: look.blurriness_at(time) / BLURRINESS_PER_SIGMA,
        color: color_adjust(&look.color, time),
        wipes: reveal.wipes.map(|wipe| {
            wipe.map(|wipe| LayerWipe {
                pattern: wipe.pattern,
                from: wipe.from,
                progress: wipe.progress,
                sweep: wipe.sweep,
                border: f32::from(wipe.border.width) / canvas[1],
                border_color: linear(wipe.border.color),
            })
        }),
        wash: reveal.wash,
        effects: turned(pixel_effects(&look.effects, time, timebase), reveal, canvas),
        blend: reveal.blend.unwrap_or(look.blend),
    }
}

/// `effects` with the transition's turn last, after the clip's own warps.
fn turned(mut effects: PixelEffects, reveal: Reveal, [width, height]: [f32; 2]) -> PixelEffects {
    let Some(turn) = reveal.turn else {
        return effects;
    };
    let aspect = width / height.max(1.0);
    let pivot = match (turn.cube, turn.vertical) {
        (false, _) => 0.0,
        (true, false) => aspect / 2.0,
        (true, true) => 0.5,
    };
    let (swivel, tilt) = match turn.vertical {
        false => (turn.angle, 0.0),
        true => (0.0, turn.angle),
    };
    // A cube backs away as it turns, keeping its near edge clear of the eye.
    let distance = pivot * (2.0 * turn.angle).sin().abs() / 2.0;
    effects.reshape.push(Warp::Turn {
        swivel,
        tilt,
        distance,
        pivot,
    });
    effects
}

fn color_adjust(color: &ColorCorrection, time: Ticks) -> ColorAdjust {
    let unit = |param: &Param| param.value_at(time) / 100.0;
    ColorAdjust {
        exposure: color.exposure.value_at(time),
        contrast: unit(&color.contrast),
        highlights: unit(&color.highlights),
        shadows: unit(&color.shadows),
        whites: unit(&color.whites),
        blacks: unit(&color.blacks),
        wheels: [
            &color.shadow_wheel,
            &color.midtone_wheel,
            &color.highlight_wheel,
        ]
        .map(|wheel| wheel.params().map(unit)),
        temperature: unit(&color.temperature),
        tint: unit(&color.tint),
        saturation: unit(&color.saturation),
        vibrance: unit(&color.vibrance),
        look_intensity: unit(&color.look_intensity),
        secondary: secondary_adjust(&color.secondary, time),
    }
}

fn secondary_adjust(secondary: &HslSecondary, time: Ticks) -> Option<SecondaryAdjust> {
    if !secondary.enabled {
        return None;
    }
    let unit = |param: &Param| param.value_at(time) / 100.0;
    let turns = |param: &Param| param.value_at(time) / 360.0;
    Some(SecondaryAdjust {
        hue: turns(&secondary.hue),
        hue_width: turns(&secondary.hue_width),
        saturation_range: secondary.saturation_range.each_ref().map(unit),
        lightness_range: secondary.lightness_range.each_ref().map(unit),
        softness: unit(&secondary.softness),
        inverted: secondary.inverted,
        show_mask: secondary.show_mask,
        temperature: unit(&secondary.temperature),
        tint: unit(&secondary.tint),
        contrast: unit(&secondary.contrast),
        saturation: unit(&secondary.saturation),
        wheel: secondary.wheel.params().map(unit),
    })
}

/// The first [`MAX_MASKS`] masks; the effect controls offer no more. A path still being
/// drawn masks nothing yet.
fn opacity_masks(masks: &[Mask], time: Ticks) -> [Option<LayerMask>; MAX_MASKS] {
    let fraction = |param: &Param| param.value_at(time) / 100.0;
    let mut layer_masks = [None; MAX_MASKS];
    let shaped = masks.iter().filter_map(|mask| {
        let outline = Outline::new(&mask.outline_at(time, MAX_OUTLINE_POINTS));
        (mask.shape != MaskShape::Path || !outline.is_empty()).then_some((mask, outline))
    });
    for (slot, (mask, outline)) in layer_masks.iter_mut().zip(shaped) {
        *slot = Some(LayerMask {
            shape: mask.shape,
            center: mask.center.each_ref().map(fraction),
            size: mask.size.each_ref().map(fraction),
            rotation_degrees: mask.rotation.value_at(time),
            feather: mask.feather.value_at(time),
            expansion: mask.expansion.value_at(time),
            opacity: fraction(&mask.opacity),
            inverted: mask.inverted,
            outline,
        });
    }
    layer_masks
}
#[cfg(test)]
mod tests {
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
            look: layer_look(
                &Look::default(),
                Ticks::ZERO,
                Reveal::FULL,
                [1.0, 1.0],
                Timebase::HZ_90K,
            ),
            grade: None,
        }
    }

    #[test]
    fn adjustment_groups_the_picture_below_and_blends_over_it_when_faded() {
        let mut look = Look::default();
        let mut layers = vec![layer(0), layer(1)];
        let full = layer_look(
            &look,
            Ticks::ZERO,
            Reveal::FULL,
            [1.0, 1.0],
            Timebase::HZ_90K,
        );
        adjust(&mut layers, TrackRef::video(2), full, None);
        assert!(
            matches!(&layers[..], [Layer { picture: LayerPicture::Nest(below), .. }] if below.len() == 2)
        );

        look.opacity = Param::from_value(50.0);
        let faded = layer_look(
            &look,
            Ticks::ZERO,
            Reveal::FULL,
            [1.0, 1.0],
            Timebase::HZ_90K,
        );
        let mut layers = vec![layer(0)];
        adjust(&mut layers, TrackRef::video(1), faded, None);
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
