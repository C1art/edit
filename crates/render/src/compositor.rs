use crate::blit::{self, Blit, DISPLAY_FORMAT};
use crate::blur::BlurTargets;
use crate::placement::{fit_scale, LayerLook, Placement};
use crate::{RenderError, WORKING_FORMAT};

/// Decoded frames arrive gamma-encoded. BT.709 video is treated as sRGB, which matches
/// the display-referred look of other editors on an sRGB monitor.
const SOURCE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presented {
    /// The frame went into the existing output texture.
    SameTarget,
    /// The canvas size changed, so the output texture was recreated and must be rebound.
    NewTarget,
}

/// A source picture to composite: tightly packed RGBA8 in the source transfer function.
#[derive(Debug, Clone, Copy)]
pub struct LayerImage<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
    pub look: LayerLook,
}

/// Upload texture and placement for one layer, reused while the layer's size holds.
struct Slot {
    width: u32,
    height: u32,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    placement: wgpu::Buffer,
    binding: wgpu::BindGroup,
    /// Created the first time the layer is blurred.
    blur: Option<BlurTargets>,
    blurred: bool,
}

impl Slot {
    fn placed(&self) -> &wgpu::BindGroup {
        match &self.blur {
            Some(blur) if self.blurred => &blur.placed,
            _ => &self.binding,
        }
    }
}

struct Targets {
    width: u32,
    height: u32,
    working: wgpu::TextureView,
    working_binding: wgpu::BindGroup,
    output_texture: wgpu::Texture,
    output: wgpu::TextureView,
}

/// Uploads source frames, composites them in linear light and presents the result to a
/// texture the UI can draw.
pub struct Compositor {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    blit: Blit,
    slots: Vec<Slot>,
    targets: Option<Targets>,
}

impl Compositor {
    pub(crate) fn output_texture(&self) -> Option<&wgpu::Texture> {
        self.targets.as_ref().map(|targets| &targets.output_texture)
    }
}

impl Compositor {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            blit: Blit::new(device),
            device: device.clone(),
            queue: queue.clone(),
            slots: Vec::new(),
            targets: None,
        }
    }

    /// The presented frame, valid until the next [`Presented::NewTarget`].
    pub fn output_view(&self) -> Option<&wgpu::TextureView> {
        self.targets.as_ref().map(|targets| &targets.output)
    }

    pub fn output_size(&self) -> Option<(u32, u32)> {
        self.targets
            .as_ref()
            .map(|targets| (targets.width, targets.height))
    }

    /// Draws `layers` bottom first over black, each fitted to `canvas` and then moved,
    /// scaled, turned, cropped, blurred and faded by its look.
    pub fn composite(
        &mut self,
        canvas: (u32, u32),
        layers: &[LayerImage<'_>],
    ) -> Result<Presented, RenderError> {
        self.check_size(canvas.0, canvas.1)?;
        for layer in layers {
            self.check_size(layer.width, layer.height)?;
            let expected = layer.width as usize * layer.height as usize * 4;
            if layer.rgba.len() != expected {
                return Err(RenderError::FrameSize {
                    width: layer.width,
                    height: layer.height,
                    bytes: layer.rgba.len(),
                });
            }
        }

        let presented = match &self.targets {
            Some(targets) if (targets.width, targets.height) == canvas => Presented::SameTarget,
            _ => {
                self.targets = Some(self.create_targets(canvas.0, canvas.1));
                Presented::NewTarget
            }
        };
        for (index, layer) in layers.iter().enumerate() {
            self.upload(index, layer, canvas);
        }
        let Some(targets) = &self.targets else {
            unreachable!("targets were created above");
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("compositor"),
            });
        let slots = &self.slots[..layers.len()];
        for slot in slots.iter().filter(|slot| slot.blurred) {
            if let Some(blur) = &slot.blur {
                blur.draw(&mut encoder, &self.blit);
            }
        }
        let layer_bindings: Vec<&wgpu::BindGroup> = slots.iter().map(Slot::placed).collect();
        blit::draw(
            &mut encoder,
            &targets.working,
            &self.blit.to_working,
            &layer_bindings,
        );
        blit::draw(
            &mut encoder,
            &targets.output,
            &self.blit.to_display,
            &[&targets.working_binding],
        );
        self.queue.submit([encoder.finish()]);
        Ok(presented)
    }

    fn check_size(&self, width: u32, height: u32) -> Result<(), RenderError> {
        let max = self.device.limits().max_texture_dimension_2d;
        if width == 0 || height == 0 || width > max || height > max {
            return Err(RenderError::FrameTooLarge { width, height, max });
        }
        Ok(())
    }

    fn upload(&mut self, index: usize, layer: &LayerImage<'_>, canvas: (u32, u32)) {
        let fits = self
            .slots
            .get(index)
            .is_some_and(|slot| (slot.width, slot.height) == (layer.width, layer.height));
        if !fits {
            let slot = self.create_slot(layer.width, layer.height);
            match self.slots.get_mut(index) {
                Some(existing) => *existing = slot,
                None => self.slots.push(slot),
            }
        }
        let size = (layer.width, layer.height);
        // The blur radius is in canvas pixels, but the blur runs on the layer itself.
        let sigma = layer.look.blur / (fit_scale(size, canvas) * layer.look.scale.max(0.01));
        let slot = &mut self.slots[index];
        slot.blurred = sigma >= 0.5;
        if slot.blurred && slot.blur.is_none() {
            let blur =
                BlurTargets::new(&self.device, &self.blit, &slot.view, &slot.placement, size);
            slot.blur = Some(blur);
        }
        if let Some(blur) = slot.blur.as_ref().filter(|_| slot.blurred) {
            blur.write(&self.queue, size, sigma);
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &slot.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            layer.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(layer.width * 4),
                rows_per_image: Some(layer.height),
            },
            extent(layer.width, layer.height),
        );
        let placement = Placement::new(size, canvas, &layer.look);
        Blit::write_placement(&self.queue, &slot.placement, placement);
    }

    fn create_slot(&self, width: u32, height: u32) -> Slot {
        let texture = self.texture(
            "layer",
            (width, height),
            SOURCE_FORMAT,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let placement = Blit::placement_buffer(&self.device);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Slot {
            width,
            height,
            binding: self.blit.bind(&self.device, &view, &placement),
            view,
            texture,
            placement,
            blur: None,
            blurred: false,
        }
    }

    fn create_targets(&self, width: u32, height: u32) -> Targets {
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let working = self
            .texture("working_frame", (width, height), WORKING_FORMAT, usage)
            .create_view(&wgpu::TextureViewDescriptor::default());
        // Copyable so export can read the presented frame back.
        let output_texture = self.texture(
            "output_frame",
            (width, height),
            DISPLAY_FORMAT,
            usage | wgpu::TextureUsages::COPY_SRC,
        );
        let placement = Blit::placement_buffer(&self.device);
        Blit::write_placement(&self.queue, &placement, Placement::FULL);
        Targets {
            width,
            height,
            working_binding: self.blit.bind(&self.device, &working, &placement),
            working,
            output: output_texture.create_view(&wgpu::TextureViewDescriptor::default()),
            output_texture,
        }
    }

    fn texture(
        &self,
        label: &str,
        (width, height): (u32, u32),
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: extent(width, height),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    }
}

pub(crate) fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}
