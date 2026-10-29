//! Checking layers and preparing their slots and steps before a composite draws them.

use super::slots::SlotKind;
use super::{Compositor, LayerImage, Picture, Step};
use crate::pixel::PixelEffects;
use crate::placement::{fit_scale, LayerLook};
use crate::{yuv, RenderError};

impl Compositor {
    pub(super) fn check_layers(&self, layers: &[LayerImage<'_>]) -> Result<(), RenderError> {
        for layer in layers {
            match &layer.picture {
                Picture::Pixels {
                    width,
                    height,
                    rgba,
                    ..
                } => {
                    self.check_size(*width, *height)?;
                    if rgba.len() != *width as usize * *height as usize * 4 {
                        return Err(RenderError::FrameSize {
                            width: *width,
                            height: *height,
                            bytes: rgba.len(),
                        });
                    }
                }
                Picture::Planes {
                    width,
                    height,
                    output,
                    luma,
                    chroma,
                    format,
                    ..
                } => {
                    self.check_size(*width, *height)?;
                    self.check_size(output.0, output.1)?;
                    let size = (*width, *height);
                    if !yuv::planes_hold((*luma, chroma), size, *format) {
                        return Err(RenderError::PlaneSize {
                            width: *width,
                            height: *height,
                        });
                    }
                }
                Picture::Group(members) => self.check_layers(members)?,
                Picture::Matted {
                    fill: first,
                    matte: second,
                    ..
                }
                | Picture::Combined {
                    layer: first,
                    other: second,
                    ..
                } => {
                    self.check_layers(first)?;
                    self.check_layers(second)?;
                }
            }
        }
        Ok(())
    }

    /// Uploads and places `layers` in the slots from `next` on, adding the work they
    /// need before drawing to `steps`. Returns their slots, bottom first.
    pub(super) fn prepare(
        &mut self,
        layers: &[LayerImage<'_>],
        canvas: (u32, u32),
        next: &mut usize,
        steps: &mut Vec<Step>,
    ) -> Vec<usize> {
        let mut placed = Vec::with_capacity(layers.len());
        for layer in layers {
            let index = match &layer.picture {
                Picture::Pixels {
                    width,
                    height,
                    rgba,
                    source,
                } => {
                    let index = self.claim(next, (*width, *height), SlotKind::Picture);
                    self.upload(index, rgba, source.as_ref());
                    index
                }
                Picture::Planes {
                    width,
                    height,
                    output,
                    luma,
                    chroma,
                    color,
                    format,
                    source,
                } => {
                    let kind = SlotKind::Planes(chroma.layout(), *format, (*width, *height));
                    let output = converted_size(*output, canvas, &layer.look);
                    let index = self.claim(next, output, kind);
                    if self.upload_planes(index, (*luma, chroma), *color, source.as_ref()) {
                        steps.push(Step::Convert(index));
                    }
                    index
                }
                Picture::Group(members) => self.group(members, canvas, next, steps),
                Picture::Matted {
                    fill,
                    matte,
                    channel,
                    reverse,
                } => {
                    let fill = self.prepare(fill, canvas, next, steps);
                    let matte = self.group(matte, canvas, next, steps);
                    let unchanged = LayerImage {
                        picture: Picture::Group(Vec::new()),
                        look: LayerLook::IDENTITY,
                        grade: None,
                    };
                    self.place(matte, &unchanged, canvas);
                    let index = self.claim(next, canvas, SlotKind::Group);
                    steps.push(Step::Matte {
                        index,
                        fill,
                        matte,
                        channel: *channel,
                        reverse: *reverse,
                    });
                    index
                }
                Picture::Combined {
                    layer,
                    other,
                    combine,
                } => {
                    let layer = self.group(layer, canvas, next, steps);
                    let other = self.group(other, canvas, next, steps);
                    let combining = LayerImage {
                        picture: Picture::Group(Vec::new()),
                        look: LayerLook {
                            effects: PixelEffects {
                                combine: Some(*combine),
                                ..PixelEffects::NONE
                            },
                            ..LayerLook::IDENTITY
                        },
                        grade: None,
                    };
                    self.place(layer, &combining, canvas);
                    let index = self.claim(next, canvas, SlotKind::Group);
                    steps.push(Step::Combine {
                        index,
                        layer,
                        other,
                    });
                    index
                }
            };
            self.place(index, layer, canvas);
            if self.slots[index].blurred {
                steps.push(Step::Blur(index));
            }
            placed.push(index);
        }
        placed
    }

    /// Prepares `members` to be drawn into a canvas-sized group slot, returned.
    fn group(
        &mut self,
        members: &[LayerImage<'_>],
        canvas: (u32, u32),
        next: &mut usize,
        steps: &mut Vec<Step>,
    ) -> usize {
        let members = self.prepare(members, canvas, next, steps);
        let index = self.claim(next, canvas, SlotKind::Group);
        steps.push(Step::Group { index, members });
        index
    }

    pub(super) fn check_size(&self, width: u32, height: u32) -> Result<(), RenderError> {
        let max = self.device.limits().max_texture_dimension_2d;
        if width == 0 || height == 0 || width > max || height > max {
            return Err(RenderError::FrameTooLarge { width, height, max });
        }
        Ok(())
    }
}

/// How many times smaller than the picture planes may be turned into RGB at.
const MAX_SHRINK: u32 = 8;

/// The size to turn planes meant for `output` into RGB at: halved for as long as the
/// layer keeps a pixel for every canvas pixel it covers, since conversion costs by the
/// pixel and detail finer than the canvas never shows. Effects that work in the
/// layer's own pixels, such as sharpening, keep it whole so they look the same.
fn converted_size(output: (u32, u32), canvas: (u32, u32), look: &LayerLook) -> (u32, u32) {
    let plain = PixelEffects {
        alpha: PixelEffects::NONE.alpha,
        color_space: PixelEffects::NONE.color_space,
        ..look.effects
    } == PixelEffects::NONE;
    if !plain {
        return output;
    }
    let shown = look.shown_size(output);
    let look = look.reframed(shown, canvas);
    let [across, down] = look.scale.map(f32::abs);
    // Canvas pixels per picture pixel, along the side stretched the most.
    let magnified = fit_scale(shown, canvas) * (across * look.pixel_aspect).max(down);
    let mut divisor = 1;
    while divisor < MAX_SHRINK && magnified * (divisor * 2) as f32 <= 1.0 {
        divisor *= 2;
    }
    let shrink = |side: u32| side.div_ceil(divisor).max(2);
    (shrink(output.0), shrink(output.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planes_convert_at_no_less_than_the_size_they_show_at() {
        let hd = (1920, 1080);
        let look = LayerLook::IDENTITY;
        assert_eq!(converted_size(hd, (1920, 1080), &look), hd);
        assert_eq!(converted_size(hd, (1280, 720), &look), hd);
        assert_eq!(converted_size(hd, (960, 540), &look), (960, 540));
        assert_eq!(converted_size(hd, (320, 240), &look), (480, 270));
        assert_eq!(converted_size((3840, 2160), (16, 9), &look), (480, 270));
        let zoomed = LayerLook {
            scale: [2.0, 2.0],
            ..look
        };
        assert_eq!(converted_size(hd, (960, 540), &zoomed), hd);
        let sharpened = LayerLook {
            effects: PixelEffects {
                sharpen: 1.0,
                ..PixelEffects::NONE
            },
            ..look
        };
        assert_eq!(converted_size(hd, (960, 540), &sharpened), hd);
    }
}
