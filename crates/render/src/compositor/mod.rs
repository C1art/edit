mod prepare;
mod slots;

use std::sync::Arc;

use timeline::MatteChannel;

use crate::backdrop::{Backdrop, Drawn};
use crate::blit::{self, Blit, LayerPass, DISPLAY_FORMAT};
use crate::combine::Combine;
use crate::grade::Grade;
use crate::layer_shader::Features;
use crate::placement::{LayerLook, Placement};
use crate::scope::{Scope, ScopeTrace};
use crate::{RenderError, WORKING_FORMAT};
use slots::Slot;

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
    /// `layer` composited as a group, combined with the group `other`.
    Combined {
        layer: Vec<LayerImage<'a>>,
        other: Vec<LayerImage<'a>>,
        combine: Combine,
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
    /// Draws the group slot `layer`, combined with the group slot `other`, into the
    /// group slot `index`.
    Combine {
        index: usize,
        layer: usize,
        other: usize,
    },
}

struct Targets {
    width: u32,
    height: u32,
    working_texture: wgpu::Texture,
    working: wgpu::TextureView,
    working_binding: wgpu::BindGroup,
    /// The effects the working frame's placement uses: none.
    working_features: Features,
    backdrop: Backdrop,
    output_texture: wgpu::Texture,
    output: wgpu::TextureView,
    ui_output: wgpu::TextureView,
}

/// What shows where no layer covers the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Background {
    #[default]
    Black,
    /// Kept clear, so the presented frame carries the layers' coverage as its alpha.
    Transparent,
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
    background: Background,
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
            background: Background::Black,
        }
    }

    /// The presented frame, valid until the next [`Presented::NewTarget`].
    pub fn output_view(&self) -> Option<&wgpu::TextureView> {
        self.targets.as_ref().map(|targets| &targets.ui_output)
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
        self.scope.as_ref().map(|trace| &trace.ui_view)
    }

    pub fn set_background(&mut self, background: Background) {
        self.background = background;
    }

    /// Draws `layers` bottom first over the background, each fitted to `canvas` and then moved,
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
                    let matte_slot = &self.slots[*matte];
                    let pass = LayerPass::Matte(*channel, *reverse);
                    blit::draw(
                        &mut encoder,
                        &group.view,
                        &self.blit.pipeline(matte_slot.features, pass),
                        &[[matte_slot.placed(), &self.blit.grade.none]],
                        wgpu::LoadOp::Load,
                    );
                }
                Step::Combine {
                    index,
                    layer,
                    other,
                } => {
                    let other = self
                        .blit
                        .bind_backdrop(&self.device, &self.slots[*other].view);
                    let layer = &self.slots[*layer];
                    let groups = [layer.placed(), &self.blit.grade.none, &other];
                    blit::draw(
                        &mut encoder,
                        &self.slots[*index].view,
                        &self.blit.pipeline(layer.features, LayerPass::Combined),
                        &[groups],
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    );
                }
            }
        }
        targets.backdrop.draw_layers(
            &mut encoder,
            &self.blit,
            (&targets.working_texture, &targets.working),
            &self.placed(&top),
            match self.background {
                Background::Black => wgpu::Color::BLACK,
                Background::Transparent => wgpu::Color::TRANSPARENT,
            },
        );
        blit::draw(
            &mut encoder,
            &targets.output,
            &self
                .blit
                .pipeline(targets.working_features, LayerPass::Display),
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
                    features: slot.features,
                }
            })
            .collect()
    }

    fn create_targets(&self, width: u32, height: u32) -> Targets {
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let working_texture = self.texture(
            "working_frame",
            (width, height),
            WORKING_FORMAT,
            usage | wgpu::TextureUsages::COPY_SRC,
        );
        let working_view = working_texture.create_view(&wgpu::TextureViewDescriptor::default());
        // Copyable so export can read the presented frame back.
        let output_texture = self.texture(
            "output_frame",
            (width, height),
            DISPLAY_FORMAT,
            usage | wgpu::TextureUsages::COPY_SRC,
        );
        let placement = Blit::placement_buffer(&self.device);
        // Layers blend into the working frame with their colors multiplied by their
        // alpha. Over black that alpha stays whole and dividing it out changes nothing.
        let working = Placement {
            premultiplied: true,
            ..Placement::FULL
        };
        let working_features = self.blit.write_placement(&self.queue, &placement, working);
        Targets {
            width,
            height,
            working_binding: self.blit.bind(&self.device, &working_view, &placement),
            working_features,
            working: working_view,
            working_texture,
            backdrop: Backdrop::new(&self.device, &self.blit, (width, height)),
            output: output_texture.create_view(&wgpu::TextureViewDescriptor::default()),
            ui_output: blit::ui_view(&output_texture),
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
            view_formats: if format == DISPLAY_FORMAT {
                blit::UI_VIEW_FORMATS
            } else {
                &[]
            },
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
