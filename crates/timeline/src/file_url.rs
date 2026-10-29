use std::fmt::Write;
use std::path::{Path, PathBuf};

/// A `file://localhost` URL, the drive of a Windows path becoming its first segment.
pub(crate) fn path_url(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut url = String::from("file://localhost");
    if !text.starts_with('/') {
        url.push('/');
    }
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                url.push(char::from(byte));
            }
            _ => {
                let _ = write!(url, "%{byte:02X}");
            }
        }
    }
    url
}

/// A `file://` URL as a path, with a drive letter for Windows paths.
pub(crate) fn path_from_url(url: &str) -> PathBuf {
    let rest = url
        .strip_prefix("file://localhost")
        .or_else(|| url.strip_prefix("file://"))
        .unwrap_or(url);
    let bytes = rest.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes
            .get(index + 1..index + 3)
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (bytes[index], hex) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                index += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    let text = String::from_utf8_lossy(&decoded).into_owned();
    let drive = text
        .as_bytes()
        .get(1..3)
        .is_some_and(|bytes| bytes[1] == b':')
        && text.starts_with('/');
    PathBuf::from(if drive { text[1..].to_owned() } else { text })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_windows_path_becomes_an_escaped_file_url() {
        assert_eq!(
            path_url(Path::new(r"C:\Media\Day 1\take#2.mov")),
            "file://localhost/C%3A/Media/Day%201/take%232.mov"
        );
    }

    #[test]
    fn a_windows_file_url_becomes_a_drive_path() {
        let path = path_from_url("file://localhost/C%3A/Media/Day%201/take%232.mov");
        assert_eq!(path, PathBuf::from("C:/Media/Day 1/take#2.mov"));
    }
}
