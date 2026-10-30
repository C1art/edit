use std::ops::Range;
use std::sync::Arc;

use super::generate::generated_item;
use crate::{
    Bin, BinId, Caption, CaptionStyle, Clip, ClipId, Edit, Generator, Param, Sequence, Ticks,
    TimelineError, Title, Track, CAPTION_MARGIN,
};

/// The longest a title made from a caption is named, in characters.
const NAME_LENGTH: usize = 40;

/// Replaces the captions within `range` with title clips on a new top video track, each
/// where its caption showed, so they can be styled and animated as titles. The titles'
/// media is kept in a bin of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptionsToTitles {
    pub range: Range<Ticks>,
}

impl Edit for CaptionsToTitles {
    fn name(&self) -> &'static str {
        "Captions to Titles"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let within =
            |caption: &Caption| caption.start < self.range.end && self.range.start < caption.end;
        let (upgraded, kept) = sequence.captions.items.iter().cloned().partition(within);
        let upgraded: Vec<Caption> = upgraded;
        if upgraded.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        sequence.captions.items = kept;
        let bin = BinId(sequence.ids.next());
        sequence.bins.push(Bin {
            id: bin,
            name: "Captions".to_owned(),
            parent: None,
            search: None,
        });
        let style = sequence.captions.style;
        let height = sequence.format.height;
        let mut track = Track {
            name: Some("Captions".to_owned()),
            ..Track::default()
        };
        for caption in upgraded {
            let title = title_of(&caption.text, style);
            let rise = lowered(&title, height);
            let mut item =
                generated_item(sequence, name_of(&caption.text), Generator::Title(title));
            item.bin = Some(bin);
            let source = item.id;
            Arc::make_mut(&mut sequence.media).push(item);
            let id = ClipId(sequence.ids.next());
            let duration = caption.end - caption.start;
            let mut clip = Clip::new(id, source, caption.start, duration, Ticks::ZERO);
            clip.look.motion.position[1] = Param::new(rise);
            track.clips.push(Arc::new(clip));
        }
        sequence.video.push(track);
        Ok(())
    }
}

/// A title drawn as the caption is: centered, in the captions' size and colors, with a
/// shadow only where there's no box behind it.
fn title_of(text: &str, style: CaptionStyle) -> Title {
    Title {
        text: text.to_owned(),
        size: style.size,
        color: style.color,
        shadow: style.background.is_none(),
        background: style.background,
        ..Title::default()
    }
}

/// How far below the frame's center, in hundredths of a pixel, a title centered there
/// moves for its last line to rest where a caption's would.
fn lowered(title: &Title, height: u32) -> i32 {
    let lines = title.text.lines().count().max(1) as f32;
    let block = lines * title.size as f32;
    let bottom = height as f32 * (0.5 - CAPTION_MARGIN);
    ((bottom - block / 2.0) * 100.0).round() as i32
}

fn name_of(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    let mut name: String = line.chars().take(NAME_LENGTH).collect();
    if line.chars().count() > NAME_LENGTH {
        name.push('\u{2026}');
    }
    name
}
