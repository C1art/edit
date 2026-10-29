use timeline::MatteChannel;

use crate::grade::GradeLayout;
use crate::placement::Placement;
use crate::WORKING_FORMAT;

/// Presenting to an sRGB texture encodes the linear composite for display.
pub const DISPLAY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// egui samples textures as already gamma-encoded and does no conversion of its own, so
/// it reads display textures through a view that skips the sRGB decode.
pub(crate) const UI_VIEW_FORMATS: &[wgpu::TextureFormat] = &[wgpu::TextureFormat::Rgba8Unorm];

pub(crate) fn ui_view(texture: &wgpu::Texture) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor {
        format: UI_VIEW_FORMATS.first().copied(),
        ..Default::default()
    })
}

/// Room for the placement, color and masks of a blit, or the parameters of a blur.
pub(crate) const UNIFORM_BYTES: u64 = 3_664;

/// The textured-rectangle pipelines and the layout their bindings share.
pub(crate) struct Blit {
    pub(crate) layout: wgpu::BindGroupLayout,
    /// The second bind group of layer and display blits; blurs have none.
    pub(crate) grade: GradeLayout,
    sampler: wgpu::Sampler,
    /// Blends layers over the working texture.
    pub(crate) to_working: wgpu::RenderPipeline,
    /// The third bind group of blend-mode blits: a copy of what lies under the layer.
    pub(crate) backdrop_layout: wgpu::BindGroupLayout,
    /// Draws a layer over the working texture by its blend mode.
    pub(crate) blended: wgpu::RenderPipeline,
    /// Keeps the part of a fill its track matte shows, by channel then reversed.
    mattes: [[wgpu::RenderPipeline; 2]; 2],
    /// Copies the working texture to the display texture.
    pub(crate) to_display: wgpu::RenderPipeline,
    /// One direction of a Gaussian blur into a working-format texture.
    pub(crate) blur: wgpu::RenderPipeline,
}

impl Blit {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                concat!(
                    include_str!("../shaders/blit.wgsl"),
                    include_str!("../shaders/blend.wgsl"),
                    include_str!("../shaders/style.wgsl"),
                    include_str!("../shaders/reshape.wgsl"),
                    include_str!("../shaders/recolor.wgsl"),
                    include_str!("../shaders/key.wgsl"),
                    include_str!("../shaders/matte.wgsl"),
                    include_str!("../shaders/adjust.wgsl"),
                    include_str!("../shaders/mask.wgsl"),
                    include_str!("../shaders/secondary.wgsl"),
                    include_str!("../shaders/lumetri.wgsl"),
                    include_str!("../shaders/generate.wgsl"),
                    include_str!("../shaders/optics.wgsl"),
                    include_str!("../shaders/clip_wipes.wgsl"),
                    include_str!("../shaders/wipe.wgsl")
                )
                .into(),
            ),
        });
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
        let backdrop_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("backdrop_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: fragment,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let blended_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blended_pipeline_layout"),
            bind_group_layouts: &[Some(&layout), Some(&grade.layout), Some(&backdrop_layout)],
            immediate_size: 0,
        });
        let blur_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blur_pipeline_layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |shader: &wgpu::ShaderModule, pipeline_layout, format, blend, fragment| {
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
                    entry_point: Some(fragment),
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
        let over = Some(wgpu::BlendState::ALPHA_BLENDING);
        let matte = |fragment, reverse| {
            let keep = wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: if reverse {
                    wgpu::BlendFactor::OneMinusSrcAlpha
                } else {
                    wgpu::BlendFactor::SrcAlpha
                },
                operation: wgpu::BlendOperation::Add,
            };
            let blend = wgpu::BlendState {
                color: keep,
                alpha: keep,
            };
            pipeline(
                &blit_shader,
                &blit_layout,
                WORKING_FORMAT,
                Some(blend),
                fragment,
            )
        };
        let mattes = ["fs_matte_alpha", "fs_matte_luma"]
            .map(|entry| [false, true].map(|reverse| matte(entry, reverse)));
        Self {
            to_working: pipeline(&blit_shader, &blit_layout, WORKING_FORMAT, over, "fs_main"),
            blended: pipeline(
                &blit_shader,
                &blended_layout,
                WORKING_FORMAT,
                over,
                "fs_blend",
            ),
            backdrop_layout,
            mattes,
            to_display: pipeline(&blit_shader, &blit_layout, DISPLAY_FORMAT, None, "fs_main"),
            blur: pipeline(&blur_shader, &blur_layout, WORKING_FORMAT, None, "fs_main"),
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

    pub(crate) fn matte(&self, channel: MatteChannel, reverse: bool) -> &wgpu::RenderPipeline {
        &self.mattes[channel as usize][usize::from(reverse)]
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
    load: wgpu::LoadOp<wgpu::Color>,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("blit_pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
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
