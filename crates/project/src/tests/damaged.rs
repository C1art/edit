//! A file edited by hand or damaged on disk must be refused with an error or load into
//! something the editor can use; it must never take the editor down.

use std::collections::HashMap;
use std::ops::Range;
use std::panic;

use super::sample;
use crate::ProjectFile;

const NASTY: [&str; 8] = [
    "0",
    "-1",
    "1",
    "2147483648",
    "-2147483649",
    "9223372036854775807",
    "-9223372036854775808",
    "170141183460469231731687303715884105727",
];

/// Where each number in `text` is, skipping digits inside strings.
fn numbers(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let (mut spans, mut at, mut in_string) = (Vec::new(), 0, false);
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'\\' if in_string => at += 1,
            b'"' => in_string = !in_string,
            b'-' | b'0'..=b'9' if !in_string => {
                let start = at;
                at += 1;
                while bytes.get(at).is_some_and(u8::is_ascii_digit) {
                    at += 1;
                }
                spans.push(start..at);
                continue;
            }
            _ => {}
        }
        at += 1;
    }
    spans
}

/// What the editor does with a project as soon as it opens.
fn open(text: &str) {
    let Ok(project) = ProjectFile::from_text(text) else {
        return;
    };
    let sequence = &project.sequence;
    let _ = (
        sequence.duration(),
        sequence.marks.range(sequence.duration()),
    );
    for marker in &sequence.markers {
        let _ = marker.end();
    }
    for (_, track) in sequence.tracks() {
        for clip in &track.clips {
            let _ = (clip.end(), clip.source_out());
            for marker in &clip.markers {
                let _ = marker.end();
            }
        }
    }
    let _ = project.to_text();
}

#[test]
fn any_number_damaged_is_refused_or_opens() {
    let text = sample().to_text().unwrap();
    // Effects repeat the same fields many times over; a few of each is enough, and
    // keeps the test quick.
    let mut seen = HashMap::new();
    let spans: Vec<Range<usize>> = numbers(&text)
        .into_iter()
        .filter(|span| {
            let line_start = text[..span.start].rfind('\n').map_or(0, |at| at + 1);
            let field = text[line_start..span.start].trim();
            let count = seen.entry(field).or_insert(0);
            *count += 1;
            *count <= 3
        })
        .collect();
    assert!(spans.len() > 50, "the sample has {} numbers", spans.len());
    let mut crashes = Vec::new();
    for span in spans {
        for value in NASTY {
            let damaged = format!("{}{value}{}", &text[..span.start], &text[span.end..]);
            if panic::catch_unwind(|| open(&damaged)).is_err() {
                let line = text[..span.start].lines().count();
                crashes.push(format!(
                    "line {line}: {value} ({})",
                    text.lines().nth(line - 1).unwrap_or("").trim()
                ));
            }
        }
    }
    assert!(
        crashes.is_empty(),
        "{} crashes:\n{}",
        crashes.len(),
        crashes.join("\n")
    );
}

#[test]
fn a_file_cut_short_anywhere_is_refused() {
    let text = sample().to_text().unwrap();
    let ends: Vec<usize> = text.char_indices().map(|(at, _)| at).skip(1).collect();
    // Odd, so the cuts fall on every kind of character rather than one column.
    let step = (ends.len() / 400) | 1;
    for &end in ends.iter().step_by(step) {
        assert!(
            ProjectFile::from_text(&text[..end]).is_err(),
            "cut at byte {end}"
        );
    }
}
