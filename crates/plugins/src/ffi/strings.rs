use std::ffi::c_char;

pub(crate) fn from_utf16(buffer: &[u16]) -> String {
    let len = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len])
}

pub(crate) fn from_char8(buffer: &[c_char]) -> String {
    let bytes: Vec<u8> = buffer
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Truncates to fit and always NUL-terminates a non-empty destination.
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
