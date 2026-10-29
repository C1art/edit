use std::ffi::{c_void, CStr};
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use ffmpeg_sys_next as sys;

/// Tried in order; the first that opens serves every decoder in the process.
#[cfg(windows)]
const DEVICE_TYPES: &[sys::AVHWDeviceType] = &[
    sys::AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA,
    sys::AVHWDeviceType::AV_HWDEVICE_TYPE_DXVA2,
];
#[cfg(target_os = "macos")]
const DEVICE_TYPES: &[sys::AVHWDeviceType] = &[sys::AVHWDeviceType::AV_HWDEVICE_TYPE_VIDEOTOOLBOX];
#[cfg(not(any(windows, target_os = "macos")))]
const DEVICE_TYPES: &[sys::AVHWDeviceType] = &[sys::AVHWDeviceType::AV_HWDEVICE_TYPE_VAAPI];

static ENABLED: AtomicBool = AtomicBool::new(true);
/// Opened on first use; `None` once every device type failed, so it isn't retried.
static DEVICE: OnceLock<Option<SharedDevice>> = OnceLock::new();

struct SharedDevice {
    buffer: NonNull<sys::AVBufferRef>,
    kind: sys::AVHWDeviceType,
}

// SAFETY: the buffer is only referenced, never mutated, and AVBufferRef reference
// counting is atomic. FFmpeg serializes access to the device itself.
unsafe impl Send for SharedDevice {}
// SAFETY: as above; shared access only takes new references.
unsafe impl Sync for SharedDevice {}

/// A reference to the shared device, released on drop unless handed to a decoder.
pub(super) struct DeviceRef(NonNull<sys::AVBufferRef>);

impl DeviceRef {
    /// Gives the reference to FFmpeg, which releases it with the codec context.
    pub(super) fn into_raw(self) -> *mut sys::AVBufferRef {
        let raw = self.0.as_ptr();
        std::mem::forget(self);
        raw
    }
}

impl Drop for DeviceRef {
    fn drop(&mut self) {
        let mut raw = self.0.as_ptr();
        // SAFETY: this reference came from av_buffer_ref and is released exactly once.
        unsafe { sys::av_buffer_unref(&mut raw) };
    }
}

pub(crate) fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// The name of the device decoding video, once one has been opened.
pub(crate) fn device_name() -> Option<&'static str> {
    let device = DEVICE.get()?.as_ref()?;
    // SAFETY: returns a static string for valid device types, or null.
    let name = unsafe { sys::av_hwdevice_get_type_name(device.kind) };
    if name.is_null() {
        return None;
    }
    // SAFETY: non-null names are static NUL-terminated strings.
    unsafe { CStr::from_ptr(name) }.to_str().ok()
}

/// A device reference and the pixel format `codec` decodes to on it, when hardware
/// decoding is enabled and the codec supports the device.
pub(super) fn device_for(codec: *const sys::AVCodec) -> Option<(DeviceRef, sys::AVPixelFormat)> {
    if !ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    let device = DEVICE.get_or_init(open_device).as_ref()?;
    let format = hardware_format(codec, device.kind)?;
    // SAFETY: the shared buffer lives for the whole process.
    let reference = NonNull::new(unsafe { sys::av_buffer_ref(device.buffer.as_ptr()) })?;
    Some((DeviceRef(reference), format))
}

/// Stores the wanted hardware format where [`choose_format`] finds it.
pub(super) fn format_tag(format: sys::AVPixelFormat) -> *mut c_void {
    format as i32 as isize as *mut c_void
}

/// FFmpeg's `get_format` callback: the hardware format tagged in `opaque` when offered,
/// otherwise FFmpeg's own software choice, e.g. for a profile the device can't decode.
///
/// # Safety
///
/// FFmpeg calls it with a valid context and a list of formats ending in
/// `AV_PIX_FMT_NONE`.
pub(super) unsafe extern "C" fn choose_format(
    context: *mut sys::AVCodecContext,
    formats: *const sys::AVPixelFormat,
) -> sys::AVPixelFormat {
    // SAFETY: the caller passes a valid context whose `opaque` was set by `format_tag`.
    let wanted = unsafe { (*context).opaque } as isize;
    let mut cursor = formats;
    // SAFETY: the list is terminated by AV_PIX_FMT_NONE, so every read is in bounds.
    unsafe {
        while *cursor != sys::AVPixelFormat::AV_PIX_FMT_NONE {
            if *cursor as i32 as isize == wanted {
                return *cursor;
            }
            cursor = cursor.add(1);
        }
        sys::avcodec_default_get_format(context, formats)
    }
}

fn open_device() -> Option<SharedDevice> {
    DEVICE_TYPES.iter().find_map(|&kind| {
        let mut buffer = ptr::null_mut();
        // SAFETY: on success FFmpeg stores a new device reference in `buffer`; the null
        // device name and options select the default adapter.
        let code = unsafe {
            sys::av_hwdevice_ctx_create(&mut buffer, kind, ptr::null(), ptr::null_mut(), 0)
        };
        if code < 0 {
            return None;
        }
        NonNull::new(buffer).map(|buffer| SharedDevice { buffer, kind })
    })
}

fn hardware_format(
    codec: *const sys::AVCodec,
    kind: sys::AVHWDeviceType,
) -> Option<sys::AVPixelFormat> {
    let method = sys::AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX as i32;
    (0..)
        .map_while(|index| {
            // SAFETY: `codec` is a valid decoder; null marks the end of its configs.
            let config = unsafe { sys::avcodec_get_hw_config(codec, index) };
            // SAFETY: non-null configs are static descriptions owned by the codec.
            unsafe { config.as_ref() }
        })
        .find(|config| config.methods & method != 0 && config.device_type == kind)
        .map(|config| config.pix_fmt)
}
