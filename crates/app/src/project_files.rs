use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{Key, ViewportCommand};
use timeline::{Sequence, Timebase, Timeline, VideoFormat};

use crate::editor::Editor;
use crate::project_store::StoreEvent;

/// What a typed path is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathPurpose {
    Import,
    OpenProject,
    SaveProject,
}

pub struct PathPrompt {
    pub purpose: PathPurpose,
    pub text: String,
}

/// What to do once unsaved changes are saved or discarded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AfterDiscard {
    NewProject,
    Open(PathBuf),
    Quit,
}

pub fn is_project(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(project::EXTENSION))
}

impl Editor {
    pub(crate) fn is_dirty(&self) -> bool {
        self.saved_revision != self.timeline.revision()
    }

    pub(crate) fn prompt_path(&mut self, purpose: PathPurpose) {
        let text = match purpose {
            PathPurpose::SaveProject => self
                .project_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            _ => String::new(),
        };
        self.path_prompt = Some(PathPrompt { purpose, text });
    }

    pub(crate) fn save_project(&mut self) {
        match self.project_path.clone() {
            Some(path) => self.save_project_to(path),
            None => self.prompt_path(PathPurpose::SaveProject),
        }
    }

    fn save_project_to(&mut self, mut path: PathBuf) {
        if !is_project(&path) {
            path.as_mut_os_string()
                .push(format!(".{}", project::EXTENSION));
        }
        let sequence = self.timeline.sequence().clone();
        let revision = self.timeline.revision();
        match &mut self.store {
            Some(store) => store.save(path, sequence, revision),
            None => self.notice("Cannot save: the project worker is not running"),
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

    fn after_discard(&mut self, action: AfterDiscard) {
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
        self.timeline = Timeline::new(sequence);
        let revision = self.timeline.revision();
        self.saved_revision = revision;
        self.project_path = path;
        self.after_save = None;
        // A new timeline restarts its revisions, so the program viewer is updated directly.
        self.shown_revision = revision;
        self.view.prune_selection(self.timeline.sequence());
        if let Some(program) = &mut self.program {
            program
                .player
                .set_sequence(Arc::new(self.timeline.sequence().clone()));
        }
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
                    self.saved_revision = revision;
                    self.project_path = Some(path);
                    if let Some(action) = self.after_save.take() {
                        self.after_discard(action);
                    }
                }
                StoreEvent::Saved(path, _, Err(error)) => {
                    self.after_save = None;
                    self.notice(format!("Cannot save {}: {error}", path.display()));
                }
                StoreEvent::Opened(path, Ok(sequence)) => {
                    self.replace_sequence(*sequence, Some(path));
                }
                StoreEvent::Opened(path, Err(error)) => {
                    self.notice(format!("Cannot open {}: {error}", path.display()));
                }
            }
        }
    }

    /// Asks before closing the window over unsaved changes, and keeps the title current.
    pub(crate) fn update_window(&mut self) {
        let ctx = self.ctx.clone();
        let closing = ctx.input(|input| input.viewport().close_requested());
        if closing && !self.quitting && self.is_dirty() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.confirm_discard = Some(AfterDiscard::Quit);
        }
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

pub fn path_dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(prompt) = &mut editor.path_prompt else {
        return;
    };
    let (title, hint, action) = match prompt.purpose {
        PathPurpose::Import => (
            "Import Media",
            "Path to a video or audio file (or drop files on the window):",
            "Import",
        ),
        PathPurpose::OpenProject => ("Open Project", "Path to a project file:", "Open"),
        PathPurpose::SaveProject => ("Save Project As", "Save the project to:", "Save"),
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
    let purpose = prompt.purpose;
    if let Some(path) = submitted.filter(|path| !path.as_os_str().is_empty()) {
        editor.path_prompt = None;
        match purpose {
            PathPurpose::Import => editor.import(path),
            PathPurpose::OpenProject => editor.discard_then(AfterDiscard::Open(path)),
            PathPurpose::SaveProject => editor.save_project_to(path),
        }
    } else if cancelled {
        editor.path_prompt = None;
        // Cancelling Save As also cancels what was waiting on the save.
        editor.after_save = None;
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
    egui::Window::new("Unsaved Changes")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(format!("Save changes to {name}?"));
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    choice = Some(Choice::Save);
                }
                if ui.button("Don't Save").clicked() {
                    choice = Some(Choice::Discard);
                }
                if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape))
                {
                    choice = Some(Choice::Cancel);
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
