use std::any::Any;
use std::sync::Arc;

use timeline::{AlphaInterpretation, BlendMode, PlaneFormat};

use super::{extent, Compositor, LayerImage};
use crate::blit::Blit;
use crate::blur::BlurTargets;
use crate::grade::Grade;
use crate::layer_shader::Features;
use crate::placement::{fit_scale, Placement, Reach};
use crate::yuv::{ChromaLayout, ChromaRows, PlaneRows, PlaneTextures};
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
    /// A working texture that uploaded planes of this size are turned into.
    Planes(ChromaLayout, PlaneFormat, (u32, u32)),
    /// A working texture that a decoder's texture is turned into.
    #[cfg(windows)]
    Surface(PlaneFormat),
}

/// Texture and placement for one layer, reused while the layer's size and kind hold.
pub(super) struct Slot {
    pub(super) kind: SlotKind,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    /// What a slot of planes uploads them to.
    pub(super) planes: Option<PlaneTextures>,
    pub(super) placement: wgpu::Buffer,
    pub(super) binding: wgpu::BindGroup,
    /// Created the first time the layer is blurred.
    pub(super) blur: Option<BlurTargets>,
    pub(super) blurred: bool,
    pub(super) blend: BlendMode,
    pub(super) reach: Reach,
    /// The effects its placement uses, which pick its pipelines.
    pub(super) features: Features,
    /// The lookups last drawn with and their textures' binding.
    pub(super) grade: Option<(Arc<Grade>, wgpu::BindGroup)>,
    /// What the picture in the texture came from. Holding it keeps its address from
    /// being reused, so the same address means the same, unchanged picture.
    pub(super) uploaded: Option<Arc<dyn Any + Send + Sync>>,
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
                Some(existing) => {
                    let replaced = std::mem::replace(existing, slot);
                    self.retire(replaced.kind, replaced.uploaded);
                }
                None => self.slots.push(slot),
            }
        }
        index
    }

    /// Keeps what a slot let go of until the GPU has finished the work queued so far,
    /// when that work may read the decoder's texture it holds.
    fn retire(&mut self, kind: SlotKind, replaced: Option<Arc<dyn Any + Send + Sync>>) {
        #[cfg(windows)]
        if matches!(kind, SlotKind::Surface(_)) {
            self.retired.extend(replaced);
        }
        #[cfg(not(windows))]
        let _ = (kind, replaced);
    }

    /// Whether the slot must take a picture from `source`, which it then holds.
    fn takes(&mut self, index: usize, source: Option<&Arc<dyn Any + Send + Sync>>) -> bool {
        let slot = &mut self.slots[index];
        let same = |held: &Arc<dyn Any + Send + Sync>| {
            source.is_some_and(|source| std::ptr::addr_eq(Arc::as_ptr(held), Arc::as_ptr(source)))
        };
        if slot.uploaded.as_ref().is_some_and(same) {
            return false;
        }
        let replaced = std::mem::replace(&mut slot.uploaded, source.cloned());
        let kind = slot.kind;
        self.retire(kind, replaced);
        true
    }

    pub(super) fn upload(
        &mut self,
        index: usize,
        rgba: &[u8],
        source: Option<&Arc<dyn Any + Send + Sync>>,
    ) {
        if !self.takes(index, source) {
            return;
        }
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

    /// Uploads planes to the slot; true if they must then be converted.
    pub(super) fn upload_planes(
        &mut self,
        index: usize,
        (luma, chroma): (PlaneRows<'_>, &ChromaRows<'_>),
        color: timeline::YuvColor,
        source: Option<&Arc<dyn Any + Send + Sync>>,
    ) -> bool {
        if !self.takes(index, source) {
            return false;
        }
        if let Some(planes) = &self.slots[index].planes {
            let output = (self.slots[index].width, self.slots[index].height);
            planes.upload(&self.queue, (luma, chroma), color, output);
        }
        true
    }

    /// Reads a picture of `size` from the decoder's texture; true if it must then be
    /// converted.
    #[cfg(windows)]
    pub(super) fn upload_surface(
        &mut self,
        index: usize,
        (texture, size): (
            &windows::Win32::Graphics::Direct3D12::ID3D12Resource,
            (u32, u32),
        ),
        (color, format): (timeline::YuvColor, PlaneFormat),
        source: Option<&Arc<dyn Any + Send + Sync>>,
    ) -> bool {
        if !self.takes(index, source) {
            return false;
        }
        let opened = self
            .shared
            .open(&self.device, texture, format)
            .map(|opened| (&opened.luma, &opened.chroma));
        let slot = &mut self.slots[index];
        let output = (slot.width, slot.height);
        if let Some(planes) = &mut slot.planes {
            planes.bind_shared(
                (&self.device, &self.queue),
                &self.yuv,
                opened,
                color,
                (size, output),
            );
        }
        true
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
        slot.reach = look.reach;
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

    fn create_slot(&mut self, (width, height): (u32, u32), kind: SlotKind) -> Slot {
        let (format, written) = match kind {
            SlotKind::Picture => (SOURCE_FORMAT, wgpu::TextureUsages::COPY_DST),
            SlotKind::Planes(..) => (WORKING_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT),
            #[cfg(windows)]
            SlotKind::Surface(_) => (WORKING_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT),
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
        let planes = match kind {
            SlotKind::Planes(layout, format, size) => Some(PlaneTextures::new(
                &self.device,
                &self.yuv,
                size,
                (layout, format),
            )),
            #[cfg(windows)]
            SlotKind::Surface(format) => Some(PlaneTextures::shared(&self.device, format)),
            SlotKind::Picture | SlotKind::Group => None,
        };
        Slot {
            kind,
            width,
            height,
            binding: self.blit.bind(&self.device, &view, &placement),
            view,
            texture,
            planes,
            placement,
            blur: None,
            blurred: false,
            blend: BlendMode::Normal,
            reach: Reach::Everywhere,
            features: Features::NONE,
            grade: None,
            uploaded: None,
        }
    }
}
