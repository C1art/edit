mod slots;

use std::sync::Arc;

use timeline::MatteChannel;

use crate::backdrop::{Backdrop, Drawn};
use crate::blit::{self, Blit, DISPLAY_FORMAT};
use crate::grade::Grade;
use crate::placement::{LayerLook, Placement};
use crate::scope::{Scope, ScopeTrace};
use crate::{RenderError, WORKING_FORMAT};
use slots::{Slot, SlotKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presented {
    /// The frame went into the existing output texture.
    SameTarget,
    /// The canvas size changed, so the output texture was recreated and must be rebound.
    NewTarget,
}

/// A picture to composite and how to draw it.
#[derive(Debug, Clone)]
pub struct LayerImage<'a> {
    pub picture: Picture<'a>,
    pub look: LayerLook,
    /// Shared between frames, so its textures are only uploaded when it changes.
    pub grade: Option<Arc<Grade>>,
}

#[derive(Debug, Clone)]
pub enum Picture<'a> {
    /// Tightly packed RGBA8 in the source transfer function.
    Pixels {
        width: u32,
        height: u32,
        rgba: &'a [u8],
    },
    /// Layers composited bottom first onto a transparent canvas of their own, so the
    /// look applies to them as one picture, as to a nested sequence.
    Group(Vec<LayerImage<'a>>),
    /// `fill` composited as a group, shown only where the group `matte` lets it.
    Matted {
        fill: Vec<LayerImage<'a>>,
        matte: Vec<LayerImage<'a>>,
        channel: MatteChannel,
        reverse: bool,
    },
}

/// GPU work that must run before the layers that use it are drawn.
enum Step {
    Blur(usize),
    /// Draws the slots in `members` into the group slot `index`.
    Group {
        index: usize,
        members: Vec<usize>,
    },
    /// Draws the slots in `fill` into the group slot `index`, then keeps the part of
    /// them the group slot `matte` shows.
    Matte {
        index: usize,
        fill: Vec<usize>,
        matte: usize,
        channel: MatteChannel,
        reverse: bool,
    },
}

struct Targets {
    width: u32,
    height: u32,
    working_texture: wgpu::Texture,
    working: wgpu::TextureView,
    working_binding: wgpu::BindGroup,
    backdrop: Backdrop,
    output_texture: wgpu::Texture,
    output: wgpu::TextureView,
}

/// Uploads source frames, composites them in linear light and presents the result to a
/// texture the UI can draw.
pub struct Compositor {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    blit: Blit,
    slots: Vec<Slot>,
    targets: Option<Targets>,
    scope: Option<ScopeTrace>,
}

impl Compositor {
    pub(crate) fn output_texture(&self) -> Option<&wgpu::Texture> {
        self.targets.as_ref().map(|targets| &targets.output_texture)
    }
}

impl Compositor {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            blit: Blit::new(device),
            device: device.clone(),
            queue: queue.clone(),
            slots: Vec::new(),
            targets: None,
            scope: None,
        }
    }

    /// The presented frame, valid until the next [`Presented::NewTarget`].
    pub fn output_view(&self) -> Option<&wgpu::TextureView> {
        self.targets.as_ref().map(|targets| &targets.output)
    }

    pub fn output_size(&self) -> Option<(u32, u32)> {
        self.targets
            .as_ref()
            .map(|targets| (targets.width, targets.height))
    }

    /// Measures every presented frame with `scope` while one is set, starting with the
    /// one already shown.
    pub fn set_scope(&mut self, scope: Option<Scope>) {
        let Some(scope) = scope else {
            self.scope = None;
            return;
        };
        let trace = match &mut self.scope {
            Some(trace) if trace.scope() == scope => return,
            Some(trace) => {
                trace.set_scope(&self.queue, scope);
                trace
            }
            None => {
                let mut trace = ScopeTrace::new(&self.device, scope);
                if let Some(targets) = &self.targets {
                    let size = (targets.width, targets.height);
                    trace.bind(&self.device, &self.queue, &targets.output, size);
                }
                self.scope.insert(trace)
            }
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("scope"),
            });
        trace.record(&mut encoder);
        self.queue.submit([encoder.finish()]);
    }

    /// The scope's square texture. The view lasts until the scope is turned off.
    pub fn scope_view(&self) -> Option<&wgpu::TextureView> {
        self.scope.as_ref().map(|trace| &trace.view)
    }

    /// Draws `layers` bottom first over black, each fitted to `canvas` and then moved,
    /// scaled, turned, cropped, blurred and faded by its look.
    pub fn composite(
        &mut self,
        canvas: (u32, u32),
        layers: &[LayerImage<'_>],
    ) -> Result<Presented, RenderError> {
        self.check_size(canvas.0, canvas.1)?;
        self.check_layers(layers)?;

        let presented = match &self.targets {
            Some(targets) if (targets.width, targets.height) == canvas => Presented::SameTarget,
            _ => {
                self.targets = Some(self.create_targets(canvas.0, canvas.1));
                Presented::NewTarget
            }
        };
        let mut steps = Vec::new();
        let top = self.prepare(layers, canvas, &mut 0, &mut steps);
        let Some(targets) = &self.targets else {
            unreachable!("targets were created above");
        };
        if let (Presented::NewTarget, Some(trace)) = (presented, &mut self.scope) {
            trace.bind(&self.device, &self.queue, &targets.output, canvas);
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("compositor"),
            });
        for step in &steps {
            match step {
                Step::Blur(index) => {
                    if let Some(blur) = &self.slots[*index].blur {
                        blur.draw(&mut encoder, &self.blit);
                    }
                }
                Step::Group { index, members } => {
                    let group = &self.slots[*index];
                    targets.backdrop.draw_layers(
                        &mut encoder,
                        &self.blit,
                        (&group.texture, &group.view),
                        &self.placed(members),
                        wgpu::Color::TRANSPARENT,
                    );
                }
                Step::Matte {
                    index,
                    fill,
                    matte,
                    channel,
                    reverse,
                } => {
                    let group = &self.slots[*index];
                    targets.backdrop.draw_layers(
                        &mut encoder,
                        &self.blit,
                        (&group.texture, &group.view),
                        &self.placed(fill),
                        wgpu::Color::TRANSPARENT,
                    );
                    blit::draw(
                        &mut encoder,
                        &group.view,
                        self.blit.matte(*channel, *reverse),
                        &[[self.slots[*matte].placed(), &self.blit.grade.none]],
                        wgpu::LoadOp::Load,
                    );
                }
            }
        }
        targets.backdrop.draw_layers(
            &mut encoder,
            &self.blit,
            (&targets.working_texture, &targets.working),
            &self.placed(&top),
            wgpu::Color::BLACK,
        );
        blit::draw(
            &mut encoder,
            &targets.output,
            &self.blit.to_display,
            &[[&targets.working_binding, &self.blit.grade.none]],
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        );
        if let Some(trace) = &self.scope {
            trace.record(&mut encoder);
        }
        self.queue.submit([encoder.finish()]);
        Ok(presented)
    }

    fn placed(&self, slots: &[usize]) -> Vec<Drawn<'_>> {
        slots
            .iter()
            .map(|&index| {
                let slot = &self.slots[index];
                let grade = slot
                    .grade
                    .as_ref()
                    .map_or(&self.blit.grade.none, |(_, binding)| binding);
                Drawn {
                    groups: [slot.placed(), grade],
                    blend: slot.blend,
                }
            })
            .collect()
    }

    fn check_layers(&self, layers: &[LayerImage<'_>]) -> Result<(), RenderError> {
        for layer in layers {
            match &layer.picture {
                Picture::Pixels {
                    width,
                    height,
                    rgba,
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
                Picture::Group(members) => self.check_layers(members)?,
                Picture::Matted { fill, matte, .. } => {
                    self.check_layers(fill)?;
                    self.check_layers(matte)?;
                }
            }
        }
        Ok(())
    }

    /// Uploads and places `layers` in the slots from `next` on, adding the work they
    /// need before drawing to `steps`. Returns their slots, bottom first.
    fn prepare(
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
                } => {
                    let index = self.claim(next, (*width, *height), SlotKind::Picture);
                    self.upload(index, rgba);
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

    fn check_size(&self, width: u32, height: u32) -> Result<(), RenderError> {
        let max = self.device.limits().max_texture_dimension_2d;
        if width == 0 || height == 0 || width > max || height > max {
            return Err(RenderError::FrameTooLarge { width, height, max });
        }
        Ok(())
    }

    fn create_targets(&self, width: u32, height: u32) -> Targets {
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let working_texture = self.texture(
            "working_frame",
            (width, height),
            WORKING_FORMAT,
            usage | wgpu::TextureUsages::COPY_SRC,
        );
        let working = working_texture.create_view(&wgpu::TextureViewDescriptor::default());
        // Copyable so export can read the presented frame back.
        let output_texture = self.texture(
            "output_frame",
            (width, height),
            DISPLAY_FORMAT,
            usage | wgpu::TextureUsages::COPY_SRC,
        );
        let placement = Blit::placement_buffer(&self.device);
        Blit::write_placement(&self.queue, &placement, Placement::FULL);
        Targets {
            width,
            height,
            working_binding: self.blit.bind(&self.device, &working, &placement),
            working,
            working_texture,
            backdrop: Backdrop::new(&self.device, &self.blit, (width, height)),
            output: output_texture.create_view(&wgpu::TextureViewDescriptor::default()),
            output_texture,
        }
    }

    fn texture(
        &self,
        label: &str,
        (width, height): (u32, u32),
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: extent(width, height),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    }
}

pub(crate) fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}
