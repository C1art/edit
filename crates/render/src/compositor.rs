use crate::{RenderError, WORKING_FORMAT};

/// The UI samples the output as an ordinary sRGB texture.
pub const DISPLAY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Decoded frames arrive gamma-encoded. BT.709 video is treated as sRGB, which matches
/// the display-referred look of other editors on an sRGB monitor.
const SOURCE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presented {
    /// The frame went into the existing output texture.
    SameTarget,
    /// The frame size changed, so the output texture was recreated and must be rebound.
    NewTarget,
}

struct Targets {
    width: u32,
    height: u32,
    source: wgpu::Texture,
    source_binding: wgpu::BindGroup,
    working: wgpu::TextureView,
    working_binding: wgpu::BindGroup,
    output: wgpu::TextureView,
}

/// Uploads source frames, composites them in linear light and presents the result to a
/// texture the UI can draw.
pub struct Compositor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    to_working: wgpu::RenderPipeline,
    to_display: wgpu::RenderPipeline,
    targets: Option<Targets>,
}

impl Compositor {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/blit.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit_pipeline_layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |format| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("blit_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            to_working: pipeline(WORKING_FORMAT),
            to_display: pipeline(DISPLAY_FORMAT),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("blit_sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            device: device.clone(),
            queue: queue.clone(),
            layout,
            targets: None,
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

    /// Takes tightly packed RGBA8 pixels in the source transfer function.
    pub fn present_frame(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<Presented, RenderError> {
        let expected = width as usize * height as usize * 4;
        if rgba.len() != expected || width == 0 || height == 0 {
            return Err(RenderError::FrameSize {
                width,
                height,
                bytes: rgba.len(),
            });
        }
        let max = self.device.limits().max_texture_dimension_2d;
        if width > max || height > max {
            return Err(RenderError::FrameTooLarge { width, height, max });
        }

        let presented = match &self.targets {
            Some(targets) if (targets.width, targets.height) == (width, height) => {
                Presented::SameTarget
            }
            _ => {
                self.targets = Some(self.create_targets(width, height));
                Presented::NewTarget
            }
        };
        let Some(targets) = &self.targets else {
            unreachable!("targets were created above");
        };

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &targets.source,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            extent(width, height),
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("compositor"),
            });
        blit(
            &mut encoder,
            &self.to_working,
            &targets.source_binding,
            &targets.working,
        );
        blit(
            &mut encoder,
            &self.to_display,
            &targets.working_binding,
            &targets.output,
        );
        self.queue.submit([encoder.finish()]);
        Ok(presented)
    }

    fn create_targets(&self, width: u32, height: u32) -> Targets {
        let texture = |label, format, usage| {
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
        };
        let sampled_target =
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let source = texture(
            "source_frame",
            SOURCE_FORMAT,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let working = texture("working_frame", WORKING_FORMAT, sampled_target)
            .create_view(&wgpu::TextureViewDescriptor::default());
        let output = texture("output_frame", DISPLAY_FORMAT, sampled_target)
            .create_view(&wgpu::TextureViewDescriptor::default());
        let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
        Targets {
            width,
            height,
            source_binding: self.bind(&source_view),
            working_binding: self.bind(&working),
            source,
            working,
            output,
        }
    }

    fn bind(&self, view: &wgpu::TextureView) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit_bind_group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

fn blit(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    source: &wgpu::BindGroup,
    target: &wgpu::TextureView,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("blit_pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, source, &[]);
    pass.draw(0..3, 0..1);
}
