mod blit;
mod compositor;
mod error;
mod placement;
mod readback;

pub use blit::DISPLAY_FORMAT;
pub use compositor::{Compositor, LayerImage, Presented};
pub use error::RenderError;

/// Compositing happens in linear light at half-float precision.
pub const WORKING_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
