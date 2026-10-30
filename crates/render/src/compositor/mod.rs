mod picture;
mod prepare;
mod slots;

use std::sync::Arc;

use timeline::MatteChannel;

use crate::backdrop::{Backdrop, Drawn};
use crate::blit::{self, Blit, DISPLAY_FORMAT};
use crate::grade::Grade;
use crate::layer_pipelines::LayerPass;
use crate::layer_shader::Features;
use crate::placement::{LayerLook, Placement};
use crate::scope::{Scope, ScopeTrace};
use crate::yuv::YuvConverter;
use crate::{RenderError, WORKING_FORMAT};
pub use picture::Picture;
use slots::Slot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presented {
    /// The frame went into the existing output texture.
    SameTarget,
    /// The canvas size changed, so the output texture was recreated and must be rebound.
    NewTarget,
    /// Nothing was drawn, as a shader the frame needs is still being built in the
    /// background; the last frame stays. Present again once [`Compositor::compiling`]
    /// is false.
    Pending,
}

/// A picture to composite and how to draw it.
#[derive(Debug, Clone)]
pub struct LayerImage<'a> {
    pub picture: Picture<'a>,
    pub look: LayerLook,
    /// Shared between frames, so its textures are only uploaded when it changes.
    pub grade: Option<Arc<Grade>>,
}

/// GPU work that must run before the layers that use it are drawn.
enum Step {
    /// Turns the planes uploaded for the slot into its texture.
    Convert(usize),
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
    /// Kept clear, with the presented colors multiplied by their alpha, as a UI blends a
    /// picture over what it draws beneath.
    UnderUi,
}

/// Uploads source frames, composites them in linear light and presents the result to a
/// texture the UI can draw.
pub struct Compositor {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    blit: Blit,
    slots: Vec<Slot>,
    yuv: YuvConverter,
    targets: Option<Targets>,
    scope: Option<ScopeTrace>,
    background: Background,
    /// Whether the output texture was recreated for a frame left pending, so the next
    /// frame shown must report it.
    unshown_target: bool,
    #[cfg(windows)]
    shared: crate::shared::SharedTextures,
    /// Decoded pictures replaced since the last submit, kept until the GPU is done
    /// reading them so their decoder can't write over them first.
    #[cfg(windows)]
    retired: Vec<Arc<dyn std::any::Any + Send + Sync>>,
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
            yuv: YuvConverter::new(device),
            targets: None,
            scope: None,
            background: Background::Black,
            unshown_target: false,
            #[cfg(windows)]
            shared: crate::shared::SharedTextures::default(),
            #[cfg(windows)]
            retired: Vec::new(),
        }
    }

    /// Builds the shaders of effects not used before on a worker from now on, so
    /// compositing never waits for the driver's compiler; frames that need one still
    /// being built come back [`Presented::Pending`]. For compositors on the UI thread.
    pub fn compile_in_background(&mut self) {
        self.blit.layers.build_in_background();
        self.yuv.build_in_background();
        // What a plain frame needs, so the first one is ready as soon as possible.
        let working = Placement {
            premultiplied: true,
            ..Placement::FULL
        };
        let working = self.blit.features(&working.to_bytes());
        self.blit.pipeline(working, LayerPass::Display);
        self.blit.pipeline(Features::NONE, LayerPass::Over);
    }

    /// Whether shaders asked for are still being built in the background.
    pub fn compiling(&self) -> bool {
        self.blit.layers.building() || self.yuv.building()
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
        if (background == Background::UnderUi) != (self.background == Background::UnderUi) {
            // The frame is presented premultiplied or not by how its targets are made.
            self.targets = None;
        }
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
        self.blit.layers.take_missed();
        let mut steps = Vec::new();
        let top = self.prepare(layers, canvas, &mut 0, &mut steps);
        let Some(targets) = &self.targets else {
            unreachable!("targets were created above");
        };
        if let (Presented::NewTarget, Some(trace)) = (presented, &mut self.scope) {
            trace.bind(&self.device, &self.queue, &targets.output, canvas);
        }

        let encoder_named = |label| {
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) })
        };
        // Submitted even when the frame is left pending, as the planes it converts
        // aren't uploaded again.
        let mut conversions = encoder_named("yuv");
        let mut encoder = encoder_named("compositor");
        let mut unconverted = false;
        for step in &steps {
            match step {
                Step::Convert(index) => {
                    let slot = &mut self.slots[*index];
                    if let Some(planes) = &slot.planes {
                        if !planes.draw(&mut conversions, &self.yuv, &slot.view) {
                            // Uploaded again next time, when the converter is built.
                            slot.uploaded = None;
                            unconverted = true;
                        }
                    }
                }
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
                    if let Some(pipeline) = self.blit.pipeline(matte_slot.features, pass) {
                        blit::draw(
                            &mut encoder,
                            &group.view,
                            &pipeline,
                            &[[matte_slot.placed(), &self.blit.grade.none]],
                            wgpu::LoadOp::Load,
                        );
                    }
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
                    if let Some(pipeline) = self.blit.pipeline(layer.features, LayerPass::Combined)
                    {
                        blit::draw(
                            &mut encoder,
                            &self.slots[*index].view,
                            &pipeline,
                            &[groups],
                            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        );
                    }
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
                Background::Transparent | Background::UnderUi => wgpu::Color::TRANSPARENT,
            },
        );
        let display = self
            .blit
            .pipeline(targets.working_features, LayerPass::Display);
        if let Some(display) = &display {
            blit::draw(
                &mut encoder,
                &targets.output,
                display,
                &[[&targets.working_binding, &self.blit.grade.none]],
                wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            );
        }
        // The encoder is dropped unsubmitted: a frame with layers left out isn't shown.
        if self.blit.layers.take_missed() | unconverted {
            self.submit([conversions.finish()]);
            self.unshown_target |= presented == Presented::NewTarget;
            return Ok(Presented::Pending);
        }
        if let Some(trace) = &self.scope {
            trace.record(&mut encoder);
        }
        self.submit([conversions.finish(), encoder.finish()]);
        if std::mem::take(&mut self.unshown_target) {
            return Ok(Presented::NewTarget);
        }
        Ok(presented)
    }

    fn submit<const N: usize>(&mut self, buffers: [wgpu::CommandBuffer; N]) {
        self.queue.submit(buffers);
        #[cfg(windows)]
        {
            self.shared.finish_composite();
            let retired = std::mem::take(&mut self.retired);
            if !retired.is_empty() {
                self.queue.on_submitted_work_done(move || drop(retired));
            }
        }
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
                    reach: slot.reach,
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
            premultiplied: self.background != Background::UnderUi,
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
