use std::fmt::Write;

use crate::{Caption, Ticks, Timebase, TimelineError};

const MILLISECONDS: Timebase = match Timebase::new(1, 1_000) {
    Some(timebase) => timebase,
    None => Timebase::HZ_90K,
};

/// Reads SubRip cues, in sequence time. Numbering, styling tags and cue positions are
/// dropped; a cue's lines are kept.
pub fn parse_srt(text: &str, timebase: Timebase) -> Result<Vec<Caption>, TimelineError> {
    let mut cues = Vec::new();
    let mut lines = text
        .trim_start_matches('\u{feff}')
        .lines()
        .map(|line| line.trim_end())
        .enumerate()
        .peekable();
    while let Some((number, line)) = lines.next() {
        if !line.contains("-->") {
            continue;
        }
        let invalid = || TimelineError::InvalidSubtitles { line: number + 1 };
        let (start, end) = line.split_once("-->").ok_or_else(invalid)?;
        let start = timestamp(start.trim()).ok_or_else(invalid)?;
        // Anything after the end time positions the cue, which captions don't use.
        let end = end.split_whitespace().next().and_then(timestamp);
        let end = end.ok_or_else(invalid)?;
        let mut text = String::new();
        while let Some((_, line)) = lines.next_if(|(_, line)| !line.is_empty()) {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&without_tags(line));
        }
        let to_ticks = |ms: i64| Ticks(ms).rescale(MILLISECONDS, timebase);
        cues.push(Caption {
            start: to_ticks(start),
            end: to_ticks(end),
            text,
        });
    }
    Ok(cues)
}

/// Writes captions as SubRip, numbered from one.
pub fn to_srt(captions: &[Caption], timebase: Timebase) -> String {
    let mut out = String::new();
    for (number, caption) in captions.iter().enumerate() {
        let ms = |time: Ticks| time.rescale(timebase, MILLISECONDS).0.max(0);
        let _ = write!(
            out,
            "{}\n{} --> {}\n{}\n\n",
            number + 1,
            format_timestamp(ms(caption.start)),
            format_timestamp(ms(caption.end)),
            caption.text.trim_end()
        );
    }
    out
}

/// Milliseconds from `HH:MM:SS,mmm`, allowing a period for the comma and any number of
/// hour digits.
fn timestamp(text: &str) -> Option<i64> {
    let (clock, fraction) = text.split_once([',', '.'])?;
    let mut parts = clock.split(':').map(|part| part.parse::<i64>().ok());
    let (hours, minutes, seconds) = (parts.next()??, parts.next()??, parts.next()??);
    if parts.next().is_some() || fraction.is_empty() || fraction.len() > 3 {
        return None;
    }
    // "5" after the comma is 500 ms, as in a decimal.
    let millis = fraction.parse::<i64>().ok()? * 10_i64.pow(3 - fraction.len() as u32);
    Some(((hours * 60 + minutes) * 60 + seconds) * 1_000 + millis)
}

fn format_timestamp(ms: i64) -> String {
    let (seconds, millis) = (ms / 1_000, ms % 1_000);
    format!(
        "{:02}:{:02}:{:02},{millis:03}",
        seconds / 3_600,
        seconds / 60 % 60,
        seconds % 60
    )
}

/// Removes `<i>`-style tags, which SubRip players treat as styling.
fn without_tags(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('<') {
        let Some(close) = rest[open..].find('>') else {
            break;
        };
        out.push_str(&rest[..open]);
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cues_read_with_any_numbering_tags_and_positions() {
        let text = "\u{feff}1\r\n00:00:01,500 --> 00:00:03,000 X1:10 X2:20\r\n<i>Hello</i>\r\nthere\r\n\r\n\
                    7\n00:01:00.25 --> 00:01:02,000\nSecond\n";
        let cues = parse_srt(text, Timebase::HZ_90K).unwrap();
        assert_eq!(cues.len(), 2);
        assert_eq!(
            (cues[0].start, cues[0].end),
            (Ticks(135_000), Ticks(270_000))
        );
        assert_eq!(cues[0].text, "Hello\nthere");
        assert_eq!(cues[1].start, Ticks(5_422_500));
        assert!(matches!(
            parse_srt(
                "1\n00:00:01 --> 00:00:02,000\nNo millis\n",
                Timebase::HZ_90K
            ),
            Err(TimelineError::InvalidSubtitles { line: 2 })
        ));
    }

    #[test]
    fn written_captions_read_back() {
        let captions = vec![Caption {
            start: Ticks(90_000 * 3_725 + 9),
            end: Ticks(90_000 * 3_726),
            text: "One\nTwo".to_owned(),
        }];
        let text = to_srt(&captions, Timebase::HZ_90K);
        assert!(text.starts_with("1\n01:02:05,000 --> 01:02:06,000\nOne\nTwo\n\n"));
        let read = parse_srt(&text, Timebase::HZ_90K).unwrap();
        assert_eq!(read[0].end, captions[0].end);
        assert_eq!(read[0].text, captions[0].text);
    }
}
