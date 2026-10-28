use crate::generator::seconds;
use crate::{Edit, Generator, MediaItem, Sequence, SourceId, TimelineError, GENERATED_SECONDS};

/// Adds generated media to the pool, drawn at the sequence's frame size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddGenerated {
    pub name: String,
    pub generator: Generator,
}

impl Edit for AddGenerated {
    fn name(&self) -> &'static str {
        match self.generator {
            Generator::ColorMatte { .. } => "New Color Matte",
            Generator::Title(_) => "New Title",
            Generator::AdjustmentLayer => "New Adjustment Layer",
            Generator::Shape(_) => "New Shape",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let id = SourceId(sequence.ids.next());
        sequence.media.push(MediaItem {
            id,
            name: self.name.clone(),
            path: Default::default(),
            duration: seconds(GENERATED_SECONDS, sequence.timebase),
            video: Some(sequence.format),
            has_audio: false,
            proxy: None,
            bin: None,
            generator: Some(self.generator.clone()),
            nest: None,
            subclip: None,
            interpreted_rate: None,
            log: Default::default(),
        });
        Ok(())
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
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let item = sequence
            .media
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
