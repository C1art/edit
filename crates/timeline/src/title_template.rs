use serde::{Deserialize, Serialize};

use crate::{Generator, MediaItem, Sequence, SourceId, Ticks, Timebase};

/// A title or graphic kept to start new ones from, with how long its clip ran so a
/// credit roll keeps its pace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TitleTemplate {
    pub name: String,
    pub generator: Generator,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<TemplateLength>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateLength {
    pub ticks: Ticks,
    pub timebase: Timebase,
}

impl TitleTemplate {
    /// The template of the title or graphic `source` that a clip `length` long plays,
    /// or `None` when it's other media.
    pub fn from_media(
        name: String,
        sequence: &Sequence,
        source: SourceId,
        length: Ticks,
    ) -> Option<Self> {
        let item = sequence.media.iter().find(|item| item.id == source)?;
        let generator = item.generator.clone().filter(|_| Self::is_kept(item))?;
        Some(Self {
            name,
            generator,
            length: Some(TemplateLength {
                ticks: length,
                timebase: sequence.timebase,
            }),
        })
    }

    pub fn is_kept(item: &MediaItem) -> bool {
        matches!(
            item.generator,
            Some(Generator::Title(_) | Generator::Graphic(_))
        )
    }

    /// How long a clip of it runs in a sequence counting `timebase`.
    pub fn length_in(&self, timebase: Timebase) -> Option<Ticks> {
        self.length
            .map(|length| length.ticks.rescale(length.timebase, timebase))
            .filter(|&length| length > Ticks::ZERO)
    }
}
