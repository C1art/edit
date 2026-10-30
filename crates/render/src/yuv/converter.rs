use std::cell::RefCell;

use crossbeam::channel::{self, Receiver, TryRecvError};
use timeline::{PlaneFormat, SampleDepth, YuvColor};

use super::uniform::{uniform, UNIFORM_BYTES};
use super::{ChromaLayout, ChromaRows, PlaneRows};
use crate::blit::{self, render_pipeline};
use crate::compositor::extent;
use crate::WORKING_FORMAT;

/// How the planes' textures hold their samples, which picks the shader that reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Samples {
    /// Read as 0 to 1 by the texture.
    Bytes,
    /// Read as integers, as 16-bit normalized textures aren't everywhere.
    Words,
}

impl Samples {
    const ALL: [Self; 2] = [Self::Bytes, Self::Words];

    fn of(depth: SampleDepth) -> Self {
        match depth {
            SampleDepth::Eight => Self::Bytes,
            SampleDepth::Ten | SampleDepth::Twelve | SampleDepth::Sixteen => Self::Words,
        }
    }

    /// The texture formats of planes of one sample a texel, and of two.
    fn formats(self) -> (wgpu::TextureFormat, wgpu::TextureFormat) {
        match self {
            Self::Bytes => (wgpu::TextureFormat::R8Unorm, wgpu::TextureFormat::Rg8Unorm),
            Self::Words => (wgpu::TextureFormat::R16Uint, wgpu::TextureFormat::Rg16Uint),
        }
    }
}

/// The pipelines that turn planes into linear RGB, one for each kind of sample. Built
/// where first needed, or all at once on a worker so the thread drawing doesn't wait
/// on the driver's shader compiler.
pub(crate) struct YuvConverter {
    device: wgpu::Device,
    layouts: [wgpu::BindGroupLayout; 2],
    pipelines: RefCell<[Option<wgpu::RenderPipeline>; 2]>,
    building: Option<Receiver<(Samples, wgpu::RenderPipeline)>>,
}

impl YuvConverter {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        Self {
            device: device.clone(),
            layouts: Samples::ALL.map(|samples| bind_group_layout(device, samples)),
            pipelines: RefCell::default(),
            building: None,
        }
    }

    pub(crate) fn build_in_background(&mut self) {
        if self.building.is_some() {
            return;
        }
        let (done, building) = channel::bounded(Samples::ALL.len());
        let device = self.device.clone();
        let layouts = self.layouts.clone();
        let spawned = std::thread::Builder::new()
            .name("yuv shader compiler".into())
            .spawn(move || {
                for samples in Samples::ALL {
                    let pipeline = pipeline(&device, &layouts[samples as usize], samples);
                    if done.send((samples, pipeline)).is_err() {
                        break;
                    }
                }
            });
        if spawned.is_ok() {
            self.building = Some(building);
        }
    }

    /// Whether the worker is still building.
    pub(crate) fn building(&self) -> bool {
        self.collect() && self.pipelines.borrow().iter().any(Option::is_none)
    }

    /// Built here unless the worker is building it, then `None` until it's done.
    fn pipeline(&self, samples: Samples) -> Option<wgpu::RenderPipeline> {
        let worker_alive = self.collect();
        let index = samples as usize;
        if let Some(pipeline) = &self.pipelines.borrow()[index] {
            return Some(pipeline.clone());
        }
        if worker_alive {
            return None;
        }
        let built = pipeline(&self.device, &self.layouts[index], samples);
        self.pipelines.borrow_mut()[index] = Some(built.clone());
        Some(built)
    }

    /// Takes in what the worker has finished; false once it's gone or there's none.
    fn collect(&self) -> bool {
        let Some(building) = &self.building else {
            return false;
        };
        let mut pipelines = self.pipelines.borrow_mut();
        loop {
            match building.try_recv() {
                Ok((samples, pipeline)) => pipelines[samples as usize] = Some(pipeline),
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => return false,
            }
        }
    }
}

fn bind_group_layout(device: &wgpu::Device, samples: Samples) -> wgpu::BindGroupLayout {
    let sample_type = match samples {
        Samples::Bytes => wgpu::TextureSampleType::Float { filterable: false },
        Samples::Words => wgpu::TextureSampleType::Uint,
    };
    let texture = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("yuv"),
        entries: &[
            texture(0),
            texture(1),
            texture(2),
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    samples: Samples,
) -> wgpu::RenderPipeline {
    let prelude = match samples {
        Samples::Bytes => include_str!("../../shaders/yuv_bytes.wgsl"),
        Samples::Words => include_str!("../../shaders/yuv_words.wgsl"),
    };
    let source = format!("{prelude}\n{}", include_str!("../../shaders/yuv.wgsl"));
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("yuv.wgsl"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("yuv"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    render_pipeline(
        device,
        &shader,
        &pipeline_layout,
        (WORKING_FORMAT, None),
        "fs_main",
    )
}

/// The plane textures of one layer and what converts them into its texture.
pub(crate) struct PlaneTextures {
    samples: Samples,
    format: PlaneFormat,
    /// The luma and chroma textures planes are uploaded to; none for a decoder's
    /// texture, bound as each picture arrives.
    uploaded: Option<(wgpu::Texture, Vec<wgpu::Texture>)>,
    uniform: wgpu::Buffer,
    /// `None` while there's no picture to read, which converts to nothing.
    binding: Option<wgpu::BindGroup>,
}

impl PlaneTextures {
    pub(crate) fn new(
        device: &wgpu::Device,
        converter: &YuvConverter,
        size: (u32, u32),
        (layout, format): (ChromaLayout, PlaneFormat),
    ) -> Self {
        let texture = |label, (width, height), format| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent(width, height),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };
        let samples = Samples::of(format.depth);
        let (single, pair) = samples.formats();
        let luma = texture("luma", size, single);
        let chroma_size = format.subsampling.chroma_size(size);
        let chroma = match layout {
            ChromaLayout::Separate => vec![
                texture("cb", chroma_size, single),
                texture("cr", chroma_size, single),
            ],
            ChromaLayout::Interleaved => vec![texture("cbcr", chroma_size, pair)],
        };
        let view = |texture: &wgpu::Texture| texture.create_view(&Default::default());
        let luma_view = view(&luma);
        let cb = view(&chroma[0]);
        let cr = view(chroma.last().unwrap_or(&chroma[0]));
        let uniform = uniform_buffer(device);
        let binding = bind(device, converter, samples, [&luma_view, &cb, &cr], &uniform);
        Self {
            samples,
            format,
            uploaded: Some((luma, chroma)),
            uniform,
            binding: Some(binding),
        }
    }

    /// For a decoder's NV12 or P010 texture, whose planes read as 0 to 1.
    #[cfg(windows)]
    pub(crate) fn shared(device: &wgpu::Device, format: PlaneFormat) -> Self {
        Self {
            samples: Samples::Bytes,
            format,
            uploaded: None,
            uniform: uniform_buffer(device),
            binding: None,
        }
    }

    /// Reads the next picture from the planes of a decoder's texture, holding a picture
    /// of `size` at its top left, or reads nothing when it couldn't be opened.
    #[cfg(windows)]
    pub(crate) fn bind_shared(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        converter: &YuvConverter,
        planes: Option<(&wgpu::TextureView, &wgpu::TextureView)>,
        color: YuvColor,
        (size, output): ((u32, u32), (u32, u32)),
    ) {
        self.binding = planes.map(|(luma, chroma)| {
            bind(
                device,
                converter,
                self.samples,
                [luma, chroma, chroma],
                &self.uniform,
            )
        });
        let uniform = uniform(
            color,
            self.format,
            (size, output),
            (ChromaLayout::Interleaved, true),
        );
        queue.write_buffer(&self.uniform, 0, &uniform);
    }

    /// Uploads the planes, to be converted into a texture of `output` size.
    pub(crate) fn upload(
        &self,
        queue: &wgpu::Queue,
        (luma, chroma): (PlaneRows<'_>, &ChromaRows<'_>),
        color: YuvColor,
        output: (u32, u32),
    ) {
        let write = |texture: &wgpu::Texture, plane: PlaneRows<'_>| {
            let size = texture.size();
            queue.write_texture(
                texture.as_image_copy(),
                plane.bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(plane.stride),
                    rows_per_image: Some(size.height),
                },
                size,
            );
        };
        let Some((luma_texture, chroma_textures)) = &self.uploaded else {
            return;
        };
        write(luma_texture, luma);
        for (texture, (plane, _)) in chroma_textures.iter().zip(chroma.planes()) {
            write(texture, plane);
        }
        let luma_size = luma_texture.size();
        let size = (luma_size.width, luma_size.height);
        let normalized = self.samples == Samples::Bytes;
        let uniform = uniform(
            color,
            self.format,
            (size, output),
            (chroma.layout(), normalized),
        );
        queue.write_buffer(&self.uniform, 0, &uniform);
    }

    /// Records the conversion; false, recording nothing, while its pipeline is still
    /// being built.
    pub(crate) fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        converter: &YuvConverter,
        target: &wgpu::TextureView,
    ) -> bool {
        let Some(binding) = &self.binding else {
            clear(encoder, target);
            return true;
        };
        let Some(pipeline) = converter.pipeline(self.samples) else {
            return false;
        };
        blit::draw(
            encoder,
            target,
            &pipeline,
            &[[binding]],
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
        );
        true
    }
}

fn uniform_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("yuv"),
        size: UNIFORM_BYTES as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Binds the luma, blue- and red-difference planes, the last two the same texture
/// when interleaved.
fn bind(
    device: &wgpu::Device,
    converter: &YuvConverter,
    samples: Samples,
    [luma, cb, cr]: [&wgpu::TextureView; 3],
    uniform: &wgpu::Buffer,
) -> wgpu::BindGroup {
    let entry = |binding, resource| wgpu::BindGroupEntry { binding, resource };
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("yuv"),
        layout: &converter.layouts[samples as usize],
        entries: &[
            entry(0, wgpu::BindingResource::TextureView(luma)),
            entry(1, wgpu::BindingResource::TextureView(cb)),
            entry(2, wgpu::BindingResource::TextureView(cr)),
            entry(3, uniform.as_entire_binding()),
        ],
    })
}

fn clear(encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("yuv clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
}
