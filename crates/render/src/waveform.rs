use crate::compositor::extent;
use crate::DISPLAY_FORMAT;

/// Horizontal resolution of the scope; narrower frames use one column per pixel.
const MAX_COLUMNS: u32 = 512;
/// One level per 8-bit code value.
const LEVELS: u32 = 256;
const PARAMS_BYTES: u64 = 16;
const WORKGROUP: u32 = 16;

/// A luma waveform of the presented frame, measured and drawn on the GPU.
pub(crate) struct Waveform {
    bin: wgpu::ComputePipeline,
    bin_layout: wgpu::BindGroupLayout,
    draw: wgpu::RenderPipeline,
    draw_binding: wgpu::BindGroup,
    counts: wgpu::Buffer,
    params: wgpu::Buffer,
    /// The binding of the frame being measured and its size.
    source: Option<(wgpu::BindGroup, (u32, u32))>,
    pub(crate) view: wgpu::TextureView,
}

impl Waveform {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let counts = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("waveform_counts"),
            size: u64::from(MAX_COLUMNS * LEVELS) * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("waveform_params"),
            size: PARAMS_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let compute = wgpu::ShaderStages::COMPUTE;
        let fragment = wgpu::ShaderStages::FRAGMENT;
        let bin_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("waveform_bin_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: compute,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                storage_entry(1, compute, false),
                uniform_entry(2, compute),
            ],
        });
        let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("waveform_draw_layout"),
            entries: &[storage_entry(0, fragment, true), uniform_entry(1, fragment)],
        });
        let draw_binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("waveform_draw_binding"),
            layout: &draw_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: counts.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params.as_entire_binding(),
                },
            ],
        });
        let view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("waveform"),
                size: extent(MAX_COLUMNS, LEVELS),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DISPLAY_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            bin: bin_pipeline(device, &bin_layout),
            draw: draw_pipeline(device, &draw_layout),
            bin_layout,
            draw_binding,
            counts,
            params,
            source: None,
            view,
        }
    }

    /// Measures `frame` from now on; call whenever the presented texture is replaced.
    pub(crate) fn bind(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &wgpu::TextureView,
        (width, height): (u32, u32),
    ) {
        let columns = width.min(MAX_COLUMNS);
        let params: Vec<u8> = [width, height, columns, LEVELS]
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        queue.write_buffer(&self.params, 0, &params);
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("waveform_bin_binding"),
            layout: &self.bin_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(frame),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.counts.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params.as_entire_binding(),
                },
            ],
        });
        self.source = Some((binding, (width, height)));
    }

    pub(crate) fn record(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some((binding, (width, height))) = &self.source else {
            return;
        };
        encoder.clear_buffer(&self.counts, 0, None);
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("waveform_bin"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.bin);
            pass.set_bind_group(0, binding, &[]);
            pass.dispatch_workgroups(width.div_ceil(WORKGROUP), height.div_ceil(WORKGROUP), 1);
        }
        crate::blit::draw(encoder, &self.view, &self.draw, &[&self.draw_binding]);
    }
}

fn storage_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    read_only: bool,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(PARAMS_BYTES),
        },
        count: None,
    }
}

fn bin_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::ComputePipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/waveform_bin.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("waveform_bin_pipeline_layout"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("waveform_bin"),
        layout: Some(&layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}

fn draw_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/waveform_draw.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("waveform_draw_pipeline_layout"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("waveform_draw"),
        layout: Some(&layout),
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
                format: DISPLAY_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        multiview_mask: None,
        cache: None,
    })
}
