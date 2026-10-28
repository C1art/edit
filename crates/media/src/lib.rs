mod codec;
mod error;
mod ffi;
mod frames;
mod peaks;
mod probe;
mod proxy;
mod reader;
mod scenes;
mod seek_trim;
mod still;
mod writer;

pub use codec::{AudioCodec, RateControl, VideoCodec};
pub use error::MediaError;
pub use frames::{AudioBuffer, DecodedFrame, VideoFrame};
/// Decodes video on the GPU where the codec and device allow, from the next file opened.
/// On by default; streams the device can't handle decode in software either way.
pub fn set_hardware_decoding(enabled: bool) {
    ffi::set_hardware_decoding(enabled);
}

/// The kind of device decoding video, such as `d3d11va`, once one is in use.
pub fn hardware_decoder() -> Option<&'static str> {
    ffi::hardware_device_name()
}

pub use peaks::{read_peaks, Peaks};
pub use probe::{probe, MediaInfo, StreamInfo, StreamKind};
pub use proxy::{create_proxy, proxy_size};
pub use reader::{MediaReader, SourceInfo, Streams, VideoInfo};
pub use scenes::detect_scenes;
pub use still::{write_still, StillFormat};
pub use writer::{AudioSettings, EncodeProfile, MediaWriter, VideoSettings};
