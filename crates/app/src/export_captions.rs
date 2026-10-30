//! The export dialog's choice of what becomes of the sequence's captions, as Premiere's
//! Captions tab: burned into the picture, written beside the file, or left out.

use std::ops::Range;
use std::path::{Path, PathBuf};

use timeline::{Sequence, Ticks};

use crate::editor::Editor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptionExport {
    BurnIn,
    Sidecar,
    Omit,
}

impl CaptionExport {
    /// As the program shows them: burned in unless they're hidden.
    pub fn of(sequence: &Sequence) -> Self {
        if sequence.captions.hidden {
            Self::Omit
        } else {
            Self::BurnIn
        }
    }

    /// What it comes to for a format with or without a picture to burn into.
    pub fn for_format(self, has_video: bool) -> Self {
        match self {
            Self::BurnIn if !has_video => Self::Omit,
            choice => choice,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::BurnIn => "Burn into video",
            Self::Sidecar => "Create sidecar file",
            Self::Omit => "None",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::BurnIn => "Draw the captions on the picture, as the program shows them",
            Self::Sidecar => "Write the captions to a SubRip file of the same name beside it",
            Self::Omit => "Leave the captions out",
        }
    }
}

/// The Captions row of the export settings grid, for a sequence with captions.
pub fn row(ui: &mut egui::Ui, choice: &mut CaptionExport, has_video: bool) {
    ui.label("Captions:");
    let shown = choice.for_format(has_video);
    egui::ComboBox::from_id_salt("export captions")
        .selected_text(shown.name())
        .show_ui(ui, |ui| {
            let options = [
                CaptionExport::BurnIn,
                CaptionExport::Sidecar,
                CaptionExport::Omit,
            ];
            for option in options
                .into_iter()
                .filter(|&option| option.for_format(has_video) == option)
            {
                ui.selectable_value(choice, option, option.name())
                    .on_hover_text(option.hint());
            }
        });
    ui.end_row();
}

/// Where the sidecar of an export to `output` goes.
pub fn sidecar_path(output: &Path) -> PathBuf {
    output.with_extension("srt")
}

impl Editor {
    /// Writes the captions over `range` of the sequence, timed as the export of it is,
    /// to the sidecar of `output`.
    pub(crate) fn write_caption_sidecar(&mut self, output: &Path, range: Range<Ticks>) {
        let sequence = self.timeline.sequence();
        let (captions, timebase) = (sequence.captions.within(range), sequence.timebase);
        match &mut self.store {
            Some(store) => store.write_captions(sidecar_path(output), captions, timebase),
            None => self.notice("Cannot write the captions: the project worker is not running"),
        }
    }
}

/// What becomes of the captions of `sequence` in a format with or without a picture.
pub fn chosen(choice: CaptionExport, has_video: bool, sequence: &Sequence) -> CaptionExport {
    if sequence.captions.items.is_empty() {
        CaptionExport::Omit
    } else {
        choice.for_format(has_video)
    }
}
