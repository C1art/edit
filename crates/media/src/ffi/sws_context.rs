use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use super::error::check;
use crate::MediaError;

/// A swscale context set up from `options`, slicing its work across threads.
pub(super) fn threaded_context(
    options: &[(&std::ffi::CStr, i64)],
) -> Result<NonNull<sys::SwsContext>, MediaError> {
    // SAFETY: plain allocator call; null means out of memory.
    let context =
        NonNull::new(unsafe { sys::sws_alloc_context() }).ok_or(MediaError::OutOfMemory)?;
    // SAFETY: `context` is a fresh, uninitialized context and every name is one of its
    // options; init validates the sizes and formats, failing on unsupported input.
    let initialized = unsafe {
        options
            .iter()
            .try_for_each(|(name, value)| {
                check(sys::av_opt_set_int(
                    context.as_ptr().cast(),
                    name.as_ptr(),
                    *value,
                    0,
                ))
                .map(drop)
            })
            .and_then(|()| {
                check(sys::sws_init_context(
                    context.as_ptr(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ))
            })
    };
    if let Err(error) = initialized {
        // SAFETY: the context is ours and freed exactly once here.
        unsafe { sys::sws_freeContext(context.as_ptr()) };
        return Err(error);
    }
    Ok(context)
}

/// Pixels handed to swscale stay owned by whoever passed them in.
pub(super) unsafe extern "C" fn borrowed_buffer_release(
    _opaque: *mut std::ffi::c_void,
    _data: *mut u8,
) {
}
