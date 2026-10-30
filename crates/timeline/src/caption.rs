use serde::{Deserialize, Serialize};

use crate::{Rgb, Ticks, TimelineError};

/// How far above the frame's bottom captions sit, as a fraction of its height, inside
/// title safe.
pub const CAPTION_MARGIN: f32 = 0.08;

/// Subtitles of a sequence, burned into its program picture unless hidden.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Captions {
    /// Sorted by start, never overlapping.
    pub items: Vec<Caption>,
    pub style: CaptionStyle,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caption {
    pub start: Ticks,
    pub end: Ticks,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct CaptionStyle {
    /// Line height in sequence pixels.
    pub size: u32,
    pub color: Rgb,
    /// A box behind each caption, as broadcast captions use.
    pub background: Option<Rgb>,
}

impl Default for CaptionStyle {
    fn default() -> Self {
        Self {
            size: 42,
            color: Rgb::WHITE,
            background: Some(Rgb::BLACK),
        }
    }
}

impl Captions {
    /// The caption showing at `time`.
    pub fn at(&self, time: Ticks) -> Option<&Caption> {
        let after = self.items.partition_point(|caption| caption.start <= time);
        self.items[..after]
            .last()
            .filter(|caption| time < caption.end)
    }

    /// The index of the first caption ending after `time`, where one starting there
    /// would go.
    pub fn index_after(&self, time: Ticks) -> usize {
        self.items.partition_point(|caption| caption.end <= time)
    }

    /// Checks the order and lengths edits must keep.
    pub(crate) fn validate(&self) -> Result<(), TimelineError> {
        if let Some(caption) = self
            .items
            .iter()
            .find(|caption| caption.end <= caption.start)
        {
            return Err(TimelineError::CaptionOutOfOrder(caption.start));
        }
        match self
            .items
            .windows(2)
            .find(|pair| pair[1].start < pair[0].end)
        {
            Some(pair) => Err(TimelineError::CaptionOutOfOrder(pair[1].start)),
            None => Ok(()),
        }
    }
}

/// Cues as subtitle files have them, in any order and possibly overlapping, made into
/// captions: sorted, each cut short where the next starts, and empty ones dropped.
pub fn captions_from_cues(mut cues: Vec<Caption>) -> Vec<Caption> {
    cues.sort_by_key(|cue| cue.start);
    let starts: Vec<Ticks> = cues.iter().skip(1).map(|cue| cue.start).collect();
    for (cue, next) in cues.iter_mut().zip(starts) {
        cue.end = cue.end.min(next);
    }
    cues.retain(|cue| cue.start < cue.end && !cue.text.trim().is_empty());
    cues
}
