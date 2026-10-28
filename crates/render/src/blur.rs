use crate::blit::{self, Blit};
use crate::compositor::extent;
use crate::WORKING_FORMAT;

/// More taps per side than this and the taps spread apart instead, which the linear
/// sampler smooths over.
const MAX_TAPS: f32 = 24.0;

/// Two intermediate textures at the layer's size: the horizontal pass writes the first
/// and the vertical pass writes the second, which is then placed like the source.
pub(crate) struct BlurTargets {
    horizontal_uniform: wgpu::Buffer,
    vertical_uniform: wgpu::Buffer,
    horizontal: wgpu::BindGroup,
    vertical: wgpu::BindGroup,
    horizontal_view: wgpu::TextureView,
    vertical_view: wgpu::TextureView,
    /// Places the blurred picture; shares the layer's placement buffer.
    pub(crate) placed: wgpu::BindGroup,
}

impl BlurTargets {
    pub(crate) fn new(
        device: &wgpu::Device,
        blit: &Blit,
        source: &wgpu::TextureView,
        placement: &wgpu::Buffer,
        (width, height): (u32, u32),
    ) -> Self {
        let view = |label| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: extent(width, height),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: WORKING_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        let horizontal_view = view("blur_horizontal");
        let vertical_view = view("blur_vertical");
        let horizontal_uniform = Blit::placement_buffer(device);
        let vertical_uniform = Blit::placement_buffer(device);
        Self {
            horizontal: blit.bind(device, source, &horizontal_uniform),
            vertical: blit.bind(device, &horizontal_view, &vertical_uniform),
            placed: blit.bind(device, &vertical_view, placement),
            horizontal_uniform,
            vertical_uniform,
            horizontal_view,
            vertical_view,
        }
    }

    /// `sigma` is in layer pixels.
    pub(crate) fn write(&self, queue: &wgpu::Queue, (width, height): (u32, u32), sigma: f32) {
        let horizontal = [1.0 / width as f32, 0.0];
        let vertical = [0.0, 1.0 / height as f32];
        queue.write_buffer(&self.horizontal_uniform, 0, &uniform(horizontal, sigma));
        queue.write_buffer(&self.vertical_uniform, 0, &uniform(vertical, sigma));
    }

    pub(crate) fn draw(&self, encoder: &mut wgpu::CommandEncoder, blit: &Blit) {
        blit::draw(
            encoder,
            &self.horizontal_view,
            &blit.blur,
            &[&self.horizontal],
        );
        blit::draw(encoder, &self.vertical_view, &blit.blur, &[&self.vertical]);
    }
}

/// Taps reach three sigma each side.
fn uniform(texel: [f32; 2], sigma: f32) -> Vec<u8> {
    let (spacing, taps) = tap_layout(sigma);
    [
        texel[0] * spacing,
        texel[1] * spacing,
        sigma / spacing,
        taps,
    ]
    .into_iter()
    .chain([0.0; 16])
    .flat_map(f32::to_le_bytes)
    .collect()
}

/// Texels between taps and taps each side.
fn tap_layout(sigma: f32) -> (f32, f32) {
    let reach = (sigma * 3.0).ceil().max(1.0);
    let spacing = (reach / MAX_TAPS).max(1.0);
    (spacing, (reach / spacing).ceil())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_blurs_spread_their_taps() {
        assert_eq!(tap_layout(2.0), (1.0, 6.0));
        assert_eq!(tap_layout(32.0), (4.0, 24.0));
    }
}
