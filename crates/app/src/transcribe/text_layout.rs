//! The words heard in the sequence laid out as paragraphs, kept until the sequence or
//! the panel's width changes.

use std::ops::Range;
use std::sync::Arc;

use egui::text::{CCursor, LayoutJob};
use egui::{Color32, FontId, Galley, Pos2, Rect, Vec2};
use timeline::{spoken_words, Sequence, SpokenWord, Ticks, Timebase};

/// Silence long enough to start a new paragraph, in seconds.
const PARAGRAPH_PAUSE: i64 = 2;
/// Words a paragraph may run to before it ends at the next sentence's end.
const PARAGRAPH_WORDS: usize = 60;
const TIME_GAP: f32 = 2.0;
const PARAGRAPH_GAP: f32 = 12.0;

pub(super) struct TextLayout {
    pub(super) revision: u64,
    pub(super) width: f32,
    pub(super) words: Vec<PlacedWord>,
    pub(super) paragraphs: Vec<Paragraph>,
    pub(super) height: f32,
}

pub(super) struct PlacedWord {
    pub(super) start: Ticks,
    pub(super) end: Ticks,
    pub(super) paragraph: usize,
    /// Where the word sits in its paragraph's text, in characters.
    pub(super) chars: Range<usize>,
    pub(super) lowercase: String,
}

pub(super) struct Paragraph {
    pub(super) time: Arc<Galley>,
    pub(super) text: Arc<Galley>,
    pub(super) top: f32,
    pub(super) words: Range<usize>,
}

impl Paragraph {
    pub(super) fn text_top(&self) -> f32 {
        self.top + self.time.size().y + TIME_GAP
    }

    pub(super) fn bottom(&self) -> f32 {
        self.text_top() + self.text.size().y
    }
}

impl TextLayout {
    pub(super) fn new(
        painter: &egui::Painter,
        sequence: &Sequence,
        (revision, width): (u64, f32),
        (text_color, time_color): (Color32, Color32),
        time_label: &dyn Fn(Ticks) -> String,
    ) -> Self {
        let spoken = spoken_words(sequence);
        let breaks = paragraph_starts(&spoken, sequence.timebase);
        let mut words = Vec::with_capacity(spoken.len());
        let mut paragraphs = Vec::with_capacity(breaks.len());
        let mut top = 0.0;
        let ends = breaks.iter().skip(1).copied().chain([spoken.len()]);
        for (index, (first, end)) in breaks.iter().copied().zip(ends).enumerate() {
            let mut text = String::new();
            for word in &spoken[first..end] {
                if !text.is_empty() {
                    text.push(' ');
                }
                let from = text.chars().count();
                text.push_str(word.text);
                words.push(PlacedWord {
                    start: word.start,
                    end: word.end,
                    paragraph: index,
                    chars: from..text.chars().count(),
                    lowercase: word.text.to_lowercase(),
                });
            }
            let time = painter.layout_job(LayoutJob::simple_singleline(
                time_label(spoken[first].start),
                FontId::proportional(11.0),
                time_color,
            ));
            let text = painter.layout_job(LayoutJob::simple(
                text,
                FontId::proportional(14.0),
                text_color,
                width,
            ));
            let paragraph = Paragraph {
                time,
                text,
                top,
                words: first..end,
            };
            top = paragraph.bottom() + PARAGRAPH_GAP;
            paragraphs.push(paragraph);
        }
        Self {
            revision,
            width,
            words,
            paragraphs,
            height: top,
        }
    }

    /// The word's box relative to the top left of the text.
    pub(super) fn word_rect(&self, index: usize) -> Option<Rect> {
        let word = self.words.get(index)?;
        let paragraph = &self.paragraphs[word.paragraph];
        let start = paragraph
            .text
            .pos_from_cursor(CCursor::new(word.chars.start));
        let end = paragraph.text.pos_from_cursor(CCursor::new(word.chars.end));
        let right = if end.min.y > start.min.y {
            paragraph.text.size().x
        } else {
            end.max.x
        };
        let rect = Rect::from_min_max(start.min, Pos2::new(right, start.max.y));
        Some(
            rect.translate(Vec2::new(0.0, paragraph.text_top()))
                .expand2(Vec2::new(2.0, 0.0)),
        )
    }

    /// The word at `at`, relative to the top left of the text, or the nearest one on
    /// the same line.
    pub(super) fn word_at(&self, at: Pos2) -> Option<usize> {
        let index = self
            .paragraphs
            .partition_point(|paragraph| paragraph.bottom() < at.y);
        let paragraph = self.paragraphs.get(index)?;
        let local = at - Vec2::new(0.0, paragraph.text_top());
        let cursor: usize = paragraph.text.cursor_from_pos(local.to_vec2()).index.into();
        let words = &self.words[paragraph.words.clone()];
        let within = words.partition_point(|word| word.chars.end < cursor);
        Some(paragraph.words.start + within.min(words.len().saturating_sub(1)))
    }

    /// The last word starting at or before `time`.
    pub(super) fn word_playing(&self, time: Ticks) -> Option<usize> {
        let after = self.words.partition_point(|word| word.start <= time);
        let index = after.checked_sub(1)?;
        (self.words[index].end > time).then_some(index)
    }
}

/// The first word of each paragraph: after a long silence, or at a sentence's end once
/// the paragraph runs long.
fn paragraph_starts(words: &[SpokenWord], timebase: Timebase) -> Vec<usize> {
    let second = Timebase::new(1, 1).unwrap_or(Timebase::HZ_90K);
    let pause = Ticks(PARAGRAPH_PAUSE).rescale(second, timebase);
    let mut starts = Vec::new();
    for (index, word) in words.iter().enumerate() {
        let new = match index.checked_sub(1).map(|previous| &words[previous]) {
            None => true,
            Some(previous) => {
                let length = index - starts.last().copied().unwrap_or(0);
                word.start - previous.end >= pause
                    || (previous.ends_sentence() && length >= PARAGRAPH_WORDS)
            }
        };
        if new {
            starts.push(index);
        }
    }
    starts
}
