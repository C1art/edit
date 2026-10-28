use timeline::BlendMode;

use crate::mask::{self, LayerMask, MAX_MASKS};
#[cfg(test)]
use crate::mask::{Outline, MAX_OUTLINE_POINTS};
use crate::pixel::PixelEffects;
use crate::secondary::SecondaryAdjust;
use crate::wipe::{self, LayerWipe};

/// How a layer is drawn: everything except the picture itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerLook {
    /// From the canvas center to the picture's center, in canvas pixels, +y down.
    pub offset: [f32; 2],
    /// Width and height relative to the picture fitted to the canvas.
    pub scale: [f32; 2],
    /// The point of the fitted picture, from its center in canvas pixels, that sits at
    /// `offset` and that the picture scales and turns about.
    pub anchor: [f32; 2],
    /// Clockwise.
    pub rotation_degrees: f32,
    /// Fractions of the picture removed from the left, top, right and bottom.
    pub crop: [f32; 4],
    pub opacity: f32,
    /// What of the picture the opacity applies to; none keeps all of it.
    pub opacity_masks: [Option<LayerMask>; MAX_MASKS],
    /// Gaussian blur standard deviation in canvas pixels.
    pub blur: f32,
    pub color: ColorAdjust,
    /// What of the canvas transitions show the layer in, whatever its placement.
    pub wipes: [Option<LayerWipe>; 2],
    /// 0 to 1: how far the picture is washed out to white.
    pub wash: f32,
    pub effects: PixelEffects,
    pub blend: BlendMode,
}

/// Color correction in linear light, every field neutral at zero except saturation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorAdjust {
    /// Stops.
    pub exposure: f32,
    /// -1 to 1: the slope around mid grey in log space grows by this.
    pub contrast: f32,
    /// -1 to 1.
    pub highlights: f32,
    /// -1 to 1.
    pub shadows: f32,
    /// -1 to 1.
    pub whites: f32,
    /// -1 to 1.
    pub blacks: f32,
    /// Shadows, midtones and highlights: each a push toward blue and red (Cb and Cr,
    /// -1 to 1) and a brightness offset (-1 to 1).
    pub wheels: [[f32; 3]; 3],
    /// -1 (cool) to 1 (warm).
    pub temperature: f32,
    /// -1 (green) to 1 (magenta).
    pub tint: f32,
    /// 1 leaves color as is, 0 is monochrome.
    pub saturation: f32,
    /// -1 to 1.
    pub vibrance: f32,
    /// 0 to 1: how much of a creative look LUT applies.
    pub look_intensity: f32,
    pub secondary: Option<SecondaryAdjust>,
}

impl ColorAdjust {
    pub const IDENTITY: Self = Self {
        exposure: 0.0,
        contrast: 0.0,
        highlights: 0.0,
        shadows: 0.0,
        whites: 0.0,
        blacks: 0.0,
        wheels: [[0.0; 3]; 3],
        temperature: 0.0,
        tint: 0.0,
        saturation: 1.0,
        vibrance: 0.0,
        look_intensity: 1.0,
        secondary: None,
    };
}

impl LayerLook {
    pub const IDENTITY: Self = Self {
        offset: [0.0, 0.0],
        scale: [1.0; 2],
        anchor: [0.0; 2],
        rotation_degrees: 0.0,
        crop: [0.0; 4],
        opacity: 1.0,
        opacity_masks: [None; MAX_MASKS],
        blur: 0.0,
        color: ColorAdjust::IDENTITY,
        wipes: [None; 2],
        wash: 0.0,
        effects: PixelEffects::NONE,
        blend: BlendMode::Normal,
    };
}

/// Where a layer lands in normalized device coordinates (the canvas spans -1..1, +y
/// up): the center of the uncropped picture and the vectors along its full width and
/// height, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placement {
    pub(crate) center: [f32; 2],
    pub(crate) axis_x: [f32; 2],
    pub(crate) axis_y: [f32; 2],
    pub(crate) crop: [f32; 4],
    pub(crate) opacity: f32,
    pub(crate) opacity_masks: [Option<LayerMask>; MAX_MASKS],
    /// The picture fitted to the canvas, in canvas pixels, which masks are measured in.
    pub(crate) fitted: [f32; 2],
    pub(crate) color: ColorAdjust,
    pub(crate) wipes: [Option<LayerWipe>; 2],
    pub(crate) wash: f32,
    pub(crate) effects: PixelEffects,
    pub(crate) blend: BlendMode,
    /// Whether the picture's colors are already multiplied by its alpha.
    pub(crate) premultiplied: bool,
    /// What the shader reads of the layer's lookups.
    pub(crate) grade: [f32; 4],
}

impl Placement {
    pub(crate) const FULL: Self = Self {
        center: [0.0, 0.0],
        axis_x: [2.0, 0.0],
        axis_y: [0.0, -2.0],
        crop: [0.0; 4],
        opacity: 1.0,
        opacity_masks: [None; MAX_MASKS],
        fitted: [0.0; 2],
        color: ColorAdjust::IDENTITY,
        wipes: [None; 2],
        wash: 0.0,
        effects: PixelEffects::NONE,
        blend: BlendMode::Normal,
        premultiplied: false,
        grade: [0.0; 4],
    };

    /// Fits the layer to the canvas keeping its aspect ratio, then applies the look.
    pub(crate) fn new(layer: (u32, u32), canvas: (u32, u32), look: &LayerLook) -> Self {
        let (cw, ch) = (canvas.0 as f32, canvas.1 as f32);
        let fitted = [layer.0, layer.1].map(|side| side as f32 * fit_scale(layer, canvas));
        let [width, height] = [0, 1].map(|axis| fitted[axis] * look.scale[axis]);
        let (sin, cos) = look.rotation_degrees.to_radians().sin_cos();
        // Rotating in pixel space, where +y is down, turns positive angles clockwise.
        let rotate = |x: f32, y: f32| [x * cos - y * sin, x * sin + y * cos];
        let [flip_x, flip_y] = look.effects.flip.map(|flip| if flip { -1.0 } else { 1.0 });
        let to_ndc = |[x, y]: [f32; 2]| [x * 2.0 / cw, -y * 2.0 / ch];
        Self {
            center: to_ndc({
                let [ax, ay] = rotate(
                    look.anchor[0] * look.scale[0],
                    look.anchor[1] * look.scale[1],
                );
                [look.offset[0] - ax, look.offset[1] - ay]
            }),
            axis_x: to_ndc(rotate(width * flip_x, 0.0)),
            axis_y: to_ndc(rotate(0.0, height * flip_y)),
            crop: look.crop.map(|side| side.clamp(0.0, 1.0)),
            opacity: look.opacity.clamp(0.0, 1.0),
            opacity_masks: look.opacity_masks,
            fitted,
            color: look.color,
            wipes: look.wipes,
            wash: look.wash.clamp(0.0, 1.0),
            effects: look.effects,
            blend: look.blend,
            premultiplied: false,
            grade: [0.0; 4],
        }
    }

    /// Width over height of the picture; 0 before it has a size.
    fn aspect(&self) -> f32 {
        let [width, height] = self.fitted;
        if height > 0.0 {
            width / height
        } else {
            0.0
        }
    }

    pub(crate) fn to_bytes(self) -> Vec<u8> {
        let [cx, cy] = self.center;
        let [xx, xy] = self.axis_x;
        let [yx, yy] = self.axis_y;
        let color = &self.color;
        let premultiplied = if self.premultiplied { 1.0 } else { 0.0 };
        [cx, cy, xx, xy, yx, yy, self.opacity, premultiplied]
            .into_iter()
            .chain(self.crop)
            .chain([
                color.exposure,
                color.contrast,
                color.highlights,
                color.shadows,
            ])
            .chain([color.temperature, color.tint, color.saturation, self.wash])
            .chain(wipe::uniform(&self.wipes))
            .chain(self.effects.uniform())
            // The shader numbers blend modes in declaration order.
            .chain([
                color.whites,
                color.blacks,
                color.vibrance,
                self.blend as u32 as f32,
            ])
            .chain(
                color
                    .wheels
                    .into_iter()
                    .flat_map(|[blue, red, level]| [blue, red, level, 0.0]),
            )
            .chain(self.grade)
            .chain(SecondaryAdjust::uniform(color.secondary.as_ref()))
            .chain(mask::uniform(&self.opacity_masks, self.fitted))
            .chain(self.effects.style.uniform())
            .chain(self.effects.reshape.uniform(self.aspect()))
            .chain(self.effects.recolor.uniform())
            .chain(self.effects.key_uniform())
            .flat_map(f32::to_le_bytes)
            .collect()
    }
}

/// Canvas pixels per layer pixel when the layer is fitted to the canvas.
pub(crate) fn fit_scale(layer: (u32, u32), canvas: (u32, u32)) -> f32 {
    let (lw, lh) = (layer.0 as f32, layer.1 as f32);
    (canvas.0 as f32 / lw).min(canvas.1 as f32 / lh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rounded(values: [f32; 2]) -> [f32; 2] {
        values.map(|value| (value * 1000.0).round() / 1000.0)
    }

    #[test]
    fn fit_pillarboxes_a_narrower_layer() {
        let placement = Placement::new((960, 1080), (1920, 1080), &LayerLook::IDENTITY);
        assert_eq!(placement.axis_x, [1.0, 0.0]);
        assert_eq!(placement.axis_y, [0.0, -2.0]);
    }

    #[test]
    fn placement_with_every_mask_fits_its_uniform() {
        let mask = LayerMask {
            shape: timeline::MaskShape::Rectangle,
            center: [0.5; 2],
            size: [0.5; 2],
            rotation_degrees: 0.0,
            feather: 10.0,
            expansion: 0.0,
            opacity: 1.0,
            inverted: true,
            outline: Outline::NONE,
        };
        let look = LayerLook {
            opacity_masks: [Some(mask); MAX_MASKS],
            ..LayerLook::IDENTITY
        };
        let bytes = Placement::new((1920, 1080), (1920, 1080), &look).to_bytes();
        // Twenty vec4s, three of them added by the wipes, lead the shader's struct. The
        // masks follow: the header, three vec4s each, then their outlines at two points
        // a vec4; then the picture style's thirteen, the reshape's five, the recolor's twelve
        // and the key's two.
        let outlines = MAX_MASKS * MAX_OUTLINE_POINTS / 2;
        assert_eq!(
            bytes.len(),
            (23 + 1 + 3 * MAX_MASKS + outlines + 13 + 5 + 12 + 2) * 16
        );
        assert!(bytes.len() as u64 <= crate::blit::UNIFORM_BYTES);
    }

    #[test]
    fn motion_moves_scales_and_turns_clockwise() {
        let look = LayerLook {
            offset: [250.0, 250.0],
            scale: [0.5; 2],
            rotation_degrees: 90.0,
            ..LayerLook::IDENTITY
        };
        let placement = Placement::new((1000, 1000), (1000, 1000), &look);
        // A quarter of the way right and down from the center.
        assert_eq!(placement.center, [0.5, -0.5]);
        // The top edge now runs down the screen, at half the width.
        assert_eq!(rounded(placement.axis_x), [0.0, -1.0]);
        assert_eq!(rounded(placement.axis_y), [-1.0, 0.0]);
    }

    #[test]
    fn the_anchor_stays_put_as_the_picture_scales_and_turns_about_it() {
        let look = LayerLook {
            offset: [100.0, 0.0],
            scale: [2.0, 1.0],
            anchor: [100.0, 0.0],
            rotation_degrees: 90.0,
            ..LayerLook::IDENTITY
        };
        let placement = Placement::new((1000, 1000), (1000, 1000), &look);
        // The anchor, 100 pixels right of center, doubled to 200 and turned to point
        // down, leaves the center 200 pixels above the anchor's place.
        assert_eq!(rounded(placement.center), [0.2, 0.4]);
        assert_eq!(rounded(placement.axis_x), [0.0, -4.0]);
        assert_eq!(rounded(placement.axis_y), [-2.0, 0.0]);
    }
}
