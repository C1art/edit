use super::ops::{self, Relinks};
use crate::{
    Clip, ClipId, Edit, LinkId, MediaItem, Nest, Sequence, SourceId, Ticks, TimelineError, Track,
};

/// Makes a multi-camera source of media items: each one with a picture becomes a camera
/// on its own video track, all starting together. Every camera's sound is kept on a track
/// of its own, but only the first is heard until another track is unmuted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateMulticam {
    pub sources: Vec<SourceId>,
    pub name: String,
}

impl Edit for CreateMulticam {
    fn name(&self) -> &'static str {
        "Create Multi-Camera Source Sequence"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let mut nest = Nest {
            multicam: true,
            ..Nest::default()
        };
        let mut bin = None;
        for &source in &self.sources {
            let item = sequence
                .media_item(source)
                .ok_or(TimelineError::MediaNotFound(source))?;
            if item.video.is_none() || item.nest.is_some() || item.generator.is_some() {
                continue;
            }
            let (duration, has_audio) = (item.duration, item.has_audio);
            bin = bin.or(item.bin);
            let link = has_audio.then(|| LinkId(sequence.ids.next()));
            let mut camera = || {
                let mut clip = Clip::new(
                    ClipId(sequence.ids.next()),
                    source,
                    Ticks::ZERO,
                    duration,
                    Ticks::ZERO,
                );
                clip.link = link;
                Track {
                    clips: vec![clip],
                    ..Track::default()
                }
            };
            nest.video.push(camera());
            if has_audio {
                let mut sound = camera();
                sound.muted = !nest.audio.is_empty();
                nest.audio.push(sound);
            }
        }
        if nest.video.len() < 2 {
            return Err(TimelineError::NothingToEdit);
        }
        let id = SourceId(sequence.ids.next());
        sequence.media.push(MediaItem {
            id,
            name: self.name.clone(),
            path: Default::default(),
            duration: Ticks::ZERO,
            video: Some(sequence.format),
            has_audio: false,
            proxy: None,
            generator: None,
            nest: Some(nest),
            bin,
        });
        sequence.fit_nest(id);
        Ok(())
    }
}

/// Shows another camera of a multi-camera clip. With `at` inside the clip, the clip and
/// those linked to it are cut there first and only the part after switches, as when
/// cutting between cameras while the sequence plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchAngle {
    pub clip: ClipId,
    pub angle: usize,
    pub at: Option<Ticks>,
}

impl Edit for SwitchAngle {
    fn name(&self) -> &'static str {
        "Switch Camera"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let (track, clip) = sequence
            .clip(self.clip)
            .ok_or(TimelineError::ClipNotFound(self.clip))?;
        let angles = sequence
            .multicam_of(clip)
            .map_or(0, |nest| nest.video.len());
        if self.angle >= angles {
            return Err(TimelineError::NothingToEdit);
        }
        let mut switched = self.clip;
        if let Some(at) = self.at.filter(|&at| clip.start < at && at < clip.end()) {
            if clip.angle == self.angle {
                return Err(TimelineError::NothingToEdit);
            }
            let tracks: Vec<_> = sequence
                .with_links(&[self.clip])
                .into_iter()
                .filter_map(|id| sequence.clip(id).map(|(track, _)| track))
                .collect();
            ops::split(sequence, &tracks, at, &mut Relinks::default());
            switched = sequence
                .track(track)
                .and_then(|track| track.clip_at(at))
                .map(|clip| clip.id)
                .ok_or(TimelineError::ClipNotFound(self.clip))?;
        }
        let clip = ops::clip_mut(sequence, switched)?;
        if clip.angle == self.angle {
            return Err(TimelineError::NothingToEdit);
        }
        clip.angle = self.angle;
        Ok(())
    }
}
