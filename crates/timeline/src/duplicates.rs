//! Where a sequence shows the same footage more than once, which editors check for
//! before delivering.

use std::collections::HashMap;
use std::ops::Range;

use crate::{Clip, ClipId, Sequence, SourceId, Ticks};

impl Sequence {
    /// For each video clip showing source frames another video clip also shows, the
    /// sequence ranges where it does. Holds, stills and generated media repeat by design
    /// and are left out, as are remapped clips.
    pub fn duplicate_frames(&self) -> HashMap<ClipId, Vec<Range<Ticks>>> {
        let mut by_source: HashMap<SourceId, Vec<(&Clip, Range<Ticks>)>> = HashMap::new();
        for clip in self.video.iter().flat_map(|track| &track.clips) {
            let footage = self
                .media_item(clip.source)
                .is_some_and(|item| item.generator.is_none() && item.nest.is_none() && !item.still);
            if footage && !clip.hold && clip.remap.is_none() {
                let range = source_range(clip);
                by_source
                    .entry(clip.source)
                    .or_default()
                    .push((clip, range));
            }
        }
        let mut found: HashMap<ClipId, Vec<Range<Ticks>>> = HashMap::new();
        for clips in by_source.values().filter(|clips| clips.len() > 1) {
            for (index, (clip, range)) in clips.iter().enumerate() {
                for (other, other_range) in &clips[index + 1..] {
                    let shared = range.start.max(other_range.start)..range.end.min(other_range.end);
                    if shared.is_empty() {
                        continue;
                    }
                    for clip in [clip, other] {
                        found
                            .entry(clip.id)
                            .or_default()
                            .push(showing(clip, &shared));
                    }
                }
            }
        }
        found
    }
}

/// The source times `clip` shows.
fn source_range(clip: &Clip) -> Range<Ticks> {
    let ends = [clip.start, clip.end() - Ticks(1)].map(|position| clip.source_time(position));
    ends[0].min(ends[1])..ends[0].max(ends[1]) + Ticks(1)
}

/// Where in the sequence `clip` shows the source times `source`.
fn showing(clip: &Clip, source: &Range<Ticks>) -> Range<Ticks> {
    let ends = [source.start, source.end].map(|time| clip.position_of(time));
    let span = clip.start..clip.end();
    ends[0].min(ends[1]).clamp(span.start, span.end)
        ..ends[0].max(ends[1]).clamp(span.start, span.end)
}
