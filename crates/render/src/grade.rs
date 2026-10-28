use std::sync::Arc;

use crate::lut::{half_rgba, Lut3d};

/// Entries in a curves table, one per 8-bit level.
pub const CURVE_SIZE: usize = 256;

/// Lookups a layer's colors pass through around its color correction, in gamma-encoded
/// values as grading tools apply them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Grade {
    /// Applied to the source before any correction.
    pub input: Option<Arc<Lut3d>>,
    /// Applied after the correction, mixed in by the look's intensity.
    pub look: Option<Arc<Lut3d>>,
    /// [`CURVE_SIZE`] red, green and blue outputs, applied last.
    pub curves: Option<Vec<[f32; 3]>>,
}

impl Grade {
    /// The input and look LUT sizes (0 for none), the look's intensity, then 1 when
    /// curves apply.
    pub(crate) fn uniform(grade: Option<&Grade>, look_intensity: f32) -> [f32; 4] {
        let Some(grade) = grade else {
            return [0.0; 4];
        };
        let size = |lut: &Option<Arc<Lut3d>>| lut.as_ref().map_or(0.0, |lut| lut.size as f32);
        let curves = if grade.curves.is_some() { 1.0 } else { 0.0 };
        [
            size(&grade.input),
            size(&grade.look),
            look_intensity,
            curves,
        ]
    }
}

/// The layout of a layer's lookup textures, and a binding for layers without any.
pub(crate) struct GradeLayout {
    pub(crate) layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pub(crate) none: wgpu::BindGroup,
}

impl GradeLayout {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let texture = |binding, view_dimension| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("grade_layout"),
            entries: &[
                texture(0, wgpu::TextureViewDimension::D2),
                texture(1, wgpu::TextureViewDimension::D3),
                texture(2, wgpu::TextureViewDimension::D3),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("grade_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // Never sampled: the shader skips lookups a layer doesn't have.
        let blank_2d = blank(device, wgpu::TextureDimension::D2);
        let blank_3d = blank(device, wgpu::TextureDimension::D3);
        let none = bind(device, &layout, &sampler, [&blank_2d, &blank_3d, &blank_3d]);
        Self {
            layout,
            sampler,
            none,
        }
    }

    pub(crate) fn bind(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        grade: &Grade,
    ) -> wgpu::BindGroup {
        let curves = match &grade.curves {
            Some(table) => {
                let width = u32::try_from(table.len()).unwrap_or(u32::MAX);
                upload(
                    device,
                    queue,
                    wgpu::TextureDimension::D2,
                    table,
                    (width, 1, 1),
                )
            }
            None => blank(device, wgpu::TextureDimension::D2),
        };
        let lut = |lut: &Option<Arc<Lut3d>>| match lut {
            Some(lut) => upload(
                device,
                queue,
                wgpu::TextureDimension::D3,
                &lut.table,
                (lut.size, lut.size, lut.size),
            ),
            None => blank(device, wgpu::TextureDimension::D3),
        };
        let (input, look) = (lut(&grade.input), lut(&grade.look));
        bind(
            device,
            &self.layout,
            &self.sampler,
            [&curves, &input, &look],
        )
    }
}

fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    textures: [&wgpu::Texture; 3],
) -> wgpu::BindGroup {
    let views =
        textures.map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));
    let [curves, input, look] = &views;
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("grade_bind_group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(curves),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(input),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(look),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn blank(device: &wgpu::Device, dimension: wgpu::TextureDimension) -> wgpu::Texture {
    texture(device, dimension, (1, 1, 1))
}

fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    dimension: wgpu::TextureDimension,
    table: &[[f32; 3]],
    size: (u32, u32, u32),
) -> wgpu::Texture {
    let texture = texture(device, dimension, size);
    // Eight bytes a texel: four half floats.
    queue.write_texture(
        texture.as_image_copy(),
        &half_rgba(table),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size.0 * 8),
            rows_per_image: Some(size.1),
        },
        wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: size.2,
        },
    );
    texture
}

fn texture(
    device: &wgpu::Device,
    dimension: wgpu::TextureDimension,
    (width, height, depth): (u32, u32, u32),
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("grade_lookup"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: depth,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension,
        format: crate::WORKING_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}
