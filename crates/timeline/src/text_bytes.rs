/// Windows-1252's characters for bytes 0x80 to 0x9F; the five it leaves undefined keep
/// their C1 code points, as Windows decodes them.
const WINDOWS_1252_HIGH: [char; 32] = [
    '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}', '\u{8f}',
    '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}', '\u{178}',
];

/// The text of a file other tools wrote, such as subtitles: UTF-16 when it starts with
/// that byte order mark (Notepad's "Unicode"), UTF-8 when it is valid as such, and
/// otherwise Windows-1252, the code page older subtitle files are typically saved in.
pub fn decode_text(bytes: &[u8]) -> String {
    match bytes {
        [0xff, 0xfe, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xfe, 0xff, rest @ ..] => utf16(rest, u16::from_be_bytes),
        [0xef, 0xbb, 0xbf, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => match std::str::from_utf8(bytes) {
            Ok(text) => text.to_owned(),
            Err(_) => bytes.iter().map(|&byte| windows_1252(byte)).collect(),
        },
    }
}

fn utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> String {
    let units = bytes.as_chunks::<2>().0.iter().map(|&pair| unit(pair));
    char::decode_utf16(units)
        .map(|decoded| decoded.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

fn windows_1252(byte: u8) -> char {
    match byte {
        0x80..=0x9f => WINDOWS_1252_HIGH[usize::from(byte - 0x80)],
        _ => char::from(byte),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_with_a_byte_order_mark_reads_in_either_byte_order() {
        let text = "1\r\nCafé ♪";
        let little: Vec<u8> = [0xff, 0xfe]
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        let big: Vec<u8> = [0xfe, 0xff]
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_be_bytes))
            .collect();
        assert_eq!(decode_text(&little), text);
        assert_eq!(decode_text(&big), text);
    }

    #[test]
    fn bytes_that_are_not_utf8_read_as_windows_1252() {
        assert_eq!(
            decode_text(b"Caf\xe9 \x93quoted\x94 \x80"),
            "Café “quoted” €"
        );
    }
}
