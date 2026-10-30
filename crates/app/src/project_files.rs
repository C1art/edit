use std::path::{Path, PathBuf};

use egui::{Key, ViewportCommand};
use timeline::{
    Captions, ClipId, Sequence, SetCaptions, SourceId, Timebase, Timeline, VideoFormat,
};

use crate::editor::Editor;
use crate::effects::LutSlot;
use crate::export::ExportDialog;
use crate::path_dialog::Chooser;
use crate::project_store::{Setting, StoreEvent};
use crate::recent_projects::RecentProjects;

/// What a picked path is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathPurpose {
    Import,
    /// A file to take the place of a media item's.
    Replace(SourceId),
    OpenProject,
    SaveProject,
    /// Where to write the project without it becoming the one being edited.
    SaveCopy,
    ImportCaptions,
    ExportCaptions,
    Export(Interchange),
    /// An image of the program at the playhead.
    ExportFrame,
    /// Where Export Media writes the sequence.
    ExportMedia,
    /// A `.cube` file for one of a clip's LUTs.
    Lut(ClipId, LutSlot),
    /// The picture for the Image Matte Key at an index in a clip's effects.
    ImageMatte(ClipId, usize),
    /// A picture to add as the top layer of a graphic.
    GraphicPicture(SourceId),
}

pub struct PathPrompt {
    pub purpose: PathPurpose,
    pub chooser: Chooser,
    /// Where the dialog started, kept to start the editor's browser there if the
    /// system's dialog can't be shown.
    pub start: Option<PathBuf>,
    /// Whether an import takes the numbered pictures that follow the file as one clip.
    pub image_sequence: bool,
}

/// A sequence description another editor reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interchange {
    Edl,
    /// Final Cut Pro 7 XML, which Premiere and Resolve also import.
    FinalCutXml,
    /// The sequence markers, for a spreadsheet or a client's notes.
    MarkersCsv,
}

impl Interchange {
    fn extension(self) -> &'static str {
        match self {
            Self::Edl => EDL,
            Self::FinalCutXml => XML,
            Self::MarkersCsv => CSV,
        }
    }
}

/// What to do once unsaved changes are saved or discarded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AfterDiscard {
    NewProject,
    Open(PathBuf),
    Quit,
}

const SUBRIP: &str = "srt";
const EDL: &str = "edl";
const XML: &str = "xml";
const CSV: &str = "csv";

/// How a file the user just chose is named in a message: they know where it is, and its
/// full path would bury what's being said about it.
pub(crate) fn quoted_name(path: &Path) -> String {
    let name = path.file_name().unwrap_or(path.as_os_str());
    format!("“{}”", name.to_string_lossy())
}

pub fn is_project(path: &Path) -> bool {
    has_extension(path, project::EXTENSION)
}

pub fn is_xml(path: &Path) -> bool {
    has_extension(path, XML)
}

pub fn is_subrip(path: &Path) -> bool {
    has_extension(path, SUBRIP)
}

pub(crate) fn with_project_extension(mut path: PathBuf) -> PathBuf {
    if !is_project(&path) {
        path.as_mut_os_string()
            .push(format!(".{}", project::EXTENSION));
    }
    path
}

fn has_extension(path: &Path, wanted: &str) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(wanted))
}

impl Editor {
    pub(crate) fn is_dirty(&self) -> bool {
        self.saved_revision != self.timeline.revision()
    }

    pub(crate) fn prompt_path(&mut self, purpose: PathPurpose) {
        if let Some(PathPrompt {
            chooser: Chooser::Native(_),
            ..
        }) = &self.path_prompt
        {
            // The system's dialog is still open; it can't be swapped for another.
            return;
        }
        let project_with = |extension: &str| {
            self.project_path
                .as_ref()
                .map(|path| path.with_extension(extension))
        };
        let start = match purpose {
            PathPurpose::SaveProject => self.project_path.clone(),
            PathPurpose::SaveCopy => self
                .project_path
                .as_deref()
                .map(crate::project_copy::copy_path),
            PathPurpose::Replace(id) => self
                .timeline
                .project()
                .media_item(id)
                .map(|item| item.path.clone()),
            PathPurpose::ExportCaptions => project_with(SUBRIP),
            PathPurpose::Export(kind) => project_with(kind.extension()),
            PathPurpose::ExportFrame => Some(self.default_frame_path()),
            PathPurpose::ExportMedia => self.export.as_ref().map(ExportDialog::output),
            _ => None,
        };
        let chooser = crate::path_dialog::chooser_for(self, purpose, start.as_deref());
        self.path_prompt = Some(PathPrompt {
            purpose,
            chooser,
            start,
            image_sequence: false,
        });
    }

    pub(crate) fn save_project(&mut self) {
        match self.project_path.clone() {
            Some(path) => self.save_project_to(path),
            None => self.prompt_path(PathPurpose::SaveProject),
        }
    }

    pub(crate) fn save_project_to(&mut self, path: PathBuf) {
        let path = with_project_extension(path);
        let sequence = self.timeline.project().clone();
        let revision = self.timeline.revision();
        match &mut self.store {
            Some(store) => store.save(path, sequence, revision),
            None => self.notice("Cannot save: the project worker is not running"),
        }
    }

    pub(crate) fn import_captions(&mut self, path: PathBuf) {
        let timebase = self.timeline.sequence().timebase;
        match &mut self.store {
            Some(store) => store.read_captions(path, timebase),
            None => self.notice("Cannot import captions: the project worker is not running"),
        }
    }

    pub(crate) fn import_interchange(&mut self, path: PathBuf) {
        match &mut self.store {
            Some(store) => store.read_interchange(path),
            None => self.notice("Cannot import XML: the project worker is not running"),
        }
    }

    pub(crate) fn export_captions(&mut self, mut path: PathBuf) {
        if !is_subrip(&path) {
            path.as_mut_os_string().push(format!(".{SUBRIP}"));
        }
        let sequence = self.timeline.sequence();
        let (captions, timebase) = (sequence.captions.items.clone(), sequence.timebase);
        match &mut self.store {
            Some(store) => store.write_captions(path, captions, timebase),
            None => self.notice("Cannot export captions: the project worker is not running"),
        }
    }

    /// Writes the sequence for another editor. An EDL holds the targeted video track, V1
    /// when none is, with the first two audio tracks.
    pub(crate) fn export_interchange(&mut self, mut path: PathBuf, kind: Interchange) {
        let extension = kind.extension();
        if !has_extension(&path, extension) {
            path.as_mut_os_string().push(format!(".{extension}"));
        }
        let title = self
            .project_path
            .as_ref()
            .and_then(|project| project.file_stem())
            .map_or_else(
                || "Untitled".to_owned(),
                |stem| stem.to_string_lossy().into_owned(),
            );
        let sequence = self.timeline.sequence();
        let text = match kind {
            Interchange::Edl => {
                timeline::to_edl(sequence, &title, self.view.targets.video.unwrap_or(0))
            }
            Interchange::FinalCutXml => timeline::to_xmeml(sequence, &title),
            Interchange::MarkersCsv => timeline::markers_csv(sequence),
        };
        match &mut self.store {
            Some(store) => store.write_interchange(path, text),
            None => self.notice("Cannot export: the project worker is not running"),
        }
    }

    /// Runs `action` now, or first asks what to do with unsaved changes.
    pub(crate) fn discard_then(&mut self, action: AfterDiscard) {
        if self.is_dirty() {
            self.confirm_discard = Some(action);
        } else {
            self.after_discard(action);
        }
    }

    pub(crate) fn after_discard(&mut self, action: AfterDiscard) {
        match action {
            AfterDiscard::NewProject => {
                let sequence =
                    Sequence::with_default_tracks(Timebase::HZ_90K, VideoFormat::default());
                self.replace_sequence(sequence, None);
            }
            AfterDiscard::Open(path) => match &mut self.store {
                Some(store) => store.open(path),
                None => self.notice("Cannot open: the project worker is not running"),
            },
            AfterDiscard::Quit => {
                self.quitting = true;
                self.ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    fn replace_sequence(&mut self, sequence: Sequence, path: Option<PathBuf>) {
        // Drop the source player first so its workers stop feeding the audio output.
        self.source = None;
        self.left_sources.clear();
        self.timeline = Timeline::new(sequence);
        let revision = self.timeline.revision();
        self.saved_revision = revision;
        self.project_path = path;
        self.after_save = None;
        self.autosave.forget();
        if let Some(offline) = &mut self.offline {
            offline.forget();
        }
        if let Some(previews) = &mut self.render_previews {
            previews.forget();
        }
        // A new timeline restarts its revisions, so the program viewer is updated directly.
        self.shown_revision = revision;
        self.view.prune_selection(self.timeline.sequence());
        self.show_program(self.timeline.sequence().clone());
    }

    pub(crate) fn poll_store(&mut self) {
        let Some(store) = &mut self.store else {
            return;
        };
        if store.is_busy() {
            self.ctx.request_repaint();
        }
        let events = store.poll();
        for event in events {
            match event {
                StoreEvent::Saved(path, revision, Ok(())) => {
                    self.remember_project(&path);
                    self.saved_revision = revision;
                    self.project_path = Some(path);
                    if let Some(action) = self.after_save.take() {
                        self.after_discard(action);
                    }
                }
                StoreEvent::Saved(path, _, Err(error)) => {
                    self.after_save = None;
                    self.notice(format!("Cannot save {}: {error}", quoted_name(&path)));
                }
                StoreEvent::Opened(path, Ok(sequence)) => match self.take_recovered(&path) {
                    Some(project) => {
                        self.replace_sequence(*sequence, project);
                        // Not on disk as the project yet, and no revision of the new
                        // timeline will ever reach this one.
                        self.saved_revision = u64::MAX;
                    }
                    None => {
                        self.remember_project(&path);
                        self.replace_sequence(*sequence, Some(path));
                    }
                },
                StoreEvent::CopySaved(path, result) => self.copy_saved(&path, result),
                StoreEvent::Opened(path, Err(error)) => {
                    self.notice(format!("Cannot open {}: {error}", quoted_name(&path)));
                }
                StoreEvent::CaptionsRead(path, Ok(items)) if items.is_empty() => {
                    self.notice(format!("No captions found in {}", quoted_name(&path)));
                }
                StoreEvent::CaptionsRead(_, Ok(items)) => {
                    let captions = Captions {
                        items,
                        ..self.timeline.sequence().captions.clone()
                    };
                    self.apply(SetCaptions { captions });
                }
                StoreEvent::CaptionsRead(path, Err(error)) => {
                    self.notice(format!("Cannot import {}: {error}", quoted_name(&path)));
                }
                StoreEvent::CaptionsWritten(_, Ok(())) => {}
                StoreEvent::CaptionsWritten(path, Err(error)) => {
                    self.notice(format!("Cannot export {}: {error}", quoted_name(&path)));
                }
                StoreEvent::InterchangeRead(path, Ok(import)) => {
                    let name = import.name.clone();
                    if self.apply(*import) {
                        self.notice(format!(
                            "Imported the sequence “{name}” from {}; open it from the Project panel",
                            quoted_name(&path)
                        ));
                    }
                }
                StoreEvent::InterchangeRead(path, Err(error)) => {
                    self.notice(format!("Cannot import {}: {error}", quoted_name(&path)));
                }
                StoreEvent::InterchangeWritten(path, Ok(())) => {
                    self.notice(format!("Exported {}", path.display()));
                }
                StoreEvent::InterchangeWritten(path, Err(error)) => {
                    self.notice(format!("Cannot export {}: {error}", quoted_name(&path)));
                }
                StoreEvent::Recoverable(Some(recoverable)) => self.offer_recovery(recoverable),
                StoreEvent::Recoverable(None) | StoreEvent::SessionMarked => {}
                StoreEvent::SettingRead(Setting::Keymap, Ok(Some(text))) => {
                    self.keymap = crate::keymap::Keymap::from_text(&text);
                }
                StoreEvent::SettingRead(Setting::Presets, Ok(Some(text))) => {
                    self.load_presets(&text);
                }
                StoreEvent::SettingRead(Setting::TitleTemplates, Ok(Some(text))) => {
                    self.load_title_templates(&text);
                }
                StoreEvent::SettingRead(Setting::TextStyles, Ok(Some(text))) => {
                    self.load_text_styles(&text);
                }
                StoreEvent::SettingRead(Setting::Workspaces, Ok(Some(text))) => {
                    self.load_workspaces(&text);
                }
                StoreEvent::SettingRead(Setting::RecentProjects, Ok(Some(text))) => {
                    self.recent_projects = RecentProjects::from_text(&text);
                }
                StoreEvent::SettingRead(Setting::Folders, Ok(Some(text))) => {
                    self.browse_folders = crate::path_dialog::folders_from_text(&text);
                }
                StoreEvent::SettingRead(Setting::Preferences, Ok(Some(text))) => {
                    self.load_preferences(&text);
                }
                StoreEvent::SettingRead(Setting::Window, Ok(Some(_))) => {}
                StoreEvent::SettingRead(_, Ok(None)) => {}
                StoreEvent::SettingRead(setting, Err(error)) => {
                    self.notice(format!(
                        "Cannot read {} from {}: {error}",
                        setting.noun(),
                        setting.path().display()
                    ));
                }
                StoreEvent::SettingWritten(_, Ok(())) => {}
                StoreEvent::SettingWritten(setting, Err(error)) => {
                    self.notice(format!(
                        "Cannot save {} to {}: {error}",
                        setting.noun(),
                        setting.path().display()
                    ));
                }
                StoreEvent::Autosaved(_, Ok(())) => {}
                StoreEvent::Autosaved(path, Err(error)) => {
                    self.notice(format!("Cannot autosave to {}: {error}", path.display()));
                }
            }
        }
    }

    /// Asks before closing the window over unsaved changes, and keeps the title current.
    pub(crate) fn update_window(&mut self) {
        let ctx = self.ctx.clone();
        let closing = ctx.input(|input| input.viewport().close_requested());
        let closes = closing && (self.quitting || !self.is_dirty());
        if closing && !closes {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.confirm_discard = Some(AfterDiscard::Quit);
        }
        self.track_window(&ctx, closes);
        self.tend_session_marker(closes);
        let name = self
            .project_path
            .as_ref()
            .and_then(|path| path.file_stem())
            .map_or_else(|| "Untitled".into(), |stem| stem.to_string_lossy());
        let mark = if self.is_dirty() { "*" } else { "" };
        let title = format!("{name}{mark} - Edit");
        if self.window_title != title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }
}

pub fn discard_dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(action) = editor.confirm_discard.clone() else {
        return;
    };
    let name = editor
        .project_path
        .as_ref()
        .and_then(|path| path.file_name())
        .map_or_else(|| "Untitled".into(), |name| name.to_string_lossy());
    let mut choice = None;
    crate::dialog::window(ctx, "Unsaved Changes").show(ctx, |ui| {
        ui.label(format!("Save changes to {name}?"));
        crate::dialog::footer(ui, |ui| {
            if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                choice = Some(Choice::Cancel);
            }
            if ui.button("Don't Save").clicked() {
                choice = Some(Choice::Discard);
            }
            if ui.add(crate::dialog::primary("Save")).clicked()
                || ui.input(|input| input.key_pressed(Key::Enter))
            {
                choice = Some(Choice::Save);
            }
        });
    });
    let Some(choice) = choice else {
        return;
    };
    editor.confirm_discard = None;
    match choice {
        Choice::Save => {
            editor.after_save = Some(action);
            editor.save_project();
        }
        Choice::Discard => editor.after_discard(action),
        Choice::Cancel => {}
    }
}

enum Choice {
    Save,
    Discard,
    Cancel,
}
