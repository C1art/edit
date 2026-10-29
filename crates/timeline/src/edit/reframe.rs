use crate::{AutoReframe, Edit, ReframeMotion, Sequence, TimelineError, VideoEffect};

/// Reshapes the sequence's frame and gives each clip of video footage an Auto Reframe
/// following its subject, as Premiere's Auto Reframe Sequence does. Clips that already
/// have one keep it and take the new motion preset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReframeSequence {
    pub width: u32,
    pub height: u32,
    pub motion: ReframeMotion,
}

impl Edit for ReframeSequence {
    fn name(&self) -> &'static str {
        "Auto Reframe Sequence"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.width == 0 || self.height == 0 {
            return Err(TimelineError::NothingToEdit);
        }
        sequence.format.width = self.width;
        sequence.format.height = self.height;
        let footage: Vec<_> = sequence
            .media
            .iter()
            .filter(|item| item.is_file() && item.video.is_some())
            .map(|item| item.id)
            .collect();
        let timebase = sequence.timebase;
        let clips = sequence.video.iter_mut().flat_map(|track| &mut track.clips);
        for clip in clips.filter(|clip| footage.contains(&clip.source)) {
            let effects = &mut clip.look.effects;
            let existing = effects.iter_mut().rev().find_map(|effect| match effect {
                VideoEffect::AutoReframe(reframe) => Some(reframe),
                _ => None,
            });
            match existing {
                Some(reframe) => {
                    reframe.motion = self.motion;
                    reframe.retune(timebase);
                }
                None => effects.push(VideoEffect::AutoReframe(AutoReframe {
                    motion: self.motion,
                    ..AutoReframe::default()
                })),
            }
        }
        Ok(())
    }
}
