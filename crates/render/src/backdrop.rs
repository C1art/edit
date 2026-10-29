use timeline::BlendMode;

use crate::blit::{self, Blit};
use crate::compositor::extent;
use crate::WORKING_FORMAT;

/// A canvas-sized copy of what lies under a layer drawn with a blend mode.
pub(crate) struct Backdrop {
    texture: wgpu::Texture,
    binding: wgpu::BindGroup,
}

/// A layer's bind groups and how it combines with what lies under it.
pub(crate) struct Drawn<'a> {
    pub(crate) groups: [&'a wgpu::BindGroup; 2],
    pub(crate) blend: BlendMode,
}

impl Backdrop {
    pub(crate) fn new(device: &wgpu::Device, blit: &Blit, (width, height): (u32, u32)) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("backdrop"),
            size: extent(width, height),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: WORKING_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("backdrop_bind_group"),
            layout: &blit.backdrop_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });
        Self { texture, binding }
    }

    /// Draws `layers` bottom first into the canvas-sized `target` cleared to `clear`.
    /// Normal layers share a pass; any other first copies the target here to read what
    /// lies under it, so it takes a pass of its own.
    pub(crate) fn draw_layers(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        blit: &Blit,
        (target, view): (&wgpu::Texture, &wgpu::TextureView),
        layers: &[Drawn<'_>],
        clear: wgpu::Color,
    ) {
        let mut load = wgpu::LoadOp::Clear(clear);
        let mut normal = Vec::new();
        for layer in layers {
            if layer.blend == BlendMode::Normal {
                normal.push(layer.groups);
                continue;
            }
            if !normal.is_empty() || load != wgpu::LoadOp::Load {
                blit::draw(encoder, view, &blit.to_working, &normal, load);
                normal.clear();
                load = wgpu::LoadOp::Load;
            }
            encoder.copy_texture_to_texture(
                target.as_image_copy(),
                self.texture.as_image_copy(),
                target.size(),
            );
            let [placed, grade] = layer.groups;
            let groups = [placed, grade, &self.binding];
            blit::draw(encoder, view, &blit.blended, &[groups], load);
        }
        if !normal.is_empty() || load != wgpu::LoadOp::Load {
            blit::draw(encoder, view, &blit.to_working, &normal, load);
        }
    }
}
