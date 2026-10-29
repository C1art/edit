use std::sync::Arc;

use render::{Grade, LayerLook};
use timeline::{
    track_layers_at, ClipId, Generator, MediaItem, PlannedLayer, Reveal, Sequence, SourceId, Ticks,
    TimeInterpolation, Track, TrackRef,
};

use crate::clip_decoders::ClipDecoders;
use crate::echo::{echo_of, echoed_frame_at};
use crate::generated::{caption_frame, scrolled};
use crate::grades::grade_for;
use crate::image_matte::image_matted;
use crate::layer_look::{interpreted, layer_look};
use crate::layered::{combination, combined, matted, track_matte};
use crate::morph_cut::morphed;
use crate::paint_bucket::bucket_filled;
use crate::time_effects::{posterized, with_burn_ins};
use crate::track_video::TrackVideo;
use crate::write_on::written_on;
use crate::{Layer, LayerPicture, PlaybackError, Resolution};

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
        // The picture just drawn for this track, and where its layers start, which a
        // morph cut replaces with its blend of that picture and the next.
        let mut previous: Option<(usize, LayerPicture)> = None;
        for plan in planned.into_iter().filter(|plan| !plan.clip.disabled) {
            let outgoing = previous.take();
            let Some(media) = sequence.media_item(plan.clip.source) else {
                continue;
            };
            let time = plan.clip.source_time(pts);
            let look_of = |reveal| {
                let look = layer_look(&plan.clip.look, time, reveal, canvas, sequence.timebase);
                interpreted(look, media)
            };
            let grade = grade_for(&plan.clip.look.color, decoding.warnings);
            if media.generator == Some(Generator::AdjustmentLayer) {
                let whole = Reveal {
                    split: None,
                    ..plan.reveal
                };
                adjust(&mut layers, TrackRef::video(index), look_of(whole), grade);
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
            let combination = combination(&plan.clip.look.effects, time)
                .filter(|(track, _)| *track != index && *track < video.len())
                .map(|(track, combine)| {
                    let pictures = matte_layers(sequence, video, track, pts, tracks, decoding);
                    (combine, pictures)
                });
            let decoders = &mut tracks[index];
            let Some(picture) = clip_picture(sequence, &plan, media, decoders, &showing, decoding)
            else {
                continue;
            };
            let morph = plan
                .morph
                .zip(outgoing)
                .and_then(|(fraction, (start, from))| {
                    Some((start, morphed(&from, &picture, fraction)?))
                });
            let (picture, reveal) = match morph {
                Some((start, morph)) => {
                    layers.truncate(start);
                    (morph, Reveal::FULL)
                }
                None => (picture, plan.reveal),
            };
            let start = layers.len();
            // A split picture is drawn once for each piece, every piece in its place.
            for piece in reveal.pieces() {
                let layer = Layer {
                    track: TrackRef::video(index),
                    picture: picture.clone(),
                    look: look_of(piece),
                    grade: grade.clone(),
                };
                let layer = match &combination {
                    Some((combine, pictures)) => combined(layer, *combine, pictures.clone()),
                    None => layer,
                };
                layers.push(match &matte {
                    Some((matte, pictures)) => matted(layer, *matte, pictures.clone()),
                    None => layer,
                });
            }
            previous = Some((start, picture));
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
            let echo = echo_of(effects);
            decoder.keep_history(0);
            let frame = match (echo, plan.clip.interpolation) {
                (Some(echo), _) => {
                    echoed_frame_at(decoder, sequence, media, (time, echo), size, resolution)
                }
                (None, TimeInterpolation::FrameSampling) => {
                    decoder.frame_at(media, time, size, resolution)
                }
                (None, interpolation) => {
                    decoder.interpolated_frame_at(media, time, size, resolution, interpolation)
                }
            };
            match frame {
                Ok(frame) => frame
                    .map(|frame| scrolled(frame, sequence, plan.clip, media, time))
                    .map(|frame| bucket_filled(frame, effects, time))
                    .map(|frame| written_on(frame, plan.clip, sequence.timebase, time))
                    .map(|frame| image_matted(frame, effects, sequence.timebase, decoding.warnings))
                    .map(|frame| with_burn_ins(frame, sequence, plan.clip, media, time))
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
            let grade = grade_for(&plan.clip.look.color, decoding.warnings);
            layers.extend(plan.reveal.pieces().into_iter().map(|piece| Layer {
                track: TrackRef::video(index),
                picture: picture.clone(),
                look: interpreted(
                    layer_look(&plan.clip.look, time, piece, canvas, sequence.timebase),
                    media,
                ),
                grade: grade.clone(),
            }));
        }
    }
    layers
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

#[cfg(test)]
mod tests {
    use media::VideoFrame;
    use timeline::{Look, Param, Timebase};

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
