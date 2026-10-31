use ffmpeg_sys_next as sys;

use super::{Encoder, Frame, Packet, Received};
use crate::{MediaError, StillFormat};

/// The file bytes of an RGBA picture, `size.0 * 4` bytes a row, in `format`.
pub(crate) fn encode_rgba(
    format: StillFormat,
    size: (u32, u32),
    rgba: &[u8],
) -> Result<Vec<u8>, MediaError> {
    let row = size.0 as usize * 4;
    if row == 0 || rgba.len() != row * size.1 as usize {
        return Err(MediaError::InvalidFrame);
    }
    let id = match format {
        StillFormat::Png => sys::AVCodecID::AV_CODEC_ID_PNG,
        StillFormat::Tiff => sys::AVCodecID::AV_CODEC_ID_TIFF,
        StillFormat::Dpx => sys::AVCodecID::AV_CODEC_ID_DPX,
    };
    let mut encoder = Encoder::still(id, size)?;
    let mut frame = Frame::for_video(size, sys::AVPixelFormat::AV_PIX_FMT_RGBA)?;
    let raw = frame.raw_mut();
    let stride = usize::try_from(raw.linesize[0]).map_err(|_| MediaError::InvalidFrame)?;
    for (y, source) in rgba.chunks_exact(row).enumerate() {
        // SAFETY: the frame owns a freshly allocated RGBA plane of `size.1` rows, each
        // `stride >= row` bytes apart, and nothing else references it yet.
        let target = unsafe { std::slice::from_raw_parts_mut(raw.data[0].add(y * stride), row) };
        target.copy_from_slice(source);
    }
    raw.pts = 0;
    encoder.send_frame(Some(&frame))?;
    encoder.send_frame(None)?;
    let mut packet = Packet::new()?;
    let mut file = Vec::new();
    while let Received::Frame = encoder.receive_packet(&mut packet)? {
        file.extend_from_slice(packet.data());
        packet.unref();
    }
    Ok(file)
}
