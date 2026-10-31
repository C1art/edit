//! Clips that read the picture of another video track: track mattes, Difference Matte,
//! Compound Blur and Texturize.

use render::{Combine, DifferenceShown, LayerLook};
use timeline::{BlendMode, DifferenceView, MatteChannel, Param, Ticks, VideoEffect};

use crate::{Layer, LayerPicture};

/// The track matte a clip's effects ask for; the last Track Matte Key or Set Matte with
/// one wins.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TrackMatte {
    pub(crate) track: usize,
    channel: MatteChannel,
    reverse: bool,
}

pub(crate) fn track_matte(effects: &[VideoEffect]) -> Option<TrackMatte> {
    effects.iter().rev().find_map(|effect| match *effect {
        VideoEffect::TrackMatteKey {
            matte: Some(track),
            channel,
            reverse,
        }
        | VideoEffect::SetMatte {
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

/// The track a clip's Difference Matte, Compound Blur or Texturize reads and how the
/// clip combines with it at `time`; the last of them with a track chosen wins.
pub(crate) fn combination(effects: &[VideoEffect], time: Ticks) -> Option<(usize, Combine)> {
    let value = |param: &Param| param.value_at(time);
    effects.iter().rev().find_map(|effect| match effect {
        VideoEffect::DifferenceMatte(matte) => Some((
            matte.difference_layer?,
            Combine::Difference {
                tolerance: value(&matte.matching_tolerance) / 100.0,
                softness: value(&matte.matching_softness) / 100.0,
                blur: value(&matte.blur_before_difference),
                view: match matte.view {
                    DifferenceView::FinalOutput => DifferenceShown::Keyed,
                    DifferenceView::SourceOnly => DifferenceShown::Source,
                    DifferenceView::MatteOnly => DifferenceShown::Matte,
                },
            },
        )),
        VideoEffect::CompoundBlur(blur) => Some((
            blur.blur_layer?,
            Combine::Blur {
                maximum: value(&blur.maximum_blur),
                invert: blur.invert_blur,
            },
        )),
        VideoEffect::Texturize(texture) => Some((
            texture.texture_layer?,
            Combine::Texture {
                direction: value(&texture.light_direction).to_radians(),
                contrast: value(&texture.texture_contrast),
            },
        )),
        _ => None,
    })
}

/// `fill` shown through `pictures`, its blend mode applied once matted.
pub(crate) fn matted(fill: Layer, matte: TrackMatte, pictures: Vec<Layer>) -> Layer {
    wrapped(fill, |fill| LayerPicture::Matted {
        fill: Box::new(fill),
        matte: pictures,
        channel: matte.channel,
        reverse: matte.reverse,
    })
}

/// `layer` combined with `other`, its blend mode applied to the result.
pub(crate) fn combined(layer: Layer, combine: Combine, other: Vec<Layer>) -> Layer {
    wrapped(layer, |layer| LayerPicture::Combined {
        layer: Box::new(layer),
        other,
        combine,
    })
}

/// The picture `wrap` makes of `layer`, drawn normally inside it, with the blend mode
/// moved out to apply to the whole.
fn wrapped(layer: Layer, wrap: impl FnOnce(Layer) -> LayerPicture) -> Layer {
    let look = LayerLook {
        blend: layer.look.blend,
        ..LayerLook::IDENTITY
    };
    let track = layer.track;
    let inner = Layer {
        look: LayerLook {
            blend: BlendMode::Normal,
            ..layer.look
        },
        ..layer
    };
    Layer {
        track,
        picture: wrap(inner),
        look,
        grade: None,
    }
}
