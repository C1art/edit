use std::fs::{self, File};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use egui::{Context, Key};

use crate::autosave::{autosave_path, SLOTS};
use crate::editor::Editor;
use crate::project_files::{quoted_name, AfterDiscard};

/// Unsaved work found autosaved after the last session ended without closing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recoverable {
    pub autosave: PathBuf,
    /// The project the work belongs to; `None` for an untitled one.
    pub project: Option<PathBuf>,
    pub autosaved_at: SystemTime,
}

#[derive(Default)]
pub(crate) struct Recovery {
    offered: Option<Recoverable>,
    opening: Option<Recoverable>,
    /// The project the session marker names; `None` until it's first written.
    marked: Option<Option<PathBuf>>,
}

/// Present while an editor runs and removed when it closes, so one found at start that
/// no running editor holds means a session ended in a crash. It names the open project,
/// whose autosaves then hold whatever work was lost.
#[derive(Default)]
pub struct SessionMarker(Option<(PathBuf, File)>);

impl SessionMarker {
    pub fn mark(&mut self, project: Option<&Path>) -> std::io::Result<()> {
        let file = match &mut self.0 {
            Some((_, file)) => file,
            None => {
                let folder = crate::project_store::app_folder();
                fs::create_dir_all(&folder)?;
                let path = folder.join(format!("Running {}.txt", std::process::id()));
                let file = open_held(&path)?;
                &mut self.0.insert((path, file)).1
            }
        };
        let text = project.map_or_else(String::new, |path| path.to_string_lossy().into_owned());
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(text.as_bytes())
    }

    pub fn end(&mut self) -> std::io::Result<()> {
        let Some((path, file)) = self.0.take() else {
            return Ok(());
        };
        drop(file);
        fs::remove_file(path)
    }
}

/// Opens the marker so that no other editor can read or remove it while this one runs;
/// the system lets go of it when the process ends, crash or not.
fn open_held(path: &Path) -> std::io::Result<File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::share_mode(&mut options, 0);
    options.open(path)
}

fn is_marker(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("Running") && name.ends_with(".txt"))
}

/// Run on the project worker at start, before this session writes its own marker. The
/// marker of the work offered is removed, so it's offered only once; other crashed
/// sessions with work keep theirs to be offered at a later start.
pub fn find_recoverable() -> Option<Recoverable> {
    let markers = fs::read_dir(crate::project_store::app_folder()).ok()?;
    let (marker, newest) = markers
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| is_marker(path))
        .filter_map(|marker| {
            // A marker that can't be read is held by an editor still running.
            let marked_at = fs::metadata(&marker).and_then(|meta| meta.modified());
            let text = fs::read_to_string(&marker);
            match recoverable_from(text.ok()?, marked_at.ok()?) {
                Some(recoverable) => Some((marker, recoverable)),
                None => {
                    let _ = fs::remove_file(&marker);
                    None
                }
            }
        })
        .max_by_key(|(_, recoverable)| recoverable.autosaved_at)?;
    let _ = fs::remove_file(marker);
    Some(newest)
}

fn recoverable_from(text: String, marked_at: SystemTime) -> Option<Recoverable> {
    let project = Some(PathBuf::from(text.trim())).filter(|path| !path.as_os_str().is_empty());
    let modified = |path: &Path| fs::metadata(path).and_then(|meta| meta.modified()).ok();
    let autosaves = (0..SLOTS)
        .map(|slot| autosave_path(project.as_deref(), slot))
        .filter_map(|path| modified(&path).map(|at| (path, at)));
    let project_saved_at = project.as_deref().and_then(modified);
    let (autosave, autosaved_at) = newest_unsaved(marked_at, project_saved_at, autosaves)?;
    Some(Recoverable {
        autosave,
        project,
        autosaved_at,
    })
}

/// The newest autosave written in the crashed session and after the project was last
/// saved; older ones hold work that was saved or deliberately thrown away.
fn newest_unsaved(
    marked_at: SystemTime,
    project_saved_at: Option<SystemTime>,
    autosaves: impl Iterator<Item = (PathBuf, SystemTime)>,
) -> Option<(PathBuf, SystemTime)> {
    let after = project_saved_at.map_or(marked_at, |saved| saved.max(marked_at));
    autosaves
        .filter(|(_, at)| *at > after)
        .max_by_key(|(_, at)| *at)
}

impl Editor {
    pub(crate) fn offer_recovery(&mut self, recoverable: Recoverable) {
        self.recovery.offered = Some(recoverable);
    }

    /// Keeps the session marker naming the open project, and removes it on closing.
    pub(crate) fn tend_session_marker(&mut self, closes: bool) {
        let Some(store) = &mut self.store else {
            return;
        };
        if closes {
            store.end_session();
        } else if self.recovery.marked.as_ref() != Some(&self.project_path) {
            store.mark_session(self.project_path.clone());
            self.recovery.marked = Some(self.project_path.clone());
        }
    }

    /// Takes over an opened file when it's the autosave being recovered: it stands for
    /// the project it was saved from, with all of it still to be saved.
    pub(crate) fn take_recovered(&mut self, opened: &Path) -> Option<Option<PathBuf>> {
        let recovered = self
            .recovery
            .opening
            .take_if(|recovering| recovering.autosave == opened)?;
        Some(recovered.project)
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &Context) {
    let Some(offered) = &editor.recovery.offered else {
        return;
    };
    let what = offered
        .project
        .as_deref()
        .map_or_else(|| "an untitled project".to_owned(), quoted_name);
    let ago = offered.autosaved_at.elapsed().unwrap_or_default();
    let mut recover = None;
    crate::dialog::window(ctx, "Recover Unsaved Work").show(ctx, |ui| {
        ui.label("Cledit didn't close normally last time.");
        ui.label(format!(
            "Changes to {what} were autosaved {} but never saved.",
            ago_text(ago)
        ));
        ui.weak("Recovered work opens unsaved, so you can look it over before saving it.");
        crate::dialog::footer(ui, |ui| {
            if ui.button("Don't Recover").clicked()
                || ui.input(|input| input.key_pressed(Key::Escape))
            {
                recover = Some(false);
            }
            if ui.add(crate::dialog::primary("Recover")).clicked()
                || ui.input(|input| input.key_pressed(Key::Enter))
            {
                recover = Some(true);
            }
        });
    });
    let Some(recover) = recover else {
        return;
    };
    let Some(offered) = editor.recovery.offered.take() else {
        return;
    };
    if recover {
        let autosave = offered.autosave.clone();
        editor.recovery.opening = Some(offered);
        editor.discard_then(AfterDiscard::Open(autosave));
    }
}

fn ago_text(ago: Duration) -> String {
    let minutes = ago.as_secs() / 60;
    let (count, unit) = match minutes {
        0 => return "less than a minute ago".to_owned(),
        1..=59 => (minutes, "minute"),
        60..=2879 => (minutes / 60, "hour"),
        _ => (minutes / 1440, "day"),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_work_newer_than_the_session_and_the_last_save_is_offered() {
        let at = |secs| SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
        let autosaves = || {
            [("a", 100), ("b", 300), ("c", 200)]
                .map(|(name, secs)| (PathBuf::from(name), at(secs)))
                .into_iter()
        };
        let newest = |marked, saved| newest_unsaved(at(marked), saved, autosaves());
        assert_eq!(newest(50, None), Some((PathBuf::from("b"), at(300))));
        assert_eq!(
            newest(50, Some(at(250))),
            Some((PathBuf::from("b"), at(300)))
        );
        // Saved after the last autosave, or autosaved only in earlier sessions.
        assert_eq!(newest(50, Some(at(300))), None);
        assert_eq!(newest(400, None), None);
    }
}
