use crate::grade::GradeLayout;
use crate::placement::Placement;
use crate::WORKING_FORMAT;

/// The UI samples the output as an ordinary sRGB texture.
pub const DISPLAY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Sixteen `vec4<f32>`: the placement and color of a blit, or the parameters of a blur.
pub(crate) const UNIFORM_BYTES: u64 = 256;

/// The textured-rectangle pipelines and the layout their bindings share.
pub(crate) struct Blit {
    pub(crate) layout: wgpu::BindGroupLayout,
    /// The second bind group of layer and display blits; blurs have none.
    pub(crate) grade: GradeLayout,
    sampler: wgpu::Sampler,
    /// Blends layers over the working texture.
    pub(crate) to_working: wgpu::RenderPipeline,
    /// Copies the working texture to the display texture.
    pub(crate) to_display: wgpu::RenderPipeline,
    /// One direction of a Gaussian blur into a working-format texture.
    pub(crate) blur: wgpu::RenderPipeline,
}

impl Blit {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let blit_shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/blit.wgsl"));
        let blur_shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/blur.wgsl"));
        let fragment = wgpu::ShaderStages::FRAGMENT;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: fragment,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: fragment,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(UNIFORM_BYTES),
                    },
                    count: None,
                },
            ],
        });
        let grade = GradeLayout::new(device);
        let blit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit_pipeline_layout"),
            bind_group_layouts: &[Some(&layout), Some(&grade.layout)],
            immediate_size: 0,
        });
        let blur_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blur_pipeline_layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |shader: &wgpu::ShaderModule, pipeline_layout, format, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("blit_pipeline"),
                layout: Some(pipeline_layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            to_working: pipeline(
                &blit_shader,
                &blit_layout,
                WORKING_FORMAT,
                Some(wgpu::BlendState::ALPHA_BLENDING),
            ),
            to_display: pipeline(&blit_shader, &blit_layout, DISPLAY_FORMAT, None),
            blur: pipeline(&blur_shader, &blur_layout, WORKING_FORMAT, None),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("blit_sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            layout,
            grade,
        }
    }

    pub(crate) fn placement_buffer(device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("placement"),
            size: UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub(crate) fn write_placement(
        queue: &wgpu::Queue,
        buffer: &wgpu::Buffer,
        placement: Placement,
    ) {
        queue.write_buffer(buffer, 0, &placement.to_bytes());
    }

    pub(crate) fn bind(
        &self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        placement: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
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
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: placement.as_entire_binding(),
                },
            ],
        })
    }
}

/// Draws one quad per entry of `bindings`, each entry holding the bind groups of the
/// pipeline's layout in order.
pub(crate) fn draw<const GROUPS: usize>(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    bindings: &[[&wgpu::BindGroup; GROUPS]],
    clear: wgpu::Color,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("blit_pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    for groups in bindings {
        for (index, group) in (0..).zip(groups) {
            pass.set_bind_group(index, *group, &[]);
        }
        pass.draw(0..6, 0..1);
    }
}
