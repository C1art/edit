use timeline::{Ticks, Timebase, Word};

/// A piece of a word as Whisper reads it, timed in hundredths of a second.
pub(crate) struct Token<'a> {
    pub bytes: &'a [u8],
    pub start: i64,
    pub end: i64,
}

/// Whisper's tokens joined into words, each starting at a token that begins with a
/// space, or with a character of a script written without spaces, and timed from
/// `offset` hundredths. Notes of sounds, as "[Music]" or
/// "(laughs)", aren't words and are dropped.
pub(crate) fn words_of<'a>(
    tokens: impl IntoIterator<Item = Token<'a>>,
    offset: i64,
    timebase: Timebase,
) -> Vec<Word> {
    let mut pieces: Vec<(Vec<u8>, i64, i64)> = Vec::new();
    for token in tokens {
        match pieces.last_mut() {
            Some((bytes, _, end))
                if !token.bytes.starts_with(b" ") && !starts_unspaced_word(bytes, token.bytes) =>
            {
                bytes.extend_from_slice(token.bytes);
                *end = token.end.max(*end);
            }
            _ => pieces.push((token.bytes.to_vec(), token.start, token.end)),
        }
    }
    let hundredths = Timebase::new(1, 100).unwrap_or(Timebase::HZ_90K);
    let time = |at: i64| Ticks(offset + at).rescale(hundredths, timebase);
    pieces
        .into_iter()
        .filter_map(|(bytes, start, end)| {
            let text = String::from_utf8_lossy(&bytes).trim().to_owned();
            let note = (text.starts_with('[') && text.ends_with(']'))
                || (text.starts_with('(') && text.ends_with(')'));
            (!text.is_empty() && !note).then(|| Word {
                start: time(start),
                end: time(end.max(start)),
                text,
            })
        })
        .collect()
}

/// Chinese, Japanese, Thai and the like have no spaces to start words, so each token
/// that starts with one of their characters does, once the word before it holds whole
/// characters: a token can end partway through one.
fn starts_unspaced_word(before: &[u8], token: &[u8]) -> bool {
    std::str::from_utf8(before).is_ok() && first_char(token).is_some_and(is_unspaced)
}

/// The character `token` starts with, or the lowest one it can be when the token ends
/// partway through it, which tells its script.
fn first_char(token: &[u8]) -> Option<char> {
    let length = match *token.first()? {
        lead if lead < 0x80 => 1,
        lead if lead >= 0xF0 => 4,
        lead if lead >= 0xE0 => 3,
        lead if lead >= 0xC0 => 2,
        _ => return None,
    };
    let mut padded = [0x80; 4];
    let known = token.len().min(length);
    padded[..known].copy_from_slice(&token[..known]);
    std::str::from_utf8(&padded[..length]).ok()?.chars().next()
}

fn is_unspaced(char: char) -> bool {
    matches!(
        char,
        // Hiragana and katakana, CJK ideographs, Thai, Lao, Myanmar and Khmer.
        '\u{3040}'..='\u{30FF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{0E00}'..='\u{0EFF}'
            | '\u{1000}'..='\u{109F}'
            | '\u{1780}'..='\u{17FF}'
    )
}

/// Where to end a stretch of `samples` so no word is cut: the middle of the quietest
/// tenth of a second within the last `search` samples.
pub(crate) fn quietest_cut(samples: &[f32], search: usize, rate: usize) -> usize {
    let window = (rate / 10).max(1);
    let from = samples.len().saturating_sub(search);
    samples[from..]
        .chunks_exact(window)
        .enumerate()
        .min_by(|(_, a), (_, b)| energy(a).total_cmp(&energy(b)))
        .map_or(samples.len(), |(index, _)| {
            from + index * window + window / 2
        })
}

fn energy(samples: &[f32]) -> f32 {
    samples.iter().map(|sample| sample * sample).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_join_into_timed_words_without_notes_of_sounds() {
        let tokens = [
            (" Hel", 0, 10),
            ("lo", 10, 20),
            (",", 20, 21),
            (" [", 30, 31),
            ("Music", 31, 40),
            ("]", 40, 41),
            (" caf\u{c3}", 50, 55),
            ("\u{a9}.", 55, 60),
        ];
        let bytes: Vec<Vec<u8>> = tokens
            .iter()
            .map(|(text, _, _)| text.chars().map(|c| c as u8).collect())
            .collect();
        let words = words_of(
            tokens
                .iter()
                .zip(&bytes)
                .map(|(&(_, start, end), bytes)| Token { bytes, start, end }),
            100,
            Timebase::HZ_90K,
        );
        let found: Vec<(i64, i64, &str)> = words
            .iter()
            .map(|word| (word.start.0, word.end.0, word.text.as_str()))
            .collect();
        assert_eq!(
            found,
            [
                (90_000, 90_000 + 18_900, "Hello,"),
                (135_000, 144_000, "café.")
            ]
        );
    }

    #[test]
    fn unspaced_scripts_split_into_words_at_whole_characters() {
        let hao = "好".as_bytes();
        let tokens = [
            ("你".as_bytes(), 0, 10),
            (&hao[..1], 10, 15),
            (&hao[1..], 15, 20),
            ("。".as_bytes(), 20, 21),
        ];
        let words = words_of(
            tokens.map(|(bytes, start, end)| Token { bytes, start, end }),
            0,
            Timebase::new(1, 100).unwrap_or(Timebase::HZ_90K),
        );
        let found: Vec<&str> = words.iter().map(|word| word.text.as_str()).collect();
        assert_eq!(found, ["你", "好。"]);
    }
}
