use std::path::PathBuf;

use egui::Key;

use crate::editor::Editor;
use crate::effects::LutSlot;
use crate::project_files::{is_xml, AfterDiscard, Interchange, PathPurpose};

pub fn path_dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(prompt) = &mut editor.path_prompt else {
        return;
    };
    let (title, hint, action) = match prompt.purpose {
        PathPurpose::Import => (
            "Import Media",
            "Path to a video, audio or Final Cut Pro XML file (or drop files on the window):",
            "Import",
        ),
        PathPurpose::Replace(_) => (
            "Replace Footage",
            "Path to the file to use instead (clips keep their source times):",
            "Replace",
        ),
        PathPurpose::OpenProject => ("Open Project", "Path to a project file:", "Open"),
        PathPurpose::SaveProject => ("Save Project As", "Save the project to:", "Save"),
        PathPurpose::ImportCaptions => (
            "Import Captions",
            "Path to a SubRip (.srt) file; its captions replace the sequence's:",
            "Import",
        ),
        PathPurpose::ExportCaptions => (
            "Export Captions",
            "Write the sequence's captions as SubRip to:",
            "Export",
        ),
        PathPurpose::Export(Interchange::Edl) => (
            "Export EDL",
            "Write the targeted video track and audio tracks A1-A2 as a CMX 3600 EDL to:",
            "Export",
        ),
        PathPurpose::Export(Interchange::FinalCutXml) => (
            "Export Final Cut Pro XML",
            "Write the sequence as Final Cut Pro XML, for Premiere, Resolve or Final Cut, to:",
            "Export",
        ),
        PathPurpose::ExportFrame => (
            "Export Frame",
            "Save the program's frame at the playhead, at the sequence's size, as .png or .tif:",
            "Export",
        ),
        PathPurpose::Lut(_, LutSlot::Input) => (
            "Input LUT",
            "Path to a .cube LUT for the clip's source, such as a log conversion:",
            "Apply",
        ),
        PathPurpose::Lut(_, LutSlot::Look) => (
            "Creative Look",
            "Path to a .cube LUT giving the clip its look:",
            "Apply",
        ),
    };
    let mut submitted = None;
    let mut cancelled = false;
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(hint);
            ui.add(egui::TextEdit::singleline(&mut prompt.text).desired_width(480.0))
                .request_focus();
            if prompt.purpose == PathPurpose::Import {
                ui.checkbox(&mut prompt.image_sequence, "Image Sequence")
                    .on_hover_text("Import this numbered picture and those after it as one clip");
            }
            // The field keeps focus every frame, so Enter never shows up as losing it.
            let entered = ui.input(|input| input.key_pressed(Key::Enter));
            ui.horizontal(|ui| {
                if ui.button(action).clicked() || entered {
                    submitted = Some(PathBuf::from(prompt.text.trim().trim_matches('"')));
                }
                if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape))
                {
                    cancelled = true;
                }
            });
        });
    let (purpose, image_sequence) = (prompt.purpose, prompt.image_sequence);
    if let Some(path) = submitted.filter(|path| !path.as_os_str().is_empty()) {
        editor.path_prompt = None;
        match purpose {
            PathPurpose::Import if image_sequence => match media::sequence_pattern(&path) {
                Some(pattern) => editor.import(pattern, None),
                None => editor.notice(format!("{} has no frame number", path.display())),
            },
            PathPurpose::Import if is_xml(&path) => editor.import_interchange(path),
            PathPurpose::Import => editor.import(path, None),
            PathPurpose::Replace(id) => editor.import(path, Some(id)),
            PathPurpose::OpenProject => editor.discard_then(AfterDiscard::Open(path)),
            PathPurpose::SaveProject => editor.save_project_to(path),
            PathPurpose::ImportCaptions => editor.import_captions(path),
            PathPurpose::ExportCaptions => editor.export_captions(path),
            PathPurpose::Export(kind) => editor.export_interchange(path, kind),
            PathPurpose::ExportFrame => editor.export_frame(path),
            PathPurpose::Lut(clip, slot) => crate::effects::set_lut(editor, clip, slot, path),
        }
    } else if cancelled {
        editor.path_prompt = None;
        // Cancelling Save As also cancels what was waiting on the save.
        editor.after_save = None;
    }
}
