mod backdrop;
mod blit;
mod blur;
mod compositor;
mod error;
mod grade;
mod lut;
mod mask;
mod pixel;
mod placement;
mod readback;
mod scope;
mod secondary;

pub use blit::DISPLAY_FORMAT;
pub use compositor::{Compositor, LayerImage, Picture, Presented};
pub use error::RenderError;
pub use grade::{Grade, CURVE_SIZE};
pub use lut::Lut3d;
pub use mask::{LayerMask, MAX_MASKS};
pub use pixel::{ChromaKey, PixelEffects, Vignette};
pub use placement::{ColorAdjust, LayerLook};
pub use scope::Scope;
pub use secondary::SecondaryAdjust;

/// Compositing happens in linear light at half-float precision.
pub const WORKING_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
