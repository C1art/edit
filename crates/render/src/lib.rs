mod blit;
mod blur;
mod compositor;
mod error;
mod pixel;
mod placement;
mod readback;
mod waveform;

pub use blit::DISPLAY_FORMAT;
pub use compositor::{Compositor, LayerImage, Picture, Presented};
pub use error::RenderError;
pub use pixel::{ChromaKey, PixelEffects, Vignette};
pub use placement::{ColorAdjust, LayerLook};

/// Compositing happens in linear light at half-float precision.
pub const WORKING_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
