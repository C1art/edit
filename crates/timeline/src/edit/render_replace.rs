use crate::{
    Captions, Clip, ClipId, Edit, Gain, Generator, Marks, Sequence, SourceId, Ticks, TimelineError,
    Track,
};

impl Sequence {
    /// Whether Render and Replace renders `id`: a nest or a graphic, but not files,
    /// adjustment layers or multi-camera sources.
    pub fn can_render(&self, id: SourceId) -> bool {
        self.media_item(id)
            .is_some_and(|item| match (&item.nest, &item.generator) {
                (Some(nest), _) => !nest.multicam,
                (None, Some(generator)) => !matches!(generator, Generator::AdjustmentLayer),
                (None, None) => false,
            })
    }

    /// What Render and Replace writes out for `id` from its start: a nest as a sequence
    /// of its own, or a graphic alone on a track.
    pub fn render_view(&self, id: SourceId) -> Option<Sequence> {
        if !self.can_render(id) {
            return None;
        }
        let item = self.media_item(id)?;
        if item.nest.is_some() {
            return self.nest_view(id);
        }
        let duration = item.duration;
        let mut view = Sequence {
            video: Vec::new(),
            audio: Vec::new(),
            master: Gain::default(),
            master_effects: Vec::new(),
            submixes: Vec::new(),
            markers: Vec::new(),
            marks: Marks::default(),
            captions: Captions::default(),
            ..self.clone()
        };
        let clip = Clip::new(
            ClipId(view.ids.next()),
            id,
            Ticks::ZERO,
            duration,
            Ticks::ZERO,
        );
        view.video.push(Track {
            clips: vec![clip],
            ..Track::default()
        });
        Some(view)
    }
}

/// Shows `rendered`, a file of `original`'s pictures and sound from its start, in place
/// of `original` on the clips and their links, as Premiere's Render and Replace. Source
/// times carry over, so keyframes and markers stay put. A link of a kind the file
/// lacks is left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderAndReplace {
    pub clips: Vec<ClipId>,
    pub original: SourceId,
    pub rendered: SourceId,
}

impl Edit for RenderAndReplace {
    fn name(&self) -> &'static str {
        "Render and Replace"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let media = sequence
            .media_item(self.rendered)
            .ok_or(TimelineError::MediaNotFound(self.rendered))?;
        let (pictures, sound) = (media.video.is_some(), media.has_audio);
        let (conform, length) = (media.conform(), media.duration);
        let ids = sequence.links_of(&self.clips);
        let video = sequence.video.iter_mut().filter(|_| pictures);
        let audio = sequence.audio.iter_mut().filter(|_| sound);
        let mut replaced = false;
        for track in video.chain(audio) {
            let chosen = track
                .clips
                .iter_mut()
                .filter(|clip| ids.contains(&clip.id) && clip.source == self.original);
            for clip in chosen {
                if clip.source_out() > length {
                    return Err(TimelineError::NotEnoughSource);
                }
                clip.unrendered = Some(clip.source);
                clip.source = self.rendered;
                clip.conform = conform;
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

/// Puts back what Render and Replace replaced on the clips and their links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreUnrendered {
    pub clips: Vec<ClipId>,
}

impl Edit for RestoreUnrendered {
    fn name(&self) -> &'static str {
        "Restore Unrendered"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let ids = sequence.links_of(&self.clips);
        let originals: Vec<_> = sequence
            .tracks()
            .flat_map(|(_, track)| &track.clips)
            .filter(|clip| ids.contains(&clip.id))
            .filter_map(|clip| clip.unrendered)
            .collect();
        let conforms = originals
            .into_iter()
            .map(|original| {
                let item = sequence
                    .media_item(original)
                    .ok_or(TimelineError::MediaNotFound(original))?;
                Ok((original, item.conform()))
            })
            .collect::<Result<Vec<_>, TimelineError>>()?;
        if conforms.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        for track in sequence.video.iter_mut().chain(&mut sequence.audio) {
            let chosen = track.clips.iter_mut().filter(|clip| ids.contains(&clip.id));
            for clip in chosen {
                let Some(original) = clip.unrendered.take() else {
                    continue;
                };
                clip.source = original;
                if let Some(&(_, conform)) = conforms.iter().find(|(id, _)| *id == original) {
                    clip.conform = conform;
                }
            }
        }
        Ok(())
    }
}
