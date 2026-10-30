//! Clips whose effects reach only inside their own masks: the picture with the other
//! effects, and over it, for each masked effect, the picture with that effect too,
//! shown inside its masks and only where the picture under it shows, so the clip's own
//! masks and mattes keep it too. Together they take the clip's opacity and blend mode.

use render::{LayerLook, LayerMask, Reach, MAX_MASKS};
use timeline::{BlendMode, ColorCorrection, Look, Mask, MaskOwner, Ticks};

use crate::layer_look::layer_mask;
use crate::{Layer, LayerPicture};

/// The layers `layer_of` makes for `look` split up by its effect masks.
pub(crate) fn effect_masked(look: &Look, time: Ticks, layer_of: impl Fn(&Look) -> Layer) -> Layer {
    let (plain, reaching) = look.split_by_effect_masks();
    let under = layer_of(&plain);
    let (track, outer) = (
        under.track,
        LayerLook {
            blend: under.look.blend,
            opacity: under.look.opacity,
            ..LayerLook::IDENTITY
        },
    );
    let drawn_plainly = |mut layer: Layer| {
        layer.look.blend = BlendMode::Normal;
        layer.look.opacity = 1.0;
        layer
    };
    let overs = reaching.into_iter().filter_map(|(look, masks)| {
        let mut over = drawn_plainly(layer_of(&look));
        over.look.opacity_masks = kept_inside(masks, &[None; MAX_MASKS], time)?;
        over.look.reach = Reach::OverBelow;
        Some(over)
    });
    let layers = std::iter::once(drawn_plainly(under)).chain(overs).collect();
    Layer {
        track,
        picture: LayerPicture::Nest(layers),
        look: outer,
        grade: None,
    }
}

/// The effect's masks, cut down to what the layer's own masks and mattes keep, so the
/// effect never reaches where the layer doesn't. Several layer masks, which add
/// together, cut down to where they overlap. `None` while no effect mask has a shape.
fn kept_inside(
    masks: &[Mask],
    clip_masks: &[Option<LayerMask>; MAX_MASKS],
    time: Ticks,
) -> Option<[Option<LayerMask>; MAX_MASKS]> {
    let mut effect_masks = masks
        .iter()
        .filter_map(|mask| layer_mask(mask, time))
        .peekable();
    effect_masks.peek()?;
    let cutting = clip_masks.iter().flatten().map(|&mask| LayerMask {
        intersect: true,
        ..mask
    });
    let mut kept = [None; MAX_MASKS];
    for (slot, mask) in kept.iter_mut().zip(effect_masks.chain(cutting)) {
        *slot = Some(mask);
    }
    Some(kept)
}

/// The adjustment layer's effects that reach everywhere, then each masked one over the
/// result inside its masks, each a look `look_of` turns into the layer look to adjust by.
pub(crate) fn adjustment_looks(
    look: &Look,
    time: Ticks,
    look_of: impl Fn(&Look) -> LayerLook,
) -> Vec<LayerLook> {
    if !look.has_effect_masks() {
        return vec![look_of(look)];
    }
    let (plain, reaching) = look.split_by_effect_masks();
    let masked = (look.effects.iter().enumerate())
        .filter(|&(effect, _)| !look.masks_of(MaskOwner::Effect(effect)).is_empty());
    let overs = masked
        .zip(reaching)
        .filter_map(|((_, effect), (_, masks))| {
            let alone = Look {
                effects: vec![effect.clone()],
                color: ColorCorrection::default(),
                ..plain.clone()
            };
            let mut over = look_of(&alone);
            over.opacity_masks = kept_inside(masks, &over.opacity_masks, time)?;
            Some(over)
        });
    std::iter::once(look_of(&plain)).chain(overs).collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use media::VideoFrame;
    use timeline::{MaskShape, Param, Reveal, Timebase, TrackRef, VideoEffect};

    use super::*;
    use crate::layer_look::layer_look;

    #[test]
    fn a_masked_effect_draws_inside_its_masks_over_only_the_plain_picture() {
        let mut look = Look {
            opacity: Param::from_value(50.0),
            blend: BlendMode::Multiply,
            masks: vec![Mask::new(MaskShape::Rectangle)],
            effects: vec![VideoEffect::BlackAndWhite],
            ..Look::default()
        };
        if let Some(masks) = look.masks_of_mut(MaskOwner::Effect(0)) {
            masks.push(Mask::new(MaskShape::Ellipse));
        }
        let frame = VideoFrame {
            pts: Ticks::ZERO,
            width: 1,
            height: 1,
            pixels: vec![0; 4],
        };
        let layer_of = |look: &Look| Layer {
            track: TrackRef::video(0),
            picture: LayerPicture::Frame(Arc::new(frame.clone())),
            look: layer_look(look, Ticks::ZERO, Reveal::FULL, [1.0; 2], Timebase::HZ_90K),
            grade: None,
        };
        let layer = effect_masked(&look, Ticks::ZERO, layer_of);
        assert_eq!(
            (layer.look.opacity, layer.look.blend),
            (0.5, BlendMode::Multiply)
        );
        let LayerPicture::Nest(layers) = &layer.picture else {
            panic!("not nested");
        };
        let [under, over] = &layers[..] else {
            panic!("{} layers", layers.len());
        };
        for inner in [under, over] {
            assert_eq!(
                (inner.look.opacity, inner.look.blend),
                (1.0, BlendMode::Normal)
            );
        }
        let masks: Vec<(MaskShape, bool)> = (over.look.opacity_masks.iter().flatten())
            .map(|mask| (mask.shape, mask.intersect))
            .collect();
        assert_eq!(masks, [(MaskShape::Ellipse, false)]);
        assert_eq!(over.look.reach, Reach::OverBelow);
    }
}
