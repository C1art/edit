use crate::edit::attributes::each_clip;
use crate::motion_preset::Frames;
use crate::{
    AudioEffect, ClipId, Edit, EffectPreset, MotionPreset, Sequence, TimelineError, TrackKind,
    VideoEffect,
};

/// An effect or preset from the Effects panel, for the clips of its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipEffect {
    Video(Box<VideoEffect>),
    Audio(Box<AudioEffect>),
    Preset(EffectPreset),
    /// Keyframes a picture's motion, opacity or blur.
    Motion(MotionPreset),
}

/// Adds an effect after the own effects of `clips` and of the clips linked to them,
/// wherever it suits the track: a video effect dropped on a clip's audio still reaches
/// its picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyEffect {
    pub clips: Vec<ClipId>,
    pub effect: ClipEffect,
}

impl Edit for ApplyEffect {
    fn name(&self) -> &'static str {
        match self.effect {
            ClipEffect::Preset(_) | ClipEffect::Motion(_) => "Apply Preset",
            ClipEffect::Video(_) | ClipEffect::Audio(_) => "Apply Effect",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let frames = Frames {
            rate: sequence.format.frame_rate,
            timebase: sequence.timebase,
            width: sequence.format.width,
        };
        each_clip(sequence, &self.clips, |kind, clip| {
            match (&self.effect, kind) {
                (ClipEffect::Video(effect), TrackKind::Video) => {
                    clip.look.effects.push(VideoEffect::clone(effect));
                }
                (ClipEffect::Audio(effect), TrackKind::Audio) => {
                    clip.audio_effects.push(AudioEffect::clone(effect));
                }
                (ClipEffect::Preset(preset), kind) => preset.apply(kind, clip),
                (ClipEffect::Motion(preset), TrackKind::Video) => preset.apply(clip, frames),
                _ => {}
            }
        })
    }
}
