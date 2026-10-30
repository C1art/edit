#[cfg(windows)]
mod d3d12;
mod decoder;
mod encode_frame;
mod encoder;
mod error;
mod format;
mod frame;
mod hardware;
mod output;
mod packet;
mod picture;
mod planar;
mod resize;
mod sample_format;
mod scale;
mod still;
mod sws_flags;
mod to_palette;
mod to_yuv;

#[cfg(windows)]
pub use d3d12::GpuSurface;
#[cfg(windows)]
pub(crate) use d3d12::Hold;
pub(crate) use decoder::{Decoder, Received};
pub(crate) use encoder::Encoder;
pub(crate) use format::InputContext;
pub(crate) use frame::Frame;
#[cfg(windows)]
pub(crate) use hardware::set_adapter as set_decode_adapter;
pub(crate) use hardware::{
    device_name as hardware_device_name, set_enabled as set_hardware_decoding,
};
pub(crate) use output::OutputContext;
pub(crate) use packet::Packet;
pub(crate) use picture::Picture;
pub use planar::{Chroma, PlanarFrame, Plane, Planes};
pub use resize::RgbaResizer;
pub(crate) use sample_format::{SampleConverter, OUTPUT_CHANNELS};
pub(crate) use scale::Scaler;
pub(crate) use still::encode_rgba;
pub(crate) use to_palette::Converter;
pub(crate) use to_yuv::RgbaToYuv;
