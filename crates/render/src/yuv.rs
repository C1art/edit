use timeline::{ChromaSiting, YuvColor, YuvMatrix, YuvRange};

use crate::blit::{self, render_pipeline};
use crate::compositor::extent;
use crate::WORKING_FORMAT;

/// One plane's rows, `stride` bytes apart.
#[derive(Debug, Clone, Copy)]
pub struct PlaneRows<'a> {
    pub bytes: &'a [u8],
    pub stride: u32,
}

/// A 4:2:0 picture's chroma, at half its width and height, rounded up.
#[derive(Debug, Clone, Copy)]
pub enum ChromaRows<'a> {
    /// The blue-difference plane, then the red-difference plane.
    Separate(PlaneRows<'a>, PlaneRows<'a>),
    /// Blue- and red-difference pairs in one plane, as NV12 lays them out.
    Interleaved(PlaneRows<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChromaLayout {
    Separate,
    Interleaved,
}

impl ChromaRows<'_> {
    pub(crate) fn layout(&self) -> ChromaLayout {
        match self {
            Self::Separate(..) => ChromaLayout::Separate,
            Self::Interleaved(_) => ChromaLayout::Interleaved,
        }
    }

    /// Each plane with its bytes per sample.
    fn planes(&self) -> impl Iterator<Item = (PlaneRows<'_>, u32)> {
        let (first, second, samples) = match *self {
            Self::Separate(cb, cr) => (cb, Some(cr), 1),
            Self::Interleaved(both) => (both, None, 2),
        };
        std::iter::once((first, samples)).chain(second.map(|cr| (cr, 1)))
    }
}

pub(crate) fn chroma_size((width, height): (u32, u32)) -> (u32, u32) {
    (width.div_ceil(2), height.div_ceil(2))
}

/// Whether `plane` holds `height` rows of `row_bytes` each.
pub(crate) fn holds(plane: PlaneRows<'_>, row_bytes: u32, height: u32) -> bool {
    let needed = (plane.stride as usize)
        .checked_mul(height as usize - 1)
        .and_then(|rows| rows.checked_add(row_bytes as usize));
    plane.stride >= row_bytes && needed.is_some_and(|needed| plane.bytes.len() >= needed)
}

pub(crate) fn chroma_holds(chroma: &ChromaRows<'_>, size: (u32, u32)) -> bool {
    let (width, height) = chroma_size(size);
    chroma
        .planes()
        .all(|(plane, samples)| holds(plane, width * samples, height))
}

/// The pipeline that turns planes into linear RGB.
pub(crate) struct YuvConverter {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

impl YuvConverter {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("yuv"),
            entries: &[
                texture(0),
                texture(1),
                texture(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("yuv"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("yuv.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/yuv.wgsl").into()),
        });
        let pipeline = render_pipeline(
            device,
            &shader,
            &pipeline_layout,
            (WORKING_FORMAT, None),
            "fs_main",
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("chroma"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        Self {
            layout,
            pipeline,
            sampler,
        }
    }
}

/// The plane textures of one layer and what converts them into its texture.
pub(crate) struct PlaneTextures {
    luma: wgpu::Texture,
    chroma: Vec<wgpu::Texture>,
    uniform: wgpu::Buffer,
    binding: wgpu::BindGroup,
}

impl PlaneTextures {
    pub(crate) fn new(
        device: &wgpu::Device,
        converter: &YuvConverter,
        size: (u32, u32),
        layout: ChromaLayout,
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
        let luma = texture("luma", size, wgpu::TextureFormat::R8Unorm);
        let half = chroma_size(size);
        let chroma = match layout {
            ChromaLayout::Separate => vec![
                texture("cb", half, wgpu::TextureFormat::R8Unorm),
                texture("cr", half, wgpu::TextureFormat::R8Unorm),
            ],
            ChromaLayout::Interleaved => vec![texture("cbcr", half, wgpu::TextureFormat::Rg8Unorm)],
        };
        let view = |texture: &wgpu::Texture| texture.create_view(&Default::default());
        let luma_view = view(&luma);
        let cb = view(&chroma[0]);
        let cr = view(chroma.last().unwrap_or(&chroma[0]));
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("yuv"),
            size: UNIFORM_BYTES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("yuv"),
            layout: &converter.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&luma_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&cb),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&cr),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&converter.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        Self {
            luma,
            chroma,
            uniform,
            binding,
        }
    }

    pub(crate) fn upload(
        &self,
        queue: &wgpu::Queue,
        luma: PlaneRows<'_>,
        chroma: &ChromaRows<'_>,
        color: YuvColor,
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
        write(&self.luma, luma);
        for (texture, (plane, _)) in self.chroma.iter().zip(chroma.planes()) {
            write(texture, plane);
        }
        let luma_size = self.luma.size();
        let uniform = uniform(color, (luma_size.width, luma_size.height), chroma.layout());
        queue.write_buffer(&self.uniform, 0, &uniform);
    }

    pub(crate) fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        converter: &YuvConverter,
        target: &wgpu::TextureView,
    ) {
        blit::draw(
            encoder,
            target,
            &converter.pipeline,
            &[[&self.binding]],
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
        );
    }
}

/// The matrix columns, offset, two chroma masks and chroma shift and scale.
const UNIFORM_BYTES: usize = 7 * 16;

fn uniform(color: YuvColor, size: (u32, u32), layout: ChromaLayout) -> [u8; UNIFORM_BYTES] {
    let (columns, offset) = conversion(color);
    let (cb_mask, cr_mask) = match layout {
        ChromaLayout::Separate => ([1.0, 0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0]),
        ChromaLayout::Interleaved => ([1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0]),
    };
    let (chroma_width, chroma_height) = chroma_size(size);
    let scale = [0.5 / chroma_width as f32, 0.5 / chroma_height as f32];
    // Left-sited chroma sits half a luma pixel left of where centered chroma would.
    let shift = match color.siting {
        ChromaSiting::Left => 0.5 * scale[0],
        ChromaSiting::Center => 0.0,
    };
    let [first, second, third] = columns.map(|[r, g, b]| [r, g, b, 0.0]);
    let values: [[f32; 4]; 7] = [
        first,
        second,
        third,
        [offset[0], offset[1], offset[2], 0.0],
        cb_mask,
        cr_mask,
        [shift, 0.0, scale[0], scale[1]],
    ];
    let mut bytes = [0; UNIFORM_BYTES];
    let (chunks, _) = bytes.as_chunks_mut::<4>();
    for (chunk, value) in chunks.iter_mut().zip(values.iter().flatten()) {
        *chunk = value.to_le_bytes();
    }
    bytes
}

/// The matrix's columns from luma, blue- and red-difference to gamma-encoded RGB, and
/// what to subtract from the samples first, both for samples from 0 to 1.
fn conversion(color: YuvColor) -> ([[f32; 3]; 3], [f32; 3]) {
    let (kr, kb) = match color.matrix {
        YuvMatrix::Bt601 => (0.299, 0.114),
        YuvMatrix::Bt709 => (0.2126, 0.0722),
        YuvMatrix::Bt2020 => (0.2627, 0.0593),
    };
    let kg = 1.0 - kr - kb;
    let (luma_scale, chroma_scale, black) = match color.range {
        YuvRange::Limited => (255.0 / 219.0, 255.0 / 224.0, 16.0 / 255.0),
        YuvRange::Full => (1.0, 1.0, 0.0),
    };
    let middle = 128.0 / 255.0;
    let cb = [0.0, -2.0 * kb * (1.0 - kb) / kg, 2.0 * (1.0 - kb)];
    let cr = [2.0 * (1.0 - kr), -2.0 * kr * (1.0 - kr) / kg, 0.0];
    let columns = [
        [luma_scale; 3],
        cb.map(|weight| weight * chroma_scale),
        cr.map(|weight| weight * chroma_scale),
    ];
    (columns, [black, middle, middle])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_rgb(color: YuvColor, [y, cb, cr]: [u8; 3]) -> [f32; 3] {
        let (columns, offset) = conversion(color);
        let sample = [y, cb, cr].map(|value| f32::from(value) / 255.0);
        let mut rgb = [0.0; 3];
        for ((column, value), offset) in columns.iter().zip(sample).zip(offset) {
            for (channel, weight) in rgb.iter_mut().zip(column) {
                *channel += weight * (value - offset);
            }
        }
        rgb.map(|channel| channel * 255.0)
    }

    #[test]
    fn video_levels_span_black_to_white_and_primaries_land_on_their_channel() {
        let color = YuvColor {
            matrix: YuvMatrix::Bt709,
            range: YuvRange::Limited,
            siting: ChromaSiting::Left,
        };
        let close = |got: [f32; 3], want: [f32; 3]| {
            got.iter()
                .zip(want)
                .all(|(got, want)| (got - want).abs() < 1.5)
        };
        assert!(close(to_rgb(color, [16, 128, 128]), [0.0; 3]));
        assert!(close(to_rgb(color, [235, 128, 128]), [255.0; 3]));
        // BT.709 red at video levels.
        assert!(close(to_rgb(color, [63, 102, 240]), [255.0, 0.0, 0.0]));
        let full = YuvColor {
            matrix: YuvMatrix::Bt601,
            range: YuvRange::Full,
            ..color
        };
        // BT.601 blue at full range, as JPEG has it.
        assert!(close(to_rgb(full, [29, 255, 107]), [0.0, 0.0, 255.0]));
    }
}
