use std::sync::Arc;

use crate::generator::seconds;
use crate::{
    Edit, Generator, MediaItem, Sequence, SourceId, Ticks, TimelineError, GENERATED_SECONDS,
    LEADER_SECONDS,
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
        match self.generator {
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

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let item = Arc::make_mut(&mut sequence.media)
            .iter_mut()
            .find(|item| item.id == self.source)
            .ok_or(TimelineError::MediaNotFound(self.source))?;
        match &mut item.generator {
            Some(generator) if *generator != self.generator => {
                *generator = self.generator.clone();
                Ok(())
            }
            _ => Err(TimelineError::NothingToEdit),
        }
    }
}
