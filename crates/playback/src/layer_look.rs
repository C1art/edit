use render::{
    ColorAdjust, Creative, CreativeVignette, LayerLook, LayerMask, LayerWipe, Outline, PageFold,
    PixelEffects, SecondaryAdjust, Warp, MAX_MASKS, MAX_OUTLINE_POINTS,
};
use timeline::{
    ColorCorrection, HslSecondary, Look, Mask, MaskShape, MediaItem, Param, Reveal, Ticks, Timebase,
};

use crate::pixel_effects::{linear, pixel_effects, transform_opacity};

/// Gaussian blur's "blurriness" reaches about two standard deviations.
const BLURRINESS_PER_SIGMA: f32 = 2.0;
/// The Sharpen effect's default amount, as a share.
const SHARPEN_DEFAULT: f32 = 0.5;

/// `look` for a picture of `media`, drawn with the pixel shape and alpha it's
/// interpreted with.
pub(crate) fn interpreted(mut look: LayerLook, media: &MediaItem) -> LayerLook {
    look.pixel_aspect = media.pixel_aspect().ratio();
    look.effects.alpha = media.alpha;
    look
}

/// The look at source time `time`, where its keyframes are, as a transition reveals it
/// on a canvas of `canvas` pixels.
pub(crate) fn layer_look(
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
    let mut effects = pixel_effects(&look.effects, time, timebase);
    // Lumetri's sharpen at 100 is the Sharpen effect's default.
    effects.sharpen += SHARPEN_DEFAULT * percent(&look.color.sharpen).max(0.0);
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
                softness: wipe.softness,
            })
        }),
        wash: reveal.wash,
        effects: smeared(turned(effects, reveal, canvas), reveal, canvas),
        blend: reveal.blend.unwrap_or(look.blend),
        pixel_aspect: 1.0,
    }
}

/// `effects` blurred along the transition's motion, in place of the clip's own
/// directional blur.
fn smeared(mut effects: PixelEffects, reveal: Reveal, canvas: [f32; 2]) -> PixelEffects {
    let [x, y] = [0, 1].map(|axis| reveal.smear[axis] * canvas[axis]);
    if x != 0.0 || y != 0.0 {
        effects.style.blur_length = x.hypot(y);
        effects.style.blur_direction = x.atan2(-y);
    }
    effects
}

/// `effects` with the transition's turn last, after the clip's own warps, and its page
/// fold.
fn turned(mut effects: PixelEffects, reveal: Reveal, [width, height]: [f32; 2]) -> PixelEffects {
    effects.reshape.fold = reveal.fold.map(|fold| PageFold {
        corner: fold.corner,
        progress: fold.progress,
        curl: fold.curl,
    });
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
        creative: creative(color, time),
        secondary: secondary_adjust(&color.secondary, time),
    }
}

fn creative(color: &ColorCorrection, time: Ticks) -> Creative {
    let unit = |param: &Param| param.value_at(time) / 100.0;
    let vignette = &color.vignette;
    Creative {
        faded_film: unit(&color.faded_film).clamp(0.0, 1.0),
        shadow_tint: color.shadow_tint.each_ref().map(unit),
        highlight_tint: color.highlight_tint.each_ref().map(unit),
        tint_balance: unit(&color.tint_balance).clamp(-1.0, 1.0),
        vignette: CreativeVignette {
            amount: (vignette.amount.value_at(time) / 5.0).clamp(-1.0, 1.0),
            midpoint: unit(&vignette.midpoint).clamp(0.0, 1.0),
            roundness: unit(&vignette.roundness).clamp(-1.0, 1.0),
            feather: unit(&vignette.feather).clamp(0.0, 1.0),
        },
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
