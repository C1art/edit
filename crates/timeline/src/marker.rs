use serde::{Deserialize, Serialize};

use std::ops::Range;

use crate::Ticks;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MarkerId(pub u64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarkerColor {
    #[default]
    Green,
    Red,
    Purple,
    Orange,
    Yellow,
    White,
    Blue,
    Cyan,
}

impl MarkerColor {
    pub const ALL: [Self; 8] = [
        Self::Green,
        Self::Red,
        Self::Purple,
        Self::Orange,
        Self::Yellow,
        Self::White,
        Self::Blue,
        Self::Cyan,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Green => "Green",
            Self::Red => "Red",
            Self::Purple => "Purple",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::White => "White",
            Self::Blue => "Blue",
            Self::Cyan => "Cyan",
        }
    }
}

/// What a marker is for, as Premiere's marker types: chapters become chapters of an
/// exported file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarkerKind {
    #[default]
    Comment,
    Chapter,
    Segmentation,
    WebLink {
        url: String,
    },
}

impl MarkerKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Comment => "Comment Marker",
            Self::Chapter => "Chapter Marker",
            Self::Segmentation => "Segmentation Marker",
            Self::WebLink { .. } => "Web Link",
        }
    }
}

/// A named point or span in the sequence, for notes and navigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    pub id: MarkerId,
    pub at: Ticks,
    /// Zero for a marker at one point.
    #[serde(default)]
    pub duration: Ticks,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub color: MarkerColor,
    #[serde(default)]
    pub kind: MarkerKind,
}

impl Marker {
    pub fn end(&self) -> Ticks {
        self.at + self.duration
    }
}

/// A part of an exported file that players can skip to, timed from the file's start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub title: String,
    pub range: Range<Ticks>,
}

/// The chapters that `markers` mark in the part of the sequence `range` exports. Each
/// runs until the next one, and the first starts at the beginning of the file even
/// when its marker comes later, as players expect.
pub fn chapters(markers: &[Marker], range: Range<Ticks>) -> Vec<Chapter> {
    let marked: Vec<&Marker> = markers
        .iter()
        .filter(|marker| marker.kind == MarkerKind::Chapter && range.contains(&marker.at))
        .collect();
    let starts = marked.iter().enumerate().map(|(index, marker)| {
        if index == 0 {
            Ticks::ZERO
        } else {
            marker.at - range.start
        }
    });
    let ends = starts.clone().skip(1).chain([range.end - range.start]);
    marked
        .iter()
        .zip(starts.zip(ends))
        .enumerate()
        .map(|(index, (marker, (start, end)))| Chapter {
            title: if marker.name.is_empty() {
                format!("Chapter {}", index + 1)
            } else {
                marker.name.clone()
            },
            range: start..end,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(at: i64, name: &str, kind: MarkerKind) -> Marker {
        Marker {
            id: MarkerId(at as u64),
            at: Ticks(at),
            duration: Ticks::ZERO,
            name: name.into(),
            comment: String::new(),
            color: MarkerColor::default(),
            kind,
        }
    }

    #[test]
    fn chapters_run_to_the_next_from_the_file_start_and_skip_other_markers() {
        let markers = [
            marker(50, "Before the range", MarkerKind::Chapter),
            marker(120, "", MarkerKind::Chapter),
            marker(200, "note", MarkerKind::Comment),
            marker(300, "Before end", MarkerKind::Chapter),
            marker(900, "After the range", MarkerKind::Chapter),
        ];
        let titled = |title: &str, start, end| Chapter {
            title: title.into(),
            range: Ticks(start)..Ticks(end),
        };
        assert_eq!(
            chapters(&markers, Ticks(100)..Ticks(500)),
            [titled("Chapter 1", 0, 200), titled("Before end", 200, 400)]
        );
    }
}
