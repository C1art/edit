use std::ffi::c_char;

pub(crate) fn copy_utf16(source: &str, destination: &mut [u16]) {
    let Some(capacity) = destination.len().checked_sub(1) else {
        return;
    };
    let mut written = 0;
    for (slot, unit) in destination
        .iter_mut()
        .zip(source.encode_utf16().take(capacity))
    {
        *slot = unit;
        written += 1;
    }
    destination[written] = 0;
}

pub(crate) fn copy_char8(source: &str, destination: &mut [c_char]) {
    let Some(capacity) = destination.len().checked_sub(1) else {
        return;
    };
    let mut written = 0;
    for (slot, &byte) in destination
        .iter_mut()
        .zip(source.as_bytes().iter().take(capacity))
    {
        *slot = byte as c_char;
        written += 1;
    }
    destination[written] = 0;
}

/// # Safety
/// `text` must point to a NUL-terminated UTF-16 string.
pub(crate) unsafe fn read_utf16(text: *const u16) -> String {
    let mut len = 0;
    // SAFETY: the string is NUL-terminated per this function's contract.
    while unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `len` units before the terminator were just read.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, len) })
}
