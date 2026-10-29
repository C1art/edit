//! Checking layers and preparing their slots and steps before a composite draws them.

use super::slots::SlotKind;
use super::{Compositor, LayerImage, Picture, Step};
use crate::pixel::PixelEffects;
use crate::placement::LayerLook;
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
                    luma,
                    chroma,
                    ..
                } => {
                    self.check_size(*width, *height)?;
                    let size = (*width, *height);
                    if !yuv::holds(*luma, *width, *height) || !yuv::chroma_holds(chroma, size) {
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
                    luma,
                    chroma,
                    color,
                    source,
                } => {
                    let kind = SlotKind::Planes(chroma.layout());
                    let index = self.claim(next, (*width, *height), kind);
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
