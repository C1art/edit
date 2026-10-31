//! What was said in a media item's sound, and where its words play in a sequence.

use serde::{Deserialize, Serialize};

use crate::{Caption, ClipId, Sequence, SourceId, Ticks};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transcript {
    /// As speech recognition names it, as "en".
    pub language: String,
    /// In file time, sequence timebase, in order.
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Word {
    pub start: Ticks,
    pub end: Ticks,
    /// With the punctuation that follows it, as "Hello,".
    pub text: String,
}

/// A transcribed word where it plays in a sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpokenWord<'a> {
    pub clip: ClipId,
    pub source: SourceId,
    pub start: Ticks,
    pub end: Ticks,
    pub text: &'a str,
}

impl SpokenWord<'_> {
    pub fn ends_sentence(&self) -> bool {
        self.text
            .trim_end_matches(['"', '\'', ')', '”', '’'])
            .ends_with(['.', '?', '!', '…'])
    }
}

/// The words heard in `sequence`, in order: those of each audible audio clip whose
/// media has a transcript, kept when their middle plays within the clip. Clips playing
/// backwards, held or remapped say nothing readable and are left out, as is the same
/// word heard twice from clips stacked on several tracks.
pub fn spoken_words(sequence: &Sequence) -> Vec<SpokenWord<'_>> {
    let mut spoken: Vec<SpokenWord> = sequence
        .audio
        .iter()
        .filter(|track| !track.muted)
        .flat_map(|track| &track.clips)
        .filter(|clip| !clip.disabled && !clip.reverse && !clip.hold && clip.remap.is_none())
        .filter_map(|clip| {
            let transcript = sequence.media_item(clip.source)?.transcript.as_deref()?;
            Some((clip, transcript))
        })
        .flat_map(|(clip, transcript)| {
            let (source_in, source_out) = (clip.source_in, clip.source_out());
            let first = transcript
                .words
                .partition_point(|word| middle(word.start, word.end) < source_in);
            transcript.words[first..]
                .iter()
                .take_while(move |word| middle(word.start, word.end) < source_out)
                .map(move |word| SpokenWord {
                    clip: clip.id,
                    source: clip.source,
                    start: clip.position_of(word.start.max(source_in)),
                    end: clip.position_of(word.end.min(source_out)),
                    text: &word.text,
                })
        })
        .collect();
    spoken.sort_by_key(|word| (word.start, word.end));
    spoken.dedup_by(|later, earlier| {
        later.source == earlier.source && later.start == earlier.start && later.text == earlier.text
    });
    spoken
}

fn middle(start: Ticks, end: Ticks) -> Ticks {
    Ticks(start.0 + (end.0 - start.0) / 2)
}

/// How captions are cut from spoken words: at most `line_chars` characters on each of
/// `lines` lines, shown no longer than `longest`, and a new one after a silence of
/// `pause` or a sentence's end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptionRules {
    pub line_chars: usize,
    pub lines: usize,
    pub longest: Ticks,
    pub pause: Ticks,
}

/// Captions of `words`, each held until the next starts when the silence between them
/// is shorter than the rules' pause, so short gaps don't flicker.
pub fn captions_from_words(words: &[SpokenWord], rules: CaptionRules) -> Vec<Caption> {
    let mut groups: Vec<Vec<&SpokenWord>> = Vec::new();
    for word in words {
        let starts_new = match groups.last().and_then(|group| Some((group, group.last()?))) {
            None => true,
            Some((group, last)) => {
                let length: usize = group.iter().map(|word| word.text.chars().count() + 1).sum();
                last.ends_sentence()
                    || word.start - last.end >= rules.pause
                    || word.end - group[0].start > rules.longest
                    || length + word.text.chars().count() > rules.line_chars * rules.lines
            }
        };
        if starts_new {
            groups.push(Vec::new());
        }
        if let Some(group) = groups.last_mut() {
            group.push(word);
        }
    }
    let mut captions: Vec<Caption> = groups
        .iter()
        .filter_map(|group| {
            let (first, last) = (group.first()?, group.last()?);
            let text: Vec<&str> = group.iter().map(|word| word.text).collect();
            Some(Caption {
                start: first.start,
                end: last.end,
                text: wrapped(&text, rules.line_chars),
            })
        })
        .collect();
    let starts: Vec<Ticks> = captions
        .iter()
        .skip(1)
        .map(|caption| caption.start)
        .collect();
    for (caption, next) in captions.iter_mut().zip(starts) {
        if next - caption.end < rules.pause {
            caption.end = caption.end.max(next);
        }
    }
    crate::captions_from_cues(captions)
}

/// `words` on one line, or broken in two at the space nearest the middle when longer
/// than `line_chars`.
fn wrapped(words: &[&str], line_chars: usize) -> String {
    let line = words.join(" ");
    let length = line.chars().count();
    if length <= line_chars || words.len() < 2 {
        return line;
    }
    let break_after = words
        .iter()
        .take(words.len() - 1)
        .scan(0, |before, word| {
            *before += word.chars().count() + 1;
            Some(*before)
        })
        .enumerate()
        .min_by_key(|&(_, before)| before.abs_diff(length / 2))
        .map_or(0, |(index, _)| index);
    format!(
        "{}\n{}",
        words[..=break_after].join(" "),
        words[break_after + 1..].join(" ")
    )
}
