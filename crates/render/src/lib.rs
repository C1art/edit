mod adjust;
mod backdrop;
mod blit;
mod blur;
mod clip_wipes;
mod compositor;
mod creative;
mod detail;
mod error;
mod generate;
mod grade;
mod lut;
mod mask;
mod optics;
mod pixel;
mod placement;
mod readback;
mod recolor;
mod reshape;
mod scope;
mod secondary;
mod style;
mod warp;
mod wipe;

pub use adjust::{Adjust, AlphaAdjust, Levels, LumaKey, ProcAmp};
pub use blit::DISPLAY_FORMAT;
pub use clip_wipes::{
    BlockDissolve, ClipWipes, LinearWipe, RadialSweep, RadialWipe, VenetianBlinds,
};
pub use compositor::{Background, Compositor, LayerImage, Picture, Presented};
pub use creative::{Creative, CreativeVignette};
pub use detail::{Detail, MedianFilter, NoiseHls, UnsharpMask};
pub use error::RenderError;
pub use generate::{Checkerboard, Circle, FourColorGradient, Generate, Grid};
pub use grade::{Grade, CURVE_SIZE};
pub use lut::Lut3d;
pub use mask::{LayerMask, Outline, MAX_MASKS, MAX_OUTLINE_POINTS};
pub use optics::{FlareLens, LensFlare, Magnify, Optics};
pub use pixel::{ChromaKey, PixelEffects, Vignette};
pub use placement::{ColorAdjust, LayerLook};
pub use recolor::{ChangeToColor, ChannelMixer, ColorChange, ColorKey, LeaveColor, Ramp, Recolor};
pub use reshape::{Mirror, PageFold, Reshape};
pub use scope::Scope;
pub use secondary::SecondaryAdjust;
pub use style::PictureStyle;
pub use warp::{PlaneTransform, Warp, MAX_WARPS};
pub use wipe::LayerWipe;

/// Compositing happens in linear light at half-float precision.
pub const WORKING_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
