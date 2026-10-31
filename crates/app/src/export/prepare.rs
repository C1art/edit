use std::path::Path;
use std::sync::Arc;

use playback::ExportSettings;
use timeline::{Sequence, Ticks};

use super::{ExportDialog, Span, EMPTY_SEQUENCE};
use crate::editor::Editor;
use crate::export_captions::{self, CaptionExport};
use crate::export_format::Format;
use crate::export_overlays;
use crate::export_target::first_frame;
use crate::project_files::quoted_name;
use crate::same_path::same_path;

/// The sequence and settings the open dialog would export.
pub(super) fn prepare_open(editor: &mut Editor) -> Option<(Arc<Sequence>, ExportSettings)> {
    let dialog = editor.export.take()?;
    let prepared = prepare(editor, &dialog);
    editor.export = Some(dialog);
    prepared
}

/// The sequence and settings `dialog` would export, after telling the user what's
/// wrong if it can't.
pub(crate) fn prepare(
    editor: &mut Editor,
    dialog: &ExportDialog,
) -> Option<(Arc<Sequence>, ExportSettings)> {
    let path = dialog.output();
    let sequence = editor.timeline.sequence();
    let sequence_size = (sequence.format.width, sequence.format.height);
    let (width, height) = dialog.format.size(sequence_size);
    let range = (dialog.span == Span::InToOut && sequence.marks.is_set())
        .then(|| sequence.marks.range(sequence.duration()));
    let has_video = dialog.format.format.has_video();
    let captions = export_captions::chosen(dialog.captions, has_video, sequence);
    let overlays = Some(dialog.overlays).filter(|overlays| has_video && !overlays.is_empty());
    let differs = export_overlays::differs_from_program(&dialog.overlays, captions, sequence);
    let previews = match &editor.render_previews {
        Some(previews) if dialog.use_previews && !differs => {
            previews.previews(editor.timeline.revision())
        }
        _ => Vec::new(),
    };
    let sidecar =
        (captions == CaptionExport::Sidecar).then(|| export_captions::sidecar_path(&path));
    // The project may read an image sequence the export would write over.
    let frames = match dialog.format.format {
        Format::Stills(format) => media::sequence_pattern(&first_frame(&path, format)),
        _ => None,
    };
    let problem = if path.as_os_str().is_empty() {
        Some("Choose an output file first".to_owned())
    } else if path.is_relative() {
        // It would land wherever the editor happened to be started from.
        Some("Give the output file's whole path, with its folder".to_owned())
    } else if let Some(taken) = std::iter::once(&path)
        .chain(&sidecar)
        .chain(&frames)
        .find(|path| editor.is_project_file(path))
    {
        Some(format!("{} is part of the project", quoted_name(taken)))
    } else if let Some(queued) = editor
        .export_queue
        .pending_paths()
        .into_iter()
        .chain(
            editor
                .export
                .as_ref()
                .and_then(ExportDialog::running_path)
                .map(Path::to_owned),
        )
        .find(|queued| {
            std::iter::once(&path)
                .chain(&sidecar)
                .any(|ours| same_path(queued, ours))
        })
    {
        Some(format!(
            "Another export already writes {}",
            quoted_name(&queued)
        ))
    } else if sequence.duration() == timeline::Ticks::ZERO {
        Some(EMPTY_SEQUENCE.to_owned())
    } else if has_video && !(width.is_multiple_of(2) && height.is_multiple_of(2)) {
        Some(format!("The frame size must be even, not {width}×{height}"))
    } else if range
        .as_ref()
        .is_some_and(|range| range.is_empty() || range.start >= sequence.duration())
    {
        Some("The in mark is past the end of the sequence".to_owned())
    } else {
        None
    };
    if let Some(problem) = problem {
        editor.notice(format!("Cannot export: {problem}"));
        return None;
    }
    let settings = ExportSettings {
        path,
        video: dialog.format.video(sequence_size),
        audio: dialog.format.audio(),
        range,
        previews,
        loudness: dialog.format.loudness.filter(|_| dialog.format.has_audio()),
    };
    let mut sequence = sequence.clone();
    sequence.captions.hidden = captions != CaptionExport::BurnIn;
    let span = (settings.range.clone()).unwrap_or(Ticks::ZERO..sequence.duration());
    if let Some(overlays) = overlays {
        sequence.add_export_overlays(overlays, span.clone());
    }
    if sidecar.is_some() {
        editor.write_caption_sidecar(&settings.path, span);
    }
    Some((Arc::new(sequence), settings))
}
