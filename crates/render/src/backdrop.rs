use timeline::BlendMode;

use crate::blit::{self, Blit, LayerPass};
use crate::compositor::extent;
use crate::layer_shader::Features;
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
    pub(crate) features: Features,
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
        let binding = blit.bind_backdrop(device, &view);
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
        let draw_normal = |encoder: &mut wgpu::CommandEncoder,
                           normal: &[(wgpu::RenderPipeline, [&wgpu::BindGroup; 2])],
                           load| {
            let quads: Vec<_> = normal
                .iter()
                .map(|(pipeline, groups)| (pipeline, *groups))
                .collect();
            blit::draw_each(encoder, view, &quads, load);
        };
        for layer in layers {
            if layer.blend == BlendMode::Normal {
                normal.push((blit.pipeline(layer.features, LayerPass::Over), layer.groups));
                continue;
            }
            if !normal.is_empty() || load != wgpu::LoadOp::Load {
                draw_normal(encoder, &normal, load);
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
            let blended = blit.pipeline(layer.features, LayerPass::Blended);
            blit::draw(encoder, view, &blended, &[groups], load);
        }
        if !normal.is_empty() || load != wgpu::LoadOp::Load {
            draw_normal(encoder, &normal, load);
        }
    }
}
