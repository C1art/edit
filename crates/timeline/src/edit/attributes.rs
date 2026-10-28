use crate::edit::ops;
use crate::{Clip, ClipId, Edit, Look, Param, Sequence, TimelineError, TrackKind};

/// A group of clip settings that Paste Attributes and Remove Attributes treat as one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Attribute {
    Motion,
    /// Opacity with its blend mode and masks.
    Opacity,
    Crop,
    Effects,
    Color,
    Volume,
    AudioEffects,
}

impl Attribute {
    pub const ALL: [Self; 7] = [
        Self::Motion,
        Self::Opacity,
        Self::Crop,
        Self::Effects,
        Self::Color,
        Self::Volume,
        Self::AudioEffects,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Motion => "Motion",
            Self::Opacity => "Opacity",
            Self::Crop => "Crop",
            Self::Effects => "Video Effects",
            Self::Color => "Lumetri Color",
            Self::Volume => "Volume",
            Self::AudioEffects => "Audio Effects",
        }
    }

    pub fn kind(self) -> TrackKind {
        match self {
            Self::Volume | Self::AudioEffects => TrackKind::Audio,
            _ => TrackKind::Video,
        }
    }
}

/// Gives `clips` and the clips linked to them the `attributes` of `video` or `audio`,
/// whichever matches their track. Effects are added after a clip's own; keyframes keep
/// their distance from the clip's start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteAttributes {
    pub video: Option<Clip>,
    pub audio: Option<Clip>,
    pub clips: Vec<ClipId>,
    pub attributes: Vec<Attribute>,
}

impl Edit for PasteAttributes {
    fn name(&self) -> &'static str {
        "Paste Attributes"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        each_clip(sequence, &self.clips, |kind, clip| {
            let from = match kind {
                TrackKind::Video => self.video.as_ref(),
                TrackKind::Audio => self.audio.as_ref(),
            };
            if let Some(from) = from {
                for &attribute in self.attributes.iter().filter(|a| a.kind() == kind) {
                    paste(attribute, from, clip);
                }
            }
        })
    }
}

/// Puts `attributes` of `clips` and the clips linked to them back to their defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveAttributes {
    pub clips: Vec<ClipId>,
    pub attributes: Vec<Attribute>,
}

impl Edit for RemoveAttributes {
    fn name(&self) -> &'static str {
        "Remove Attributes"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        each_clip(sequence, &self.clips, |kind, clip| {
            let look = Look::default();
            for &attribute in self.attributes.iter().filter(|a| a.kind() == kind) {
                match attribute {
                    Attribute::Motion => clip.look.motion = look.motion.clone(),
                    Attribute::Opacity => {
                        clip.look.opacity = look.opacity.clone();
                        clip.look.blend = look.blend;
                        clip.look.masks.clear();
                    }
                    Attribute::Crop => clip.look.crop = look.crop.clone(),
                    Attribute::Effects => clip.look.effects.clear(),
                    Attribute::Color => clip.look.color = look.color.clone(),
                    Attribute::Volume => clip.volume = Param::default(),
                    Attribute::AudioEffects => clip.audio_effects.clear(),
                }
            }
        })
    }
}

/// Runs `change` on `clips` and their linked clips, failing when it changes none.
fn each_clip(
    sequence: &mut Sequence,
    clips: &[ClipId],
    mut change: impl FnMut(TrackKind, &mut Clip),
) -> Result<(), TimelineError> {
    let mut changed = false;
    for id in sequence.with_links(clips) {
        let (track, _) = sequence.clip(id).ok_or(TimelineError::ClipNotFound(id))?;
        let clip = ops::clip_mut(sequence, id)?;
        let before = clip.clone();
        change(track.kind, clip);
        changed |= *clip != before;
    }
    if changed {
        Ok(())
    } else {
        Err(TimelineError::NothingToEdit)
    }
}

fn paste(attribute: Attribute, from: &Clip, to: &mut Clip) {
    let timing = to.clone();
    // Keyframes are kept in source time, which differs between the two clips.
    let retime = |param: &mut Param| {
        for key in &mut param.keys {
            let offset = from.position_of(key.time) - from.start;
            key.time = timing.source_time(timing.start + offset);
        }
        param.keys.sort_by_key(|key| key.time);
    };
    let look = &mut to.look;
    match attribute {
        Attribute::Motion => {
            look.motion = from.look.motion.clone();
            look.motion.params_mut().for_each(retime);
        }
        Attribute::Opacity => {
            look.opacity = from.look.opacity.clone();
            look.blend = from.look.blend;
            look.masks = from.look.masks.clone();
            retime(&mut look.opacity);
            look.masks
                .iter_mut()
                .flat_map(|mask| mask.params_mut())
                .for_each(retime);
        }
        Attribute::Crop => {
            look.crop = from.look.crop.clone();
            look.crop.params_mut().into_iter().for_each(retime);
        }
        Attribute::Effects => {
            let mut effects = from.look.effects.clone();
            effects
                .iter_mut()
                .flat_map(|effect| effect.params_mut())
                .for_each(retime);
            look.effects.extend(effects);
        }
        Attribute::Color => {
            look.color = from.look.color.clone();
            look.color.params_mut().for_each(retime);
        }
        Attribute::Volume => {
            to.volume = from.volume.clone();
            retime(&mut to.volume);
        }
        Attribute::AudioEffects => {
            let mut effects = from.audio_effects.clone();
            effects
                .iter_mut()
                .flat_map(|effect| effect.params_mut())
                .for_each(retime);
            to.audio_effects.extend(effects);
        }
    }
}
