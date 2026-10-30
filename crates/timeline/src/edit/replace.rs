use std::sync::Arc;

use crate::{ClipId, Edit, Sequence, SourceId, Ticks, TimelineError};

/// Puts other media under a clip and the clips linked to it, keeping their place, length
/// and effects, as Premiere's Replace With Clip. The clip now reads from `source_in`,
/// and its links keep their offsets from it; a link of a kind the media lacks is left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceClip {
    pub clip: ClipId,
    pub source: SourceId,
    pub source_in: Ticks,
}

impl Edit for ReplaceClip {
    fn name(&self) -> &'static str {
        "Replace With Clip"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let media = sequence
            .media_item(self.source)
            .ok_or(TimelineError::MediaNotFound(self.source))?;
        let (pictures, sound) = (media.video.is_some(), media.has_audio);
        let (conform, length) = (media.conform(), media.duration);
        let (_, clip) = sequence
            .clip(self.clip)
            .ok_or(TimelineError::ClipNotFound(self.clip))?;
        if clip.source == self.source && clip.source_in == self.source_in {
            return Err(TimelineError::NothingToEdit);
        }
        let shift = self.source_in - clip.source_in;
        let ids = sequence.with_links(&[self.clip]);
        let video = sequence.video.iter_mut().filter(|_| pictures);
        let audio = sequence.audio.iter_mut().filter(|_| sound);
        let mut replaced = false;
        for track in video.chain(audio) {
            let chosen = track.clips.iter_mut().filter(|clip| ids.contains(&clip.id));
            for clip in chosen.map(Arc::make_mut) {
                clip.source = self.source;
                clip.source_in = clip.source_in + shift;
                clip.conform = conform;
                // They marked moments in the old media.
                clip.markers.clear();
                if clip.source_in < Ticks::ZERO || clip.source_out() > length {
                    return Err(TimelineError::NotEnoughSource);
                }
                replaced = true;
            }
        }
        if replaced {
            Ok(())
        } else {
            Err(TimelineError::NothingToEdit)
        }
    }
}
