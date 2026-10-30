use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::editor::Editor;
use crate::effects::LutSlot;
use crate::file_browser::{self, FileBrowser, FileFilter, Mode, Outcome};
use crate::native_dialog::{Ask, NativeDialog, Polled};
use crate::project_files::{is_xml, AfterDiscard, Interchange, PathPurpose};
use crate::project_store::Setting;

/// What asks for a path: the system's own dialog where there is one, or else the
/// editor's browser.
pub enum Chooser {
    Native(NativeDialog),
    Browser(Box<FileBrowser>),
}

pub fn path_dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(prompt) = &mut editor.path_prompt else {
        return;
    };
    let purpose = prompt.purpose;
    let (title, hint, action) = wording(purpose);
    let (outcome, folder) = match &mut prompt.chooser {
        Chooser::Native(dialog) => match dialog.poll() {
            Polled::Waiting => return,
            Polled::Failed => {
                let start = prompt.start.clone();
                let browser = browser_for(editor, purpose, start.as_deref());
                if let Some(prompt) = &mut editor.path_prompt {
                    prompt.chooser = Chooser::Browser(Box::new(browser));
                }
                return;
            }
            Polled::Answered(answer) => {
                prompt.image_sequence = answer.image_sequence;
                let folder = answer.paths.first().and_then(|path| path.parent());
                let folder = folder.map(Path::to_owned);
                let outcome = if answer.paths.is_empty() {
                    Outcome::Cancelled
                } else {
                    Outcome::Chosen(answer.paths)
                };
                (outcome, folder)
            }
        },
        Chooser::Browser(browser) => {
            let mut outcome = None;
            crate::dialog::window(ctx, title).show(ctx, |ui| {
                if !hint.is_empty() {
                    ui.label(hint);
                }
                let image_sequence = &mut prompt.image_sequence;
                outcome = browser.show(ui, action, |ui| {
                    if purpose == PathPurpose::Import {
                        ui.checkbox(image_sequence, "Image Sequence").on_hover_text(
                            "Import the numbered picture and those after it as one clip",
                        );
                    }
                });
            });
            let Some(outcome) = outcome else {
                return;
            };
            (outcome, browser.folder().map(Path::to_owned))
        }
    };
    let image_sequence = prompt.image_sequence;
    if let Some(folder) = folder {
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
        PathPurpose::SaveCopy => editor.save_copy_to(path),
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

/// A dialog's title, a hint shown with it, and the label of the button that chooses.
fn wording(purpose: PathPurpose) -> (&'static str, &'static str, &'static str) {
    match purpose {
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
        PathPurpose::SaveCopy => (
            "Save a Copy",
            "The open project stays the one you edit and save.",
            "Save",
        ),
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
        PathPurpose::Export(Interchange::MarkersCsv) => (
            "Export Markers",
            "Writes the sequence markers as comma-separated values, for a spreadsheet.",
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
            PathPurpose::OpenProject | PathPurpose::SaveProject | PathPurpose::SaveCopy => {
                Self::Project
            }
            PathPurpose::ImportCaptions
            | PathPurpose::ExportCaptions
            | PathPurpose::Export(_)
            | PathPurpose::ExportFrame
            | PathPurpose::ExportMedia => Self::Output,
            PathPurpose::Lut(..) => Self::Look,
        }
    }
}

/// Whether `purpose` opens or saves, and the files it lets through.
fn kind(purpose: PathPurpose) -> (Mode, Vec<FileFilter>) {
    use file_browser::*;
    match purpose {
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
        PathPurpose::SaveProject | PathPurpose::SaveCopy => (Mode::Save, vec![PROJECTS]),
        PathPurpose::ImportCaptions => (Mode::Open { many: false }, vec![SUBRIP, ALL_FILES]),
        PathPurpose::ExportCaptions => (Mode::Save, vec![SUBRIP]),
        PathPurpose::Export(Interchange::Edl) => (Mode::Save, vec![EDL]),
        PathPurpose::Export(Interchange::FinalCutXml) => (Mode::Save, vec![FINAL_CUT_XML]),
        PathPurpose::Export(Interchange::MarkersCsv) => (Mode::Save, vec![MARKERS_CSV]),
        PathPurpose::ExportFrame => (Mode::Save, vec![FRAME_IMAGES]),
        PathPurpose::ExportMedia => (Mode::Save, vec![ALL_MEDIA]),
        PathPurpose::Lut(..) => (Mode::Open { many: false }, vec![CUBE_LUT, ALL_FILES]),
        PathPurpose::ImageMatte(..) | PathPurpose::GraphicPicture(_) => {
            (Mode::Open { many: false }, vec![STILL_IMAGES, ALL_FILES])
        }
    }
}

/// How to ask for a path for `purpose`, starting at `start` if it's given.
pub fn chooser_for(editor: &Editor, purpose: PathPurpose, start: Option<&Path>) -> Chooser {
    let (mode, filters) = kind(purpose);
    let (title, _, action) = wording(purpose);
    let folder = start
        .and_then(Path::parent)
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_owned)
        .or_else(|| editor.browse_folders.get(&FolderUse::of(purpose)).cloned())
        .or_else(|| project_folder(editor));
    let file_name = start
        .filter(|_| mode == Mode::Save)
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned());
    let ask = Ask {
        title,
        action,
        mode,
        filters,
        folder,
        file_name,
        image_sequence: purpose == PathPurpose::Import,
    };
    match NativeDialog::open(ask, &editor.ctx) {
        Some(dialog) => Chooser::Native(dialog),
        None => Chooser::Browser(Box::new(browser_for(editor, purpose, start))),
    }
}

/// The editor's own browser for picking a file for `purpose`.
fn browser_for(editor: &Editor, purpose: PathPurpose, start: Option<&Path>) -> FileBrowser {
    let (mode, filters) = kind(purpose);
    let remembered = editor.browse_folders.get(&FolderUse::of(purpose)).cloned();
    FileBrowser::new(mode, filters, start, remembered, project_folder(editor))
}

fn project_folder(editor: &Editor) -> Option<PathBuf> {
    editor
        .project_path
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_owned)
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
