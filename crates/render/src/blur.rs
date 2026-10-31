use crate::blit::{self, Blit};
use crate::compositor::extent;
use crate::placement::Premultiplied;
use crate::WORKING_FORMAT;

/// More taps per side than this and the taps spread apart instead.
const MAX_TAPS: f32 = 24.0;

/// Two intermediate textures at the layer's size that the passes write in turn: across,
/// then down, the second written last and placed like the source. A blur too wide for
/// neighbouring taps first runs a narrow blur each way, so its spread taps don't pick
/// single texels out of fine detail and leave stripes.
pub(crate) struct BlurTargets {
    uniforms: [wgpu::Buffer; 4],
    /// Reading the source, the first, the second and the first texture again.
    passes: [wgpu::BindGroup; 4],
    first: wgpu::TextureView,
    second: wgpu::TextureView,
    prefiltered: bool,
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
        let first = view("blur_first");
        let second = view("blur_second");
        let uniforms = [(); 4].map(|()| Blit::placement_buffer(device));
        let read = [source, &first, &second, &first];
        let passes = [0, 1, 2, 3].map(|pass| blit.bind(device, read[pass], &uniforms[pass]));
        Self {
            placed: blit.bind(device, &second, placement),
            uniforms,
            passes,
            first,
            second,
            prefiltered: false,
        }
    }

    /// `sigma` is the red, green, blue and alpha channels' across and then down, in
    /// layer pixels. Returns whether the blurred picture is premultiplied: a straight
    /// source is, when all its channels blur alike, so its colour doesn't bleed in from
    /// transparent texels.
    pub(crate) fn write(
        &mut self,
        queue: &wgpu::Queue,
        (width, height): (u32, u32),
        sigma: [[f32; 4]; 2],
        source: Premultiplied,
    ) -> Premultiplied {
        let premultiply = source == Premultiplied::No
            && sigma
                .iter()
                .all(|way| way.iter().all(|&each| each == way[3]));
        let texel = [[1.0 / width as f32, 0.0], [0.0, 1.0 / height as f32]];
        let spacing = sigma.map(|way| tap_layout(way.into_iter().fold(0.0, f32::max)).0);
        self.prefiltered = spacing.iter().any(|&spacing| spacing > 1.0);
        let passes = if self.prefiltered {
            [0, 1]
                .into_iter()
                .flat_map(|way| {
                    // Gaussians add in variance, so the two together blur by `sigma`.
                    let narrow = sigma[way].map(|sigma| sigma.min(spacing[way]));
                    let rest = [0, 1, 2, 3].map(|channel| {
                        (sigma[way][channel].powi(2) - narrow[channel].powi(2))
                            .max(0.0)
                            .sqrt()
                    });
                    [
                        uniform(texel[way], narrow, premultiply && way == 0),
                        uniform(texel[way], rest, false),
                    ]
                })
                .collect()
        } else {
            vec![
                uniform(texel[0], sigma[0], premultiply),
                uniform(texel[1], sigma[1], false),
            ]
        };
        for (buffer, bytes) in self.uniforms.iter().zip(&passes) {
            queue.write_buffer(buffer, 0, bytes);
        }
        if premultiply {
            Premultiplied::InLight
        } else {
            source
        }
    }

    pub(crate) fn draw(&self, encoder: &mut wgpu::CommandEncoder, blit: &Blit) {
        let count = if self.prefiltered { 4 } else { 2 };
        let targets = [&self.first, &self.second];
        for (pass, bindings) in self.passes.iter().take(count).enumerate() {
            blit::draw(
                encoder,
                targets[pass % 2],
                &blit.blur,
                &[[bindings]],
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            );
        }
    }
}

/// Taps reach three of the widest sigma each side.
fn uniform(texel: [f32; 2], sigma: [f32; 4], premultiply: bool) -> Vec<u8> {
    let widest = sigma.into_iter().fold(0.0, f32::max);
    let (spacing, taps) = tap_layout(widest);
    [texel[0] * spacing, texel[1] * spacing, 0.0, taps]
        .into_iter()
        .chain(sigma.map(|sigma| sigma / spacing))
        .chain([f32::from(u8::from(premultiply)), 0.0, 0.0, 0.0])
        .chain([0.0; 8])
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
