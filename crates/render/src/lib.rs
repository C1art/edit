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
mod recolor;
mod reshape;
mod scope;
mod secondary;
mod style;

pub use blit::DISPLAY_FORMAT;
pub use compositor::{Compositor, LayerImage, Picture, Presented};
pub use error::RenderError;
pub use grade::{Grade, CURVE_SIZE};
pub use lut::Lut3d;
pub use mask::{LayerMask, Outline, MAX_MASKS, MAX_OUTLINE_POINTS};
pub use pixel::{ChromaKey, PixelEffects, Vignette};
pub use placement::{ColorAdjust, LayerLook};
pub use recolor::{ColorKey, LeaveColor, Ramp, Recolor};
pub use reshape::{Mirror, Reshape};
pub use scope::Scope;
pub use secondary::SecondaryAdjust;
pub use style::PictureStyle;

/// Compositing happens in linear light at half-float precision.
pub const WORKING_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
