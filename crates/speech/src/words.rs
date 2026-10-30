use timeline::{Ticks, Timebase, Word};

/// A piece of a word as Whisper reads it, timed in hundredths of a second.
pub(crate) struct Token<'a> {
    pub bytes: &'a [u8],
    pub start: i64,
    pub end: i64,
}

/// Whisper's tokens joined into words, each starting at a token that begins with a
/// space and timed from `offset` hundredths. Notes of sounds, as "[Music]" or
/// "(laughs)", aren't words and are dropped.
pub(crate) fn words_of<'a>(
    tokens: impl IntoIterator<Item = Token<'a>>,
    offset: i64,
    timebase: Timebase,
) -> Vec<Word> {
    let mut pieces: Vec<(Vec<u8>, i64, i64)> = Vec::new();
    for token in tokens {
        match pieces.last_mut() {
            Some((bytes, _, end)) if !token.bytes.starts_with(b" ") => {
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
}
