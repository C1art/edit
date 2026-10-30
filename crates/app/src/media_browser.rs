//! Media Browser: folders on disk listed in a panel, to import media from without a
//! dialog. Folders are read on the file browser's thread, never the UI's.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{vec2, Align2, Rect, ScrollArea, Sense, TextEdit, Ui};
use timeline::SourceId;

use crate::editor::Editor;
use crate::file_browser::{file_icon, home, Entry, Found, Lister, Place, ALL_MEDIA};
use crate::icons::{self, Icon};
use crate::path_dialog::FolderUse;

const ROW_HEIGHT: f32 = 22.0;
const ICON: f32 = 14.0;
const NAME_LEFT: f32 = 28.0;
const BUTTON: egui::Vec2 = egui::vec2(24.0, 22.0);
/// How often the shown folder is read again, so files copied in while the panel is open
/// turn up.
const RELIST_EVERY: Duration = Duration::from_secs(3);

#[derive(Default)]
pub(crate) struct MediaBrowser {
    lister: Option<Lister>,
    started: bool,
    folder: Option<PathBuf>,
    looking: Option<PathBuf>,
    listed_at: Option<Instant>,
    location: String,
    entries: Vec<Entry>,
    /// Indices into `entries` of the folders and media the search lets through.
    shown: Vec<usize>,
    places: Vec<Place>,
    back: Vec<PathBuf>,
    search: String,
    selected: HashSet<PathBuf>,
    error: Option<String>,
}

enum Action {
    Import(Vec<PathBuf>),
    Open(SourceId),
    Reveal(PathBuf),
}

impl MediaBrowser {
    fn start(&mut self, first: Option<PathBuf>) {
        self.started = true;
        match Lister::new() {
            Ok(lister) => {
                lister.find_places();
                self.lister = Some(lister);
            }
            Err(error) => self.error = Some(format!("Cannot list folders: {error}")),
        }
        if let Some(first) = first.or_else(home) {
            self.look(first);
        }
    }

    fn look(&mut self, path: PathBuf) {
        if let Some(lister) = &self.lister {
            lister.look(path.clone());
            self.looking = Some(path);
        }
    }

    fn open_folder(&mut self, path: PathBuf) {
        if let Some(folder) = self.folder.clone() {
            self.back.push(folder);
        }
        self.look(path);
    }

    /// Takes in what the lister found; the folder it settles on, if a new one.
    fn take_found(&mut self) -> Option<PathBuf> {
        let mut opened = None;
        while let Some(found) = self.lister.as_ref().and_then(Lister::poll) {
            match found {
                Found::Places(places) => self.places = places,
                Found::Folder(path, entries) => {
                    if self.looking.take_if(|asked| *asked == path).is_none() {
                        continue;
                    }
                    if self.folder.as_ref() != Some(&path) {
                        self.selected.clear();
                        self.location = path.display().to_string();
                        opened = Some(path.clone());
                    }
                    self.folder = Some(path);
                    self.entries = entries;
                    self.listed_at = Some(Instant::now());
                    self.error = None;
                    self.refilter();
                }
                Found::File(path) | Found::Missing(path, _)
                    if self.folder.is_none() && self.looking.as_ref() == Some(&path) =>
                {
                    // Where it was to open is gone, so it opens at home instead.
                    self.looking = None;
                    match home().filter(|home| *home != path) {
                        Some(home) => self.look(home),
                        None => self.error = Some(format!("Cannot open {}", path.display())),
                    }
                }
                Found::File(path) | Found::Missing(path, _) => {
                    if self.looking.take_if(|asked| *asked == path).is_some() {
                        self.error = Some(format!("{} is not a folder", path.display()));
                        self.location = self
                            .folder
                            .as_ref()
                            .map(|folder| folder.display().to_string())
                            .unwrap_or_default();
                    }
                }
            }
        }
        opened
    }

    fn refilter(&mut self) {
        let search = self.search.trim().to_lowercase();
        self.shown = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.folder || ALL_MEDIA.shows(&entry.path))
            .filter(|(_, entry)| search.is_empty() || entry.name.to_lowercase().contains(&search))
            .map(|(index, _)| index)
            .collect();
    }

    fn selected_files(&self) -> Vec<PathBuf> {
        self.shown
            .iter()
            .map(|&index| &self.entries[index])
            .filter(|entry| !entry.folder && self.selected.contains(&entry.path))
            .map(|entry| entry.path.clone())
            .collect()
    }

    fn location_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let back = ui.add_enabled_ui(!self.back.is_empty(), |ui| {
                icons::button(ui, Icon::Back, BUTTON).on_hover_text("Back")
            });
            if back.inner.clicked() {
                if let Some(previous) = self.back.pop() {
                    self.look(previous);
                }
            }
            let parent = self
                .folder
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_owned);
            let up = ui.add_enabled_ui(parent.is_some(), |ui| {
                icons::button(ui, Icon::Up, BUTTON).on_hover_text("Up one folder")
            });
            if let (true, Some(parent)) = (up.inner.clicked(), parent) {
                self.open_folder(parent);
            }
            let places = icons::button(ui, Icon::Folder, BUTTON).on_hover_text("Places");
            let mut chosen = None;
            egui::Popup::menu(&places).show(|ui| {
                for place in &self.places {
                    if ui.button(&place.name).clicked() {
                        chosen = Some(place.path.clone());
                    }
                }
            });
            if let Some(place) = chosen {
                self.open_folder(place);
            }
            let location = ui.add(
                TextEdit::singleline(&mut self.location)
                    .hint_text("Folder")
                    .desired_width(f32::INFINITY),
            );
            let enter = ui.input(|input| input.key_pressed(egui::Key::Enter));
            if location.lost_focus() && enter {
                let typed = PathBuf::from(self.location.trim().trim_matches('"'));
                if !typed.as_os_str().is_empty() {
                    self.open_folder(typed);
                }
            }
        });
    }

    fn entries_list(
        &mut self,
        ui: &mut Ui,
        height: f32,
        in_project: &HashMap<&Path, SourceId>,
    ) -> Option<Action> {
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut action = None;
        let mut entered = None;
        let output = ScrollArea::vertical()
            .id_salt("media browser")
            .max_height(height)
            .auto_shrink([false, false])
            .show_rows(ui, ROW_HEIGHT, self.shown.len(), |ui, rows| {
                for row in rows {
                    let entry = &self.entries[self.shown[row]];
                    let (rect, response) = ui.allocate_exact_size(
                        vec2(ui.available_width(), ROW_HEIGHT),
                        Sense::click(),
                    );
                    let selected = self.selected.contains(&entry.path);
                    let source = in_project.get(entry.path.as_path()).copied();
                    paint_row(
                        ui,
                        rect,
                        entry,
                        selected,
                        response.hovered(),
                        source.is_some(),
                    );
                    let response = match source {
                        Some(_) => response.on_hover_text("In the project"),
                        None => response,
                    };
                    if response.double_clicked() {
                        if entry.folder {
                            entered = Some(entry.path.clone());
                        } else {
                            action = Some(match source {
                                Some(source) => Action::Open(source),
                                None => Action::Import(vec![entry.path.clone()]),
                            });
                        }
                    } else if response.clicked() || response.secondary_clicked() {
                        let toggle = ui.input(|input| input.modifiers.command);
                        if toggle && response.clicked() {
                            if !self.selected.remove(&entry.path) {
                                self.selected.insert(entry.path.clone());
                            }
                        } else if !selected {
                            self.selected = HashSet::from([entry.path.clone()]);
                        }
                    }
                    if !entry.folder {
                        let path = entry.path.clone();
                        response.context_menu(|ui| {
                            if ui.button("Import").clicked() {
                                action = Some(Action::Import(self.selected_files()));
                            }
                            if let Some(source) = source {
                                if ui.button("Open in Source Monitor").clicked() {
                                    action = Some(Action::Open(source));
                                }
                            }
                            if ui.button("Reveal in Explorer").clicked() {
                                action = Some(Action::Reveal(path));
                            }
                        });
                    }
                }
            });
        if let Some(folder) = entered {
            self.open_folder(folder);
        }
        if self.shown.is_empty() {
            let message = match (&self.looking, &self.folder, &self.error) {
                (_, _, Some(error)) => error.as_str(),
                (Some(_), None, _) => "Reading the folder…",
                (_, Some(_), _) if !self.search.trim().is_empty() => "Nothing matches the search.",
                (_, Some(_), _) => "No folders or media here.",
                _ => "",
            };
            ui.painter().text(
                output.inner_rect.center_top() + vec2(0.0, 24.0),
                Align2::CENTER_CENTER,
                message,
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().weak_text_color(),
            );
        }
        action
    }
}

fn paint_row(ui: &Ui, rect: Rect, entry: &Entry, selected: bool, hovered: bool, imported: bool) {
    let visuals = ui.visuals();
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 3.0, visuals.selection.bg_fill);
    } else if hovered {
        painter.rect_filled(rect, 3.0, visuals.widgets.hovered.weak_bg_fill);
    }
    let (icon, tint) = file_icon(entry.folder, &entry.path);
    let glyph = Rect::from_center_size(
        rect.left_center() + vec2(6.0 + ICON / 2.0, 0.0),
        vec2(ICON, ICON),
    );
    icons::paint(painter, glyph, icon, tint);
    let text_color = if selected {
        visuals.selection.stroke.color
    } else if imported {
        visuals.weak_text_color()
    } else {
        visuals.text_color()
    };
    painter.with_clip_rect(rect.shrink2(vec2(4.0, 0.0))).text(
        rect.left_center() + vec2(NAME_LEFT, 0.0),
        Align2::LEFT_CENTER,
        &entry.name,
        egui::TextStyle::Body.resolve(ui.style()),
        text_color,
    );
}

pub(crate) fn ui(editor: &mut Editor, ui: &mut Ui) {
    let browser = &mut editor.media_browser;
    if !browser.started {
        let first = editor
            .browse_folders
            .get(&FolderUse::Media)
            .cloned()
            .or_else(|| editor.project_path.as_deref()?.parent().map(Path::to_owned));
        browser.start(first);
    }
    if let Some(folder) = browser.take_found() {
        editor.browse_folders.insert(FolderUse::Media, folder);
    }
    let browser = &mut editor.media_browser;
    if browser.looking.is_some() {
        ui.ctx().request_repaint_after(Duration::from_millis(30));
    } else if let Some(folder) = &browser.folder {
        let due = browser
            .listed_at
            .is_none_or(|listed| listed.elapsed() >= RELIST_EVERY);
        if due {
            let folder = folder.clone();
            browser.look(folder);
        }
        ui.ctx().request_repaint_after(RELIST_EVERY);
    }
    browser.location_bar(ui);
    let search = ui.add(
        TextEdit::singleline(&mut browser.search)
            .hint_text("Search this folder")
            .desired_width(f32::INFINITY),
    );
    if search.changed() {
        browser.refilter();
    }
    ui.add_space(4.0);
    let media = &editor.timeline.project().media;
    let in_project: HashMap<&Path, SourceId> = media
        .iter()
        .filter(|item| item.is_file())
        .map(|item| (item.path.as_path(), item.id))
        .collect();
    let footer = ROW_HEIGHT + 10.0;
    let height = (ui.available_height() - footer).max(ROW_HEIGHT);
    let browser = &mut editor.media_browser;
    let mut action = browser.entries_list(ui, height, &in_project);
    let files = browser.selected_files();
    let new: Vec<PathBuf> = files
        .iter()
        .filter(|path| !in_project.contains_key(path.as_path()))
        .cloned()
        .collect();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let count = browser.shown.len();
        ui.weak(match count {
            1 => "1 item".to_owned(),
            _ => format!("{count} items"),
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let import = ui
                .add_enabled(!new.is_empty(), egui::Button::new("Import"))
                .on_hover_text("Adds the selected files to the project")
                .on_disabled_hover_text(if files.is_empty() {
                    "Select media files to import"
                } else {
                    "Already in the project"
                });
            if import.clicked() {
                action = Some(Action::Import(new.clone()));
            }
        });
    });
    let Some(action) = action else {
        return;
    };
    match action {
        Action::Import(paths) => {
            let paths: Vec<PathBuf> = paths
                .into_iter()
                .filter(|path| !in_project.contains_key(path.as_path()))
                .collect();
            if paths.is_empty() {
                editor.notice("The selected files are already in the project");
            }
            for path in paths {
                editor.import(path, None);
            }
        }
        Action::Open(source) => editor.open_source(source),
        Action::Reveal(path) => editor.reveal_file(&path),
    }
}
