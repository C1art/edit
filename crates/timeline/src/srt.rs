use std::fmt::Write;

use crate::{Caption, Ticks, Timebase, TimelineError};

const MILLISECONDS: Timebase = match Timebase::new(1, 1_000) {
    Some(timebase) => timebase,
    None => Timebase::HZ_90K,
};

/// Reads SubRip cues, in sequence time. Numbering, styling tags and cue positions are
/// dropped; a cue's lines are kept. A cue runs to the next timing line, as files in the
/// wild leave out the blank line between cues or put one inside a cue's text.
pub fn parse_srt(text: &str, timebase: Timebase) -> Result<Vec<Caption>, TimelineError> {
    let lines: Vec<&str> = text
        .trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim_end)
        .collect();
    let timings: Vec<usize> = (0..lines.len())
        .filter(|&index| lines[index].contains("-->"))
        .collect();
    let mut cues = Vec::with_capacity(timings.len());
    for (cue, &number) in timings.iter().enumerate() {
        let line = lines[number];
        let invalid = || TimelineError::InvalidSubtitles { line: number + 1 };
        let (start, end) = line.split_once("-->").ok_or_else(invalid)?;
        let start = timestamp(start.trim()).ok_or_else(invalid)?;
        // Anything after the end time positions the cue, which captions don't use.
        let end = end.split_whitespace().next().and_then(timestamp);
        let end = end.ok_or_else(invalid)?;
        let next = timings.get(cue + 1).copied().unwrap_or(lines.len());
        let mut body = &lines[number + 1..next];
        // The next cue's number, unless this is the last cue.
        if next < lines.len() {
            if let Some((last, rest)) = body.split_last() {
                if !last.is_empty() && last.bytes().all(|byte| byte.is_ascii_digit()) {
                    body = rest;
                }
            }
        }
        let first = body.iter().position(|line| !line.is_empty());
        let last = body.iter().rposition(|line| !line.is_empty());
        let body = match (first, last) {
            (Some(first), Some(last)) => &body[first..=last],
            _ => &[],
        };
        let text = body
            .iter()
            .map(|line| without_tags(line))
            .collect::<Vec<_>>()
            .join("\n");
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
    let number = |part: &str| {
        let digits = !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
        digits.then(|| part.parse::<i64>().ok()).flatten()
    };
    let mut parts = clock.split(':').map(number);
    let (hours, minutes, seconds) = (parts.next()??, parts.next()??, parts.next()??);
    if parts.next().is_some() || fraction.len() > 3 {
        return None;
    }
    // "5" after the comma is 500 ms, as in a decimal.
    let millis = number(fraction)? * 10_i64.pow(3 - fraction.len() as u32);
    hours
        .checked_mul(60)?
        .checked_add(minutes)?
        .checked_mul(60)?
        .checked_add(seconds)?
        .checked_mul(1_000)?
        .checked_add(millis)
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

    #[test]
    fn cues_run_to_the_next_timing_line() {
        let text = "1\n00:00:01,000 --> 00:00:02,000\nOne\n\nstill one\n\n2\n\
                    00:00:03,000 --> 00:00:04,000\nTwo\n00:00:05,000 --> 00:00:06,000\nThree\n";
        let cues = parse_srt(text, MILLISECONDS).unwrap();
        let texts: Vec<&str> = cues.iter().map(|cue| cue.text.as_str()).collect();
        assert_eq!(texts, ["One\n\nstill one", "Two", "Three"]);
        let huge = "99999999999999:00:00,000 --> 99999999999999:00:01,000\nX\n";
        assert!(parse_srt(huge, MILLISECONDS).is_err());
        assert!(parse_srt("00:00:-1,000 --> 00:00:01,000\nX\n", MILLISECONDS).is_err());
    }
}
