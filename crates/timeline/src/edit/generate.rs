use std::sync::Arc;

use super::ops::clip_mut;
use crate::generator::seconds;
use crate::{
    ClipId, Edit, Generator, MediaItem, Sequence, SourceId, Ticks, TimelineError,
    GENERATED_SECONDS, LEADER_SECONDS,
};

/// Adds generated media to the pool, drawn at the sequence's frame size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddGenerated {
    pub name: String,
    pub generator: Generator,
    /// How long an edit places of it when nothing is marked, instead of the standard.
    pub placed: Option<Ticks>,
}

impl Edit for AddGenerated {
    fn name(&self) -> &'static str {
        match self.generator {
            Generator::ColorMatte { .. } => "New Color Matte",
            Generator::Title(_) => "New Title",
            Generator::AdjustmentLayer => "New Adjustment Layer",
            Generator::Shape(_) => "New Shape",
            Generator::BarsAndTone => "New Bars and Tone",
            Generator::BlackVideo => "New Black Video",
            Generator::TransparentVideo => "New Transparent Video",
            Generator::CountingLeader(_) => "New Universal Counting Leader",
            Generator::Graphic(_) => "New Graphic",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let item = generated_item(sequence, self.name.clone(), self.generator.clone());
        Arc::make_mut(&mut sequence.media).push(MediaItem {
            placed: self.placed,
            ..item
        });
        Ok(())
    }
}

/// New media drawing `generator`, at the sequence's frame size and outside any bin.
pub(crate) fn generated_item(
    sequence: &mut Sequence,
    name: String,
    generator: Generator,
) -> MediaItem {
    let seconds_long = match generator {
        Generator::CountingLeader(_) => LEADER_SECONDS,
        _ => GENERATED_SECONDS,
    };
    MediaItem {
        id: SourceId(sequence.ids.next()),
        name,
        path: Default::default(),
        duration: seconds(seconds_long, sequence.timebase),
        video: Some(sequence.format),
        has_audio: matches!(
            generator,
            Generator::BarsAndTone | Generator::CountingLeader(_)
        ),
        proxy: None,
        bin: None,
        generator: Some(generator),
        nest: None,
        subclip: None,
        interpreted_rate: None,
        interpreted_aspect: None,
        interpreted_color: None,
        interpreted_fields: None,
        alpha: Default::default(),
        log: Default::default(),
        offline: false,
        still: false,
        channel_map: None,
        placed: None,
        frame_fit: None,
        timecode: None,
        poster: None,
        transcript: None,
    }
}

/// Replaces what generated media draws, changing every clip of it at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetGenerator {
    pub source: SourceId,
    pub generator: Generator,
}

impl Edit for SetGenerator {
    fn name(&self) -> &'static str {
        generator_edit_name(&self.generator)
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        set_generator(sequence, self.source, &self.generator)
    }
}

/// Replaces what one clip of generated media draws. A clip sharing its media with other
/// clips, as the halves of a cut title do, is first given a copy of its own, so editing
/// one piece leaves the rest as they were.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetClipGenerator {
    pub clip: ClipId,
    pub generator: Generator,
}

impl Edit for SetClipGenerator {
    fn name(&self) -> &'static str {
        generator_edit_name(&self.generator)
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let source = clip_mut(sequence, self.clip)?.source;
        let index = sequence
            .media
            .iter()
            .position(|item| item.id == source)
            .ok_or(TimelineError::MediaNotFound(source))?;
        let unchanged = sequence.media[index].generator.as_ref() == Some(&self.generator);
        if unchanged || sequence.media[index].generator.is_none() {
            return Err(TimelineError::NothingToEdit);
        }
        let shared = sequence
            .all_clips()
            .filter(|clip| clip.source == source || clip.unrendered == Some(source))
            .nth(1)
            .is_some();
        let source = if shared {
            let mut copy = sequence.media[index].clone();
            copy.id = SourceId(sequence.ids.next());
            let id = copy.id;
            Arc::make_mut(&mut sequence.media).insert(index + 1, copy);
            clip_mut(sequence, self.clip)?.source = id;
            id
        } else {
            source
        };
        set_generator(sequence, source, &self.generator)
    }
}

fn generator_edit_name(generator: &Generator) -> &'static str {
    match generator {
        Generator::ColorMatte { .. } => "Change Matte Color",
        Generator::Title(_) => "Edit Title",
        Generator::AdjustmentLayer => "Change Adjustment Layer",
        Generator::Shape(_) => "Edit Shape",
        Generator::BarsAndTone | Generator::BlackVideo | Generator::TransparentVideo => {
            "Change Synthetic Media"
        }
        Generator::CountingLeader(_) => "Change Counting Leader",
        Generator::Graphic(_) => "Edit Graphic",
    }
}

fn set_generator(
    sequence: &mut Sequence,
    source: SourceId,
    generator: &Generator,
) -> Result<(), TimelineError> {
    let item = Arc::make_mut(&mut sequence.media)
        .iter_mut()
        .find(|item| item.id == source)
        .ok_or(TimelineError::MediaNotFound(source))?;
    match &mut item.generator {
        Some(current) if current != generator => {
            *current = generator.clone();
            Ok(())
        }
        _ => Err(TimelineError::NothingToEdit),
    }
}
