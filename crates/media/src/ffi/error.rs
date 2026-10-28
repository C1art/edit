use std::ffi::{c_char, c_int, CStr};

use ffmpeg_sys_next as sys;

use crate::MediaError;

pub(crate) fn check(code: c_int) -> Result<c_int, MediaError> {
    if code >= 0 {
        return Ok(code);
    }
    let mut buffer = [0 as c_char; sys::AV_ERROR_MAX_STRING_SIZE];
    // SAFETY: `buffer` is writable for its full length, and av_strerror always
    // NUL-terminates within the given size.
    let message = unsafe {
        sys::av_strerror(code, buffer.as_mut_ptr(), buffer.len());
        CStr::from_ptr(buffer.as_ptr())
    }
    .to_string_lossy()
    .into_owned();
    Err(MediaError::Ffmpeg { code, message })
}
