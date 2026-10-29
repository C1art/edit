use std::sync::Arc;

use timeline::{AlphaInterpretation, BlendMode};

use super::{extent, Compositor, LayerImage};
use crate::blit::Blit;
use crate::blur::BlurTargets;
use crate::grade::Grade;
use crate::layer_shader::Features;
use crate::placement::{fit_scale, Placement};
use crate::WORKING_FORMAT;

/// Decoded frames arrive gamma-encoded. BT.709 video is treated as sRGB, which matches
/// the display-referred look of other editors on an sRGB monitor.
const SOURCE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SlotKind {
    /// Uploaded source pixels.
    Picture,
    /// A canvas-sized working texture that a group is drawn into.
    Group,
}

/// Texture and placement for one layer, reused while the layer's size and kind hold.
pub(super) struct Slot {
    pub(super) kind: SlotKind,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    pub(super) placement: wgpu::Buffer,
    pub(super) binding: wgpu::BindGroup,
    /// Created the first time the layer is blurred.
    pub(super) blur: Option<BlurTargets>,
    pub(super) blurred: bool,
    pub(super) blend: BlendMode,
    /// The effects its placement uses, which pick its pipelines.
    pub(super) features: Features,
    /// The lookups last drawn with and their textures' binding.
    pub(super) grade: Option<(Arc<Grade>, wgpu::BindGroup)>,
}

impl Slot {
    pub(super) fn placed(&self) -> &wgpu::BindGroup {
        match &self.blur {
            Some(blur) if self.blurred => &blur.placed,
            _ => &self.binding,
        }
    }
}

impl Compositor {
    /// The next slot, recreated unless it already has this size and kind.
    pub(super) fn claim(&mut self, next: &mut usize, size: (u32, u32), kind: SlotKind) -> usize {
        let index = *next;
        *next += 1;
        let fits = self
            .slots
            .get(index)
            .is_some_and(|slot| slot.kind == kind && (slot.width, slot.height) == size);
        if !fits {
            let slot = self.create_slot(size, kind);
            match self.slots.get_mut(index) {
                Some(existing) => *existing = slot,
                None => self.slots.push(slot),
            }
        }
        index
    }

    pub(super) fn upload(&self, index: usize, rgba: &[u8]) {
        let slot = &self.slots[index];
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &slot.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(slot.width * 4),
                rows_per_image: Some(slot.height),
            },
            extent(slot.width, slot.height),
        );
    }

    pub(super) fn place(&mut self, index: usize, layer: &LayerImage<'_>, canvas: (u32, u32)) {
        let look = &layer.look;
        let slot = &mut self.slots[index];
        slot.grade = match (&layer.grade, slot.grade.take()) {
            (Some(grade), Some((drawn, binding))) if Arc::ptr_eq(grade, &drawn) => {
                Some((drawn, binding))
            }
            (Some(grade), _) => Some((
                Arc::clone(grade),
                self.blit.grade.bind(&self.device, &self.queue, grade),
            )),
            (None, _) => None,
        };
        slot.blend = look.blend;
        let size = (slot.width, slot.height);
        let shown = look.shown_size(size);
        let look = &look.reframed(shown, canvas);
        // The blur radius is in canvas pixels, but the blur runs on the layer itself.
        let layer_pixels =
            fit_scale(shown, canvas) * (look.scale[0] * look.scale[1]).abs().sqrt().max(0.01);
        let sigma = look.blur.map(|sigma| sigma / layer_pixels);
        slot.blurred = sigma.iter().any(|&sigma| sigma >= 0.5);
        if slot.blurred && slot.blur.is_none() {
            let blur =
                BlurTargets::new(&self.device, &self.blit, &slot.view, &slot.placement, size);
            slot.blur = Some(blur);
        }
        if let Some(blur) = slot.blur.as_ref().filter(|_| slot.blurred) {
            blur.write(&self.queue, size, sigma);
        }
        let placement = Placement {
            // Compositing leaves a group's colors multiplied by its coverage.
            premultiplied: slot.kind == SlotKind::Group
                || look.effects.alpha == AlphaInterpretation::Premultiplied,
            grade: Grade::uniform(layer.grade.as_deref(), look.color.look_intensity),
            ..Placement::new(shown, canvas, look)
        };
        slot.features = self
            .blit
            .write_placement(&self.queue, &slot.placement, placement);
    }

    fn create_slot(&self, (width, height): (u32, u32), kind: SlotKind) -> Slot {
        let (format, written) = match kind {
            SlotKind::Picture => (SOURCE_FORMAT, wgpu::TextureUsages::COPY_DST),
            // Copied to the backdrop under members with a blend mode.
            SlotKind::Group => (
                WORKING_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            ),
        };
        let texture = self.texture(
            "layer",
            (width, height),
            format,
            written | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let placement = Blit::placement_buffer(&self.device);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Slot {
            kind,
            width,
            height,
            binding: self.blit.bind(&self.device, &view, &placement),
            view,
            texture,
            placement,
            blur: None,
            blurred: false,
            blend: BlendMode::Normal,
            features: Features::NONE,
            grade: None,
        }
    }
}
