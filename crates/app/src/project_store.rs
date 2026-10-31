use std::path::PathBuf;
use std::thread;

use crate::error::AppError;
use crate::recovery::{self, Recoverable};
use crossbeam::channel::{self, Receiver, Sender};
use project::ProjectFile;
use timeline::{
    captions_from_cues, parse_srt, read_xmeml, to_srt, Caption, ImportXmeml, Sequence, Timebase,
};

enum Request {
    Save {
        path: PathBuf,
        sequence: Box<Sequence>,
        mark: SaveMark,
    },
    /// Writes the project elsewhere without it counting as saved.
    SaveCopy {
        path: PathBuf,
        sequence: Box<Sequence>,
    },
    Open(PathBuf),
    Autosave {
        path: PathBuf,
        sequence: Box<Sequence>,
    },
    ReadCaptions(PathBuf, Timebase),
    WriteCaptions {
        path: PathBuf,
        captions: Vec<Caption>,
        timebase: Timebase,
    },
    ReadSetting(Setting),
    WriteSetting(Setting, String),
    /// Renames a settings file that couldn't be read, so saving over it loses nothing.
    SetAsideSetting(Setting),
    /// Keeps saves off a settings file this build can't read but that isn't damaged,
    /// such as one a newer version wrote.
    HoldSetting(Setting),
    /// A Final Cut Pro XML file to read a sequence from.
    ReadInterchange(PathBuf),
    /// A sequence written for another editor, such as an EDL.
    WriteInterchange(PathBuf, String),
    FindRecoverable,
    MarkSession(Option<PathBuf>),
    EndSession,
}

/// Which project and timeline revision a save wrote, so later edits still count as
/// unsaved and a save that finishes after another project was opened is ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveMark {
    pub project: u64,
    pub revision: u64,
}

pub enum StoreEvent {
    Saved(PathBuf, SaveMark, Result<(), AppError>),
    CopySaved(PathBuf, Result<(), AppError>),
    Opened(PathBuf, Result<Box<Sequence>, AppError>),
    Autosaved(PathBuf, Result<(), AppError>),
    CaptionsRead(PathBuf, Result<Vec<Caption>, AppError>),
    CaptionsWritten(PathBuf, Result<(), AppError>),
    /// The settings file's text, or none where there is no file yet.
    SettingRead(Setting, Result<Option<String>, AppError>),
    SettingWritten(Setting, Result<(), AppError>),
    /// Where an unreadable settings file now is.
    SettingSetAside(Setting, Result<PathBuf, AppError>),
    SettingHeld,
    InterchangeRead(PathBuf, Result<Box<ImportXmeml>, AppError>),
    InterchangeWritten(PathBuf, Result<(), AppError>),
    Recoverable(Option<Recoverable>),
    SessionMarked,
}

/// A file of the user's own settings, kept across projects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Keymap,
    Presets,
    TitleTemplates,
    TextStyles,
    Workspaces,
    Folders,
    RecentProjects,
    Window,
    Preferences,
}

impl Setting {
    /// Those the editor reads once it starts; the window's is needed before then.
    pub const READ_AT_START: [Self; 8] = [
        Self::Keymap,
        Self::Presets,
        Self::TitleTemplates,
        Self::TextStyles,
        Self::Workspaces,
        Self::Folders,
        Self::RecentProjects,
        Self::Preferences,
    ];

    pub fn path(self) -> PathBuf {
        app_folder().join(match self {
            Self::Keymap => "Keyboard Shortcuts.txt",
            Self::Presets => "Effect Presets.json",
            Self::TitleTemplates => "Title Templates.json",
            Self::TextStyles => "Text Styles.json",
            Self::Workspaces => "Workspaces.txt",
            Self::Folders => "Recent Folders.txt",
            Self::RecentProjects => "Recent Projects.txt",
            Self::Window => "Window.txt",
            Self::Preferences => "Preferences.txt",
        })
    }

    /// The first free name to move an unreadable file to, so one set aside earlier is
    /// kept too.
    pub fn unreadable_path(self) -> PathBuf {
        let path = self.path();
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let extension = path.extension().unwrap_or_default().to_string_lossy();
        let named = |suffix: String| {
            path.with_file_name(format!("{stem} (unreadable{suffix}).{extension}"))
        };
        std::iter::once(named(String::new()))
            .chain((2..).map(|n| named(format!(" {n}"))))
            .find(|candidate| !candidate.exists())
            .unwrap_or_else(|| named(String::new()))
    }

    /// What the file holds, for messages about it.
    pub fn noun(self) -> &'static str {
        match self {
            Self::Keymap => "shortcuts",
            Self::Presets => "effect presets",
            Self::TitleTemplates => "title templates",
            Self::TextStyles => "text styles",
            Self::Workspaces => "workspaces",
            Self::Folders => "recent folders",
            Self::RecentProjects => "recent projects",
            Self::Window => "window size",
            Self::Preferences => "preferences",
        }
    }
}

const APP_FOLDER_NAME: &str = "Cledit";
/// The folder's name from before the app was called Cledit.
const OLD_APP_FOLDER_NAME: &str = "Edit";

/// Where the editor keeps files of its own: `%APPDATA%\Cledit`.
pub fn app_folder() -> PathBuf {
    app_data().join(APP_FOLDER_NAME)
}

fn app_data() -> PathBuf {
    std::env::var_os("APPDATA").map_or_else(std::env::temp_dir, PathBuf::from)
}

/// Carries preferences, shortcuts, autosaves and downloaded models over from the
/// folder used before the rename, once, before anything reads the new one.
pub fn adopt_old_app_folder() {
    let old = app_data().join(OLD_APP_FOLDER_NAME);
    let new = app_folder();
    if old.is_dir() && !new.exists() {
        // Failing leaves the old folder in place, and the app starts with defaults.
        let _ = std::fs::rename(old, new);
    }
}

/// Reads and writes project files on a worker thread so the UI never touches the disk.
pub struct ProjectStore {
    requests: Sender<Request>,
    events: Receiver<StoreEvent>,
    pending: usize,
    worker: Option<thread::JoinHandle<()>>,
}

impl ProjectStore {
    pub fn new() -> std::io::Result<Self> {
        let (request_tx, request_rx) = channel::unbounded::<Request>();
        let (event_tx, event_rx) = channel::unbounded();
        let worker = thread::Builder::new()
            .name("project".to_owned())
            .spawn(move || {
                let mut marker = recovery::SessionMarker::default();
                let mut held = Vec::new();
                for request in request_rx {
                    if event_tx
                        .send(handle(request, &mut marker, &mut held))
                        .is_err()
                    {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests: request_tx,
            events: event_rx,
            pending: 0,
            worker: Some(worker),
        })
    }

    pub fn save(&mut self, path: PathBuf, sequence: Sequence, mark: SaveMark) {
        let request = Request::Save {
            path,
            sequence: Box::new(sequence),
            mark,
        };
        if self.requests.send(request).is_ok() {
            self.pending += 1;
        }
    }

    pub fn save_copy(&mut self, path: PathBuf, sequence: Sequence) {
        let request = Request::SaveCopy {
            path,
            sequence: Box::new(sequence),
        };
        if self.requests.send(request).is_ok() {
            self.pending += 1;
        }
    }

    /// Writes a copy that doesn't count as saving the project.
    pub fn autosave(&mut self, path: PathBuf, sequence: Sequence) {
        let request = Request::Autosave {
            path,
            sequence: Box::new(sequence),
        };
        if self.requests.send(request).is_ok() {
            self.pending += 1;
        }
    }

    pub fn open(&mut self, path: PathBuf) {
        if self.requests.send(Request::Open(path)).is_ok() {
            self.pending += 1;
        }
    }

    /// Reads a SubRip file as captions in `timebase`.
    pub fn read_captions(&mut self, path: PathBuf, timebase: Timebase) {
        self.send(Request::ReadCaptions(path, timebase));
    }

    pub fn write_captions(&mut self, path: PathBuf, captions: Vec<Caption>, timebase: Timebase) {
        self.send(Request::WriteCaptions {
            path,
            captions,
            timebase,
        });
    }

    pub fn read_interchange(&mut self, path: PathBuf) {
        self.send(Request::ReadInterchange(path));
    }

    pub fn write_interchange(&mut self, path: PathBuf, text: String) {
        self.send(Request::WriteInterchange(path, text));
    }

    pub fn read_setting(&mut self, setting: Setting) {
        self.send(Request::ReadSetting(setting));
    }

    pub fn write_setting(&mut self, setting: Setting, text: String) {
        self.send(Request::WriteSetting(setting, text));
    }

    pub fn set_aside_setting(&mut self, setting: Setting) {
        self.send(Request::SetAsideSetting(setting));
    }

    pub fn hold_setting(&mut self, setting: Setting) {
        self.send(Request::HoldSetting(setting));
    }

    /// Looks for work lost when the last session crashed; ask before marking this one.
    pub fn find_recoverable(&mut self) {
        self.send(Request::FindRecoverable);
    }

    pub fn mark_session(&mut self, project: Option<PathBuf>) {
        self.send(Request::MarkSession(project));
    }

    pub fn end_session(&mut self) {
        self.send(Request::EndSession);
    }

    fn send(&mut self, request: Request) {
        if self.requests.send(request).is_ok() {
            self.pending += 1;
        }
    }

    pub fn is_busy(&self) -> bool {
        self.pending > 0
    }

    pub fn poll(&mut self) -> Vec<StoreEvent> {
        let events: Vec<_> = self.events.try_iter().collect();
        self.pending = self.pending.saturating_sub(events.len());
        events
    }
}

impl Drop for ProjectStore {
    /// Lets writes asked for just before quitting, like settings, reach the disk before
    /// the process ends and takes the thread with it.
    fn drop(&mut self) {
        let (closed, _) = channel::bounded(0);
        drop(std::mem::replace(&mut self.requests, closed));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// `held` lists the settings whose files couldn't be read and are still where they
/// were: saving over one would replace what it holds with only what was made since.
fn handle(
    request: Request,
    marker: &mut recovery::SessionMarker,
    held: &mut Vec<Setting>,
) -> StoreEvent {
    match request {
        Request::Save {
            path,
            sequence,
            mark,
        } => {
            let result = ProjectFile::new(*sequence)
                .save(&path)
                .map_err(AppError::from);
            StoreEvent::Saved(path, mark, result)
        }
        Request::SaveCopy { path, sequence } => {
            let result = ProjectFile::new(*sequence)
                .save(&path)
                .map_err(AppError::from);
            StoreEvent::CopySaved(path, result)
        }
        Request::Open(path) => {
            let result = ProjectFile::load(&path)
                .map(|file| Box::new(file.sequence))
                .map_err(AppError::from);
            StoreEvent::Opened(path, result)
        }
        Request::Autosave { path, sequence } => {
            let result = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .map_err(AppError::from)
                .and_then(|()| {
                    ProjectFile::new(*sequence)
                        .save(&path)
                        .map_err(AppError::from)
                });
            StoreEvent::Autosaved(path, result)
        }
        Request::ReadCaptions(path, timebase) => {
            let result = std::fs::read(&path)
                .map_err(AppError::from)
                .and_then(|bytes| Ok(parse_srt(&timeline::decode_text(&bytes), timebase)?))
                .map(captions_from_cues);
            StoreEvent::CaptionsRead(path, result)
        }
        Request::WriteCaptions {
            path,
            captions,
            timebase,
        } => {
            let result = project::replace_file(&path, to_srt(&captions, timebase).as_bytes())
                .map_err(AppError::from);
            StoreEvent::CaptionsWritten(path, result)
        }
        Request::ReadSetting(setting) => {
            let result = match std::fs::read_to_string(setting.path()) {
                Ok(text) => Ok(Some(text)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => {
                    held.push(setting);
                    Err(error.into())
                }
            };
            StoreEvent::SettingRead(setting, result)
        }
        Request::WriteSetting(setting, text) => {
            if held.contains(&setting) {
                return StoreEvent::SettingWritten(setting, Err(AppError::SettingHeld));
            }
            let path = setting.path();
            let result = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| project::replace_file(&path, text.as_bytes()))
                .map_err(AppError::from);
            StoreEvent::SettingWritten(setting, result)
        }
        Request::SetAsideSetting(setting) => {
            let path = setting.path();
            let aside = setting.unreadable_path();
            let result = project::retry_while_in_use(|| std::fs::rename(&path, &aside))
                .map(|()| aside)
                .map_err(AppError::from);
            match result {
                Ok(_) => held.retain(|kept| *kept != setting),
                Err(_) => held.push(setting),
            }
            StoreEvent::SettingSetAside(setting, result)
        }
        Request::HoldSetting(setting) => {
            held.push(setting);
            StoreEvent::SettingHeld
        }
        Request::ReadInterchange(path) => {
            let result = std::fs::read(&path)
                .map_err(AppError::from)
                .and_then(|bytes| Ok(read_xmeml(&timeline::decode_text(&bytes))?))
                .map(Box::new);
            StoreEvent::InterchangeRead(path, result)
        }
        Request::WriteInterchange(path, text) => {
            let result = project::replace_file(&path, text.as_bytes()).map_err(AppError::from);
            StoreEvent::InterchangeWritten(path, result)
        }
        Request::FindRecoverable => StoreEvent::Recoverable(recovery::find_recoverable()),
        // The marker only matters after a crash, so failing to keep it isn't worth
        // interrupting anyone over.
        Request::MarkSession(project) => {
            let _ = marker.mark(project.as_deref());
            StoreEvent::SessionMarked
        }
        Request::EndSession => {
            let _ = marker.end();
            StoreEvent::SessionMarked
        }
    }
}
