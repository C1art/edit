mod block_match;
mod channel_map;
mod codec;
mod deinterlace;
mod error;
mod ffi;
mod fit;
mod frames;
mod motion;
mod peaks;
mod probe;
mod proxy;
mod quantize;
mod reader;
mod region_track;
mod scenes;
mod seek_landing;
mod seek_trim;
mod sequence;
mod spare_pixels;
mod still;
mod subject;
#[cfg(test)]
mod test_picture;
mod turn;
mod writer;

pub use codec::{AudioCodec, DnxhrQuality, RateControl, VideoCodec};
pub use error::MediaError;
pub use ffi::{Chroma, PlanarFrame, Plane, Planes, RgbaResizer};
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

pub use motion::{track_motion, Motion};
pub use peaks::{read_peaks, Peaks};
pub use probe::{probe, MediaInfo, StreamInfo, StreamKind};
pub use proxy::{create_proxy, proxy_size};
pub use reader::{MediaReader, Progress, SourceInfo, Streams, VideoInfo};
pub use region_track::{track_region, RegionTrack, TrackDirection, TrackedFrame};
pub use scenes::detect_scenes;
pub use sequence::{
    sequence_frame, sequence_frames, sequence_pattern, source_exists, source_files,
};
pub use still::{write_still, StillFormat};
pub use subject::track_subject;
pub use writer::{AudioSettings, EncodeProfile, MediaWriter, VideoSettings};
