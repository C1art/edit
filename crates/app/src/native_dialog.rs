//! The system's own file dialogs, where there are some. One is shown from a worker so
//! the editor keeps drawing while it's open; the system keeps the editor's window from
//! taking input meanwhile.

use std::path::PathBuf;

use crossbeam::channel::{Receiver, TryRecvError};

use crate::file_browser::{FileFilter, Mode};

/// What a dialog asks for.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) struct Ask {
    pub(crate) title: &'static str,
    /// The label of the button that chooses.
    pub(crate) action: &'static str,
    pub(crate) mode: Mode,
    pub(crate) filters: Vec<FileFilter>,
    pub(crate) folder: Option<PathBuf>,
    /// The name a save starts with.
    pub(crate) file_name: Option<String>,
    /// Whether to offer the choice of taking numbered pictures as one clip.
    pub(crate) image_sequence: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Answer {
    /// Empty when the dialog was cancelled.
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) image_sequence: bool,
}

pub(crate) enum Polled {
    Waiting,
    Answered(Answer),
    /// The dialog couldn't be shown, so the editor's own browser should ask instead.
    Failed,
}

pub(crate) struct NativeDialog {
    answer: Receiver<Option<Answer>>,
}

impl NativeDialog {
    /// The dialog for `ask`, or `None` where the system has none to show.
    #[cfg(windows)]
    pub(crate) fn open(ask: Ask, ctx: &egui::Context) -> Option<Self> {
        let owner = windows_dialog::active_window();
        let (sender, answer) = crossbeam::channel::bounded(1);
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("file dialog".to_owned())
            .spawn(move || {
                let _ = sender.send(windows_dialog::show(&ask, owner));
                ctx.request_repaint();
            })
            .ok()?;
        Some(Self { answer })
    }

    #[cfg(not(windows))]
    pub(crate) fn open(_ask: Ask, _ctx: &egui::Context) -> Option<Self> {
        None
    }

    pub(crate) fn poll(&self) -> Polled {
        match self.answer.try_recv() {
            Ok(Some(answer)) => Polled::Answered(answer),
            Err(TryRecvError::Empty) => Polled::Waiting,
            Ok(None) | Err(TryRecvError::Disconnected) => Polled::Failed,
        }
    }
}

/// `*.mp4;*.mov`, as a dialog's file types are written; every file when there are none.
#[cfg_attr(not(windows), allow(dead_code))]
fn patterns(filter: FileFilter) -> String {
    let patterns: Vec<String> = filter
        .extensions
        .iter()
        .flat_map(|group| group.iter())
        .map(|extension| format!("*.{extension}"))
        .collect();
    if patterns.is_empty() {
        "*.*".to_owned()
    } else {
        patterns.join(";")
    }
}

#[cfg(windows)]
mod windows_dialog {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;

    use windows::core::{Interface, HSTRING, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FileOpenDialog, FileSaveDialog, IFileDialog, IFileDialogCustomize, IFileOpenDialog,
        IShellItem, SHCreateItemFromParsingName, FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST,
        FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
    };

    use super::{patterns, Answer, Ask};
    use crate::file_browser::Mode;

    const IMAGE_SEQUENCE: u32 = 1;

    /// The editor's window, as a number so it can go to the worker; taken on the thread
    /// that owns the window.
    pub(super) fn active_window() -> isize {
        // SAFETY: no arguments; a null handle when the thread has no active window.
        unsafe { GetActiveWindow() }.0 as isize
    }

    /// `None` when the dialog couldn't be made or failed other than by being cancelled.
    pub(super) fn show(ask: &Ask, owner: isize) -> Option<Answer> {
        // SAFETY: this worker's own apartment, ended below before the thread does; every
        // COM object is made and dropped inside `ask_in_apartment`.
        let initialized = unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).is_ok()
        };
        if !initialized {
            return None;
        }
        let answer = ask_in_apartment(ask, HWND(owner as *mut _));
        // SAFETY: balances the initialization above.
        unsafe { CoUninitialize() };
        match answer {
            Ok(answer) => Some(answer),
            Err(error) if error.code() == ERROR_CANCELLED.to_hresult() => Some(Answer::default()),
            Err(_) => None,
        }
    }

    fn ask_in_apartment(ask: &Ask, owner: HWND) -> windows::core::Result<Answer> {
        let class = match ask.mode {
            Mode::Open { .. } => &FileOpenDialog,
            Mode::Save => &FileSaveDialog,
        };
        // SAFETY: the apartment is initialized on this thread.
        let dialog: IFileDialog = unsafe { CoCreateInstance(class, None, CLSCTX_INPROC_SERVER)? };
        // Kept alive until the dialog closes, as it reads the file types from them.
        let names: Vec<HSTRING> = ask.filters.iter().map(|f| HSTRING::from(f.name)).collect();
        let specs: Vec<HSTRING> = ask
            .filters
            .iter()
            .map(|&filter| HSTRING::from(patterns(filter)))
            .collect();
        let types: Vec<COMDLG_FILTERSPEC> = names
            .iter()
            .zip(&specs)
            .map(|(name, spec)| COMDLG_FILTERSPEC {
                pszName: PCWSTR(name.as_ptr()),
                pszSpec: PCWSTR(spec.as_ptr()),
            })
            .collect();
        let options = match ask.mode {
            Mode::Open { many } if many => FOS_FILEMUSTEXIST | FOS_ALLOWMULTISELECT,
            Mode::Open { .. } => FOS_FILEMUSTEXIST,
            Mode::Save => FOS_OVERWRITEPROMPT | FOS_PATHMUSTEXIST,
        };
        let extension = ask.filters.first().and_then(|f| f.default_extension());
        // SAFETY: the dialog is alive, and every string outlives the calls that read it.
        unsafe {
            dialog.SetOptions(dialog.GetOptions()? | options | FOS_FORCEFILESYSTEM)?;
            dialog.SetTitle(&HSTRING::from(ask.title))?;
            dialog.SetOkButtonLabel(&HSTRING::from(ask.action))?;
            if !types.is_empty() {
                dialog.SetFileTypes(&types)?;
            }
            if let Some(extension) = extension {
                dialog.SetDefaultExtension(&HSTRING::from(extension))?;
            }
            if let Some(folder) = &ask.folder {
                // A folder that's gone leaves the system's choice.
                let item: windows::core::Result<IShellItem> =
                    SHCreateItemFromParsingName(&HSTRING::from(folder.as_path()), None);
                if let Ok(item) = item {
                    dialog.SetFolder(&item)?;
                }
            }
            if let Some(name) = &ask.file_name {
                dialog.SetFileName(&HSTRING::from(name.as_str()))?;
            }
            let customize = ask
                .image_sequence
                .then(|| dialog.cast::<IFileDialogCustomize>())
                .transpose()?;
            if let Some(customize) = &customize {
                let label = HSTRING::from("Image Sequence");
                customize.AddCheckButton(IMAGE_SEQUENCE, &label, false)?;
            }
            let owner = (!owner.is_invalid()).then_some(owner);
            dialog.Show(owner)?;
            let paths = match ask.mode {
                Mode::Open { .. } => {
                    let results = dialog.cast::<IFileOpenDialog>()?.GetResults()?;
                    (0..results.GetCount()?)
                        .map(|index| path_of(&results.GetItemAt(index)?))
                        .collect::<windows::core::Result<_>>()?
                }
                Mode::Save => vec![path_of(&dialog.GetResult()?)?],
            };
            let image_sequence = match &customize {
                Some(customize) => customize.GetCheckButtonState(IMAGE_SEQUENCE)?.as_bool(),
                None => false,
            };
            Ok(Answer {
                paths,
                image_sequence,
            })
        }
    }

    fn path_of(item: &IShellItem) -> windows::core::Result<PathBuf> {
        // SAFETY: the item is alive; the name it returns is ours to free, and is read
        // before it's freed.
        unsafe {
            let name: PWSTR = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path = OsString::from_wide(name.as_wide());
            CoTaskMemFree(Some(name.0 as *const _));
            Ok(PathBuf::from(path))
        }
    }
}
