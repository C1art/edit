use std::collections::HashSet;
use std::ops::Range;

use crate::{Clip, ColorCorrection, Look, Sequence, SourceId, Ticks, Track};

/// The stretches between consecutive video edit points whose pictures are too costly to
/// play live, which Premiere marks red above the timeline and renders previews of.
pub fn render_segments(sequence: &Sequence) -> Vec<Range<Ticks>> {
    let visible: Vec<&Track> = sequence.video.iter().filter(|t| !t.muted).collect();
    let mut points: Vec<Ticks> = visible
        .iter()
        .flat_map(|track| track.clips.iter().filter(|clip| !clip.disabled))
        .flat_map(|clip| [clip.start, clip.end()])
        .collect();
    points.sort_unstable();
    points.dedup();
    points
        .windows(2)
        .map(|pair| pair[0]..pair[1])
        .filter(|segment| costly(sequence, &visible, segment))
        .collect()
}

fn costly(sequence: &Sequence, visible: &[&Track], segment: &Range<Ticks>) -> bool {
    let mut layers = 0;
    for track in visible {
        if let Some(clip) = track.clip_at(segment.start).filter(|clip| !clip.disabled) {
            layers += 1;
            if costly_clip(sequence, clip) {
                return true;
            }
        }
        // An incoming clip's transition starts before its cut.
        let incoming = track.clips.iter().find(|clip| clip.start == segment.end);
        if incoming.is_some_and(|clip| clip.transition_in.is_some()) {
            return true;
        }
    }
    layers > 1
}

/// Motion, opacity and crop cost the compositor next to nothing; effects, grades,
/// masks, transitions, retiming and nests all cost decoding or shading.
fn costly_clip(sequence: &Sequence, clip: &Clip) -> bool {
    let look = &clip.look;
    let nested = sequence
        .media_item(clip.source)
        .is_some_and(|media| media.nest.is_some());
    !look.effects.is_empty()
        || look.color != ColorCorrection::default()
        || !look.masks.is_empty()
        || look.blend != Look::default().blend
        || clip.transition_in.is_some()
        || clip.transition_out.is_some()
        || !clip.rate().is_normal()
        || clip.reverse
        || clip.remap.is_some()
        || !clip.interpolation.is_sampling()
        || nested
}

/// What decides the pictures of `segment`: the video tracks' clips there and either side
/// of it, for transitions, the captions over it and the media they show. Its preview
/// can be reused for as long as this is unchanged, as after undoing an edit.
pub fn segment_content(sequence: &Sequence, segment: &Range<Ticks>) -> Sequence {
    let mut content = Sequence::new(sequence.timebase, sequence.format);
    content.video = sequence
        .video
        .iter()
        .map(|track| Track {
            clips: around(track, segment).to_vec(),
            muted: track.muted,
            ..Track::default()
        })
        .collect();
    content.captions.style = sequence.captions.style;
    content.captions.hidden = sequence.captions.hidden;
    content.captions.items = sequence
        .captions
        .items
        .iter()
        .filter(|caption| caption.start < segment.end && segment.start < caption.end)
        .cloned()
        .collect();
    let used: HashSet<SourceId> = content
        .video
        .iter()
        .flat_map(|track| track.clips.iter().map(|clip| clip.source))
        .collect();
    // A nest's tracks draw on the whole pool.
    let nests = sequence
        .media
        .iter()
        .any(|media| used.contains(&media.id) && media.nest.is_some());
    content.media = sequence
        .media
        .iter()
        .filter(|media| nests || used.contains(&media.id))
        .map(|media| {
            let mut media = media.clone();
            // Neither changes a picture.
            media.name.clear();
            media.bin = None;
            media.proxy = None;
            media
        })
        .collect();
    content
}

/// The clips overlapping `segment` and their neighbours on either side.
fn around<'a>(track: &'a Track, segment: &Range<Ticks>) -> &'a [Clip] {
    let first = track
        .clips
        .partition_point(|clip| clip.end() <= segment.start);
    let last = track.clips.partition_point(|clip| clip.start < segment.end);
    &track.clips[first.saturating_sub(1)..(last + 1).min(track.clips.len())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipId, Timebase, VideoEffect, VideoFormat};

    fn clip(id: u64, start: i64, duration: i64) -> Clip {
        Clip::new(
            ClipId(id),
            SourceId(1),
            Ticks(start),
            Ticks(duration),
            Ticks(0),
        )
    }

    fn sequence(tracks: Vec<Vec<Clip>>) -> Sequence {
        let mut sequence = Sequence::new(Timebase::HZ_90K, VideoFormat::default());
        sequence.video = tracks
            .into_iter()
            .map(|clips| Track {
                clips,
                ..Track::default()
            })
            .collect();
        sequence
    }

    #[test]
    fn only_stacked_or_affected_stretches_need_rendering() {
        let mut blurred = clip(2, 300, 100);
        blurred.look.effects.push(VideoEffect::HorizontalFlip);
        let sequence = sequence(vec![
            vec![clip(1, 0, 300), blurred],
            vec![clip(3, 100, 100)],
        ]);
        assert_eq!(
            render_segments(&sequence),
            [Ticks(100)..Ticks(200), Ticks(300)..Ticks(400)]
        );
    }

    #[test]
    fn an_edit_elsewhere_keeps_a_segments_content() {
        let mut sequence = sequence(vec![vec![clip(1, 0, 100), clip(2, 100, 100)]; 2]);
        let segment = Ticks(0)..Ticks(100);
        let before = segment_content(&sequence, &segment);
        sequence.video[0].clips.push(clip(4, 500, 100));
        assert_eq!(segment_content(&sequence, &segment), before);
        sequence.video[1].clips[1].look.opacity = crate::Param::new(5000);
        assert_ne!(segment_content(&sequence, &segment), before);
    }
}
