mod compositor;
mod error;

pub use compositor::{Compositor, Presented, DISPLAY_FORMAT};
pub use error::RenderError;

/// Compositing happens in linear light at half-float precision.
pub const WORKING_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
