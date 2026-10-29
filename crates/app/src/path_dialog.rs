use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::editor::Editor;
use crate::effects::LutSlot;
use crate::file_browser::{self, FileBrowser, Outcome};
use crate::project_files::{is_xml, AfterDiscard, Interchange, PathPurpose};
use crate::project_store::Setting;

pub fn path_dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(prompt) = &mut editor.path_prompt else {
        return;
    };
    let (title, hint, action) = match prompt.purpose {
        PathPurpose::Import => (
            "Import Media",
            "Video, audio, stills or Final Cut Pro XML. Ctrl- or Shift-click to pick several, or drop files on the window.",
            "Import",
        ),
        PathPurpose::Replace(_) => (
            "Replace Footage",
            "The file to use instead. Clips keep their source times.",
            "Replace",
        ),
        PathPurpose::OpenProject => ("Open Project", "", "Open"),
        PathPurpose::SaveProject => ("Save Project As", "", "Save"),
        PathPurpose::ImportCaptions => (
            "Import Captions",
            "A SubRip (.srt) file. Its captions replace the sequence's.",
            "Import",
        ),
        PathPurpose::ExportCaptions => (
            "Export Captions",
            "Writes the sequence's captions as SubRip.",
            "Export",
        ),
        PathPurpose::Export(Interchange::Edl) => (
            "Export EDL",
            "Writes the targeted video track and audio tracks A1-A2 as a CMX 3600 EDL.",
            "Export",
        ),
        PathPurpose::Export(Interchange::FinalCutXml) => (
            "Export Final Cut Pro XML",
            "Writes the sequence as Final Cut Pro XML, for Premiere, Resolve or Final Cut.",
            "Export",
        ),
        PathPurpose::ExportMedia => ("Export Media To", "", "Choose"),
        PathPurpose::ExportFrame => (
            "Export Frame",
            "Saves the program's frame at the playhead, at the sequence's size, as PNG or TIFF.",
            "Export",
        ),
        PathPurpose::Lut(_, LutSlot::Input) => (
            "Input LUT",
            "A .cube LUT for the clip's source, such as a log conversion.",
            "Apply",
        ),
        PathPurpose::ImageMatte(..) => (
            "Image Matte",
            "A still image whose alpha or luma shows the clip through.",
            "Apply",
        ),
        PathPurpose::GraphicPicture(_) => (
            "New Layer From File",
            "A picture, such as a logo, to add over the graphic.",
            "Add",
        ),
        PathPurpose::Lut(_, LutSlot::Look) => (
            "Creative Look",
            "A .cube LUT giving the clip its look.",
            "Apply",
        ),
    };
    let mut outcome = None;
    crate::dialog::window(ctx, title).show(ctx, |ui| {
        if !hint.is_empty() {
            ui.label(hint);
        }
        let (purpose, image_sequence) = (prompt.purpose, &mut prompt.image_sequence);
        outcome = prompt.browser.show(ui, action, |ui| {
            if purpose == PathPurpose::Import {
                ui.checkbox(image_sequence, "Image Sequence")
                    .on_hover_text("Import the numbered picture and those after it as one clip");
            }
        });
    });
    let (purpose, image_sequence) = (prompt.purpose, prompt.image_sequence);
    let Some(outcome) = outcome else {
        return;
    };
    if let Some(folder) = prompt.browser.folder() {
        let folder = folder.to_owned();
        let previous = editor
            .browse_folders
            .insert(FolderUse::of(purpose), folder.clone());
        if previous != Some(folder) {
            if let Some(store) = &mut editor.store {
                store.write_setting(Setting::Folders, folders_to_text(&editor.browse_folders));
            }
        }
    }
    editor.path_prompt = None;
    let paths = match outcome {
        Outcome::Chosen(paths) => paths,
        Outcome::Cancelled => {
            // Cancelling Save As also cancels what was waiting on the save.
            editor.after_save = None;
            return;
        }
    };
    if purpose == PathPurpose::Import {
        import(editor, paths, image_sequence);
        return;
    }
    let Some(path) = paths.into_iter().next() else {
        return;
    };
    match purpose {
        PathPurpose::Import => {}
        PathPurpose::Replace(id) => editor.import(path, Some(id)),
        PathPurpose::OpenProject => editor.discard_then(AfterDiscard::Open(path)),
        PathPurpose::SaveProject => editor.save_project_to(path),
        PathPurpose::ImportCaptions => editor.import_captions(path),
        PathPurpose::ExportCaptions => editor.export_captions(path),
        PathPurpose::Export(kind) => editor.export_interchange(path, kind),
        PathPurpose::ExportFrame => editor.export_frame(path),
        PathPurpose::ExportMedia => {
            if let Some(export) = &mut editor.export {
                export.set_output(&path);
            }
        }
        PathPurpose::Lut(clip, slot) => crate::effects::set_lut(editor, clip, slot, path),
        PathPurpose::ImageMatte(clip, effect) => {
            crate::effects::set_image_matte(editor, clip, effect, path);
        }
        PathPurpose::GraphicPicture(source) => {
            crate::effects::add_graphic_picture(editor, source, path);
        }
    }
}

fn import(editor: &mut Editor, paths: Vec<PathBuf>, image_sequence: bool) {
    for path in paths {
        if image_sequence {
            match media::sequence_pattern(&path) {
                Some(pattern) => editor.import(pattern, None),
                None => editor.notice(format!(
                    "{} has no frame number",
                    crate::project_files::quoted_name(&path)
                )),
            }
        } else if is_xml(&path) {
            editor.import_interchange(path);
        } else {
            editor.import(path, None);
        }
    }
}

/// Which folder a browser for each purpose opens in, the last one used for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FolderUse {
    Media,
    Project,
    Output,
    Look,
}

impl FolderUse {
    const ALL: [Self; 4] = [Self::Media, Self::Project, Self::Output, Self::Look];

    fn name(self) -> &'static str {
        match self {
            Self::Media => "media",
            Self::Project => "project",
            Self::Output => "output",
            Self::Look => "look",
        }
    }

    fn of(purpose: PathPurpose) -> Self {
        match purpose {
            PathPurpose::Import
            | PathPurpose::Replace(_)
            | PathPurpose::ImageMatte(..)
            | PathPurpose::GraphicPicture(_) => Self::Media,
            PathPurpose::OpenProject | PathPurpose::SaveProject => Self::Project,
            PathPurpose::ImportCaptions
            | PathPurpose::ExportCaptions
            | PathPurpose::Export(_)
            | PathPurpose::ExportFrame
            | PathPurpose::ExportMedia => Self::Output,
            PathPurpose::Lut(..) => Self::Look,
        }
    }
}

/// A browser for picking a file for `purpose`, starting at `start` if it's given.
pub fn browser_for(editor: &Editor, purpose: PathPurpose, start: Option<&Path>) -> FileBrowser {
    use file_browser::*;
    let (mode, filters) = match purpose {
        PathPurpose::Import => (
            Mode::Open { many: true },
            vec![
                ALL_MEDIA,
                VIDEO_FILES,
                AUDIO_FILES,
                STILL_IMAGES,
                FINAL_CUT_XML,
                ALL_FILES,
            ],
        ),
        PathPurpose::Replace(_) => (
            Mode::Open { many: false },
            vec![ALL_MEDIA, VIDEO_FILES, AUDIO_FILES, STILL_IMAGES, ALL_FILES],
        ),
        PathPurpose::OpenProject => (Mode::Open { many: false }, vec![PROJECTS, ALL_FILES]),
        PathPurpose::SaveProject => (Mode::Save, vec![PROJECTS]),
        PathPurpose::ImportCaptions => (Mode::Open { many: false }, vec![SUBRIP, ALL_FILES]),
        PathPurpose::ExportCaptions => (Mode::Save, vec![SUBRIP]),
        PathPurpose::Export(Interchange::Edl) => (Mode::Save, vec![EDL]),
        PathPurpose::Export(Interchange::FinalCutXml) => (Mode::Save, vec![FINAL_CUT_XML]),
        PathPurpose::ExportFrame => (Mode::Save, vec![FRAME_IMAGES]),
        PathPurpose::ExportMedia => (Mode::Save, vec![ALL_MEDIA]),
        PathPurpose::Lut(..) => (Mode::Open { many: false }, vec![CUBE_LUT, ALL_FILES]),
        PathPurpose::ImageMatte(..) | PathPurpose::GraphicPicture(_) => {
            (Mode::Open { many: false }, vec![STILL_IMAGES, ALL_FILES])
        }
    };
    let remembered = editor.browse_folders.get(&FolderUse::of(purpose)).cloned();
    let project_folder = editor
        .project_path
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_owned);
    FileBrowser::new(mode, filters, start, remembered, project_folder)
}

/// One `use<TAB>folder` line each, in a fixed order so the file only changes with them.
pub fn folders_to_text(folders: &HashMap<FolderUse, PathBuf>) -> String {
    FolderUse::ALL
        .iter()
        .filter_map(|usage| {
            let folder = folders.get(usage)?;
            Some(format!("{}\t{}\n", usage.name(), folder.display()))
        })
        .collect()
}

/// Lines it doesn't know are skipped, so a damaged file only forgets folders.
pub fn folders_from_text(text: &str) -> HashMap<FolderUse, PathBuf> {
    text.lines()
        .filter_map(|line| {
            let (name, folder) = line.split_once('\t')?;
            let usage = FolderUse::ALL
                .into_iter()
                .find(|usage| usage.name() == name)?;
            Some((usage, PathBuf::from(folder)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembered_folders_survive_the_text_file_and_bad_lines_are_skipped() {
        let folders = HashMap::from([
            (FolderUse::Media, PathBuf::from("D:\\Shoot 2\\Day\tone")),
            (FolderUse::Look, PathBuf::from("C:\\Looks")),
        ]);
        let text = format!("{}nonsense\nzoom\tC:\\\n", folders_to_text(&folders));
        assert_eq!(folders_from_text(&text), folders);
    }
}
