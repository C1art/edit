use crate::edit::ops;
use crate::{ClipId, Edit, MediaItem, Param, Sequence, TimelineError, TrackKind, VideoFormat};

/// How a picture of another shape is matched to the sequence frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameFit {
    /// All of the picture shows, with bars where the shapes differ.
    Fit,
    /// The frame is covered, cropping the picture where the shapes differ.
    Fill,
}

/// Sets the scale of video clips so their pictures match the sequence frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetToFrameSize {
    pub clips: Vec<ClipId>,
    pub fit: FrameFit,
}

impl Edit for SetToFrameSize {
    fn name(&self) -> &'static str {
        match self.fit {
            FrameFit::Fit => "Set to Frame Size",
            FrameFit::Fill => "Set to Fill Frame",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let frame = sequence.format;
        let mut changed = false;
        for &id in &self.clips {
            let (track, clip) = sequence.clip(id).ok_or(TimelineError::ClipNotFound(id))?;
            if track.kind != TrackKind::Video {
                continue;
            }
            let Some(picture) = sequence
                .media_item(clip.source)
                .and_then(MediaItem::played_format)
            else {
                continue;
            };
            let scale = Param::new(scale_hundredths(picture, frame, self.fit));
            let clip = ops::clip_mut(sequence, id)?;
            let motion = &mut clip.look.motion;
            changed |= motion.scale != scale || motion.scale_width != scale;
            motion.scale_width = scale.clone();
            motion.scale = scale;
        }
        if changed {
            Ok(())
        } else {
            Err(TimelineError::NothingToEdit)
        }
    }
}

/// The scale, in hundredths of a percent, that matches `picture` to `frame`.
fn scale_hundredths(picture: VideoFormat, frame: VideoFormat, fit: FrameFit) -> i32 {
    let ratio = |to: u32, from: u32| {
        let from = u64::from(from.max(1));
        (u64::from(to) * 10_000 + from / 2) / from
    };
    let across = ratio(frame.width, picture.width);
    let down = ratio(frame.height, picture.height);
    let scale = match fit {
        FrameFit::Fit => across.min(down),
        FrameFit::Fill => across.max(down),
    };
    i32::try_from(scale).unwrap_or(i32::MAX)
}
