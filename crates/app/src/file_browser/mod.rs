//! A file dialog drawn in the window, for picking files to open or a name to save to.

mod filter;
mod listing;
mod view;

use std::path::{Path, PathBuf};

pub use filter::*;
use listing::{natural_order, Entry, Found, Lister, Place};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Picks files that are there, one or, with `many`, several.
    Open { many: bool },
    /// Picks a name to write to, asking before it replaces a file.
    Save,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Chosen(Vec<PathBuf>),
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortBy {
    Name,
    Size,
    Modified,
}

/// What finding out about a path is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Intent {
    Show,
    /// Shows a folder, else picks the file: what typing a name and Enter does.
    Choose,
}

pub struct FileBrowser {
    mode: Mode,
    filters: Vec<FileFilter>,
    filter: usize,
    lister: Option<Lister>,
    looking: Option<(PathBuf, Intent)>,
    folder: Option<PathBuf>,
    location: String,
    entries: Vec<Entry>,
    /// Indices into `entries` of those the filter shows, in the order they're listed.
    shown: Vec<usize>,
    sort: (SortBy, bool),
    selected: Vec<usize>,
    /// Where a shift-click range starts, as a position in `shown`.
    anchor: Option<usize>,
    name: String,
    name_from_selection: bool,
    places: Vec<Place>,
    back: Vec<PathBuf>,
    error: Option<String>,
    replacing: Option<PathBuf>,
    focus_name: bool,
    scroll_to: Option<usize>,
    /// How far the list was scrolled and how tall it showed, when last drawn.
    view: (f32, f32),
}

impl FileBrowser {
    /// Opens in the folder of `start`, with its file name typed, or in `folder`.
    pub fn new(
        mode: Mode,
        filters: Vec<FileFilter>,
        start: Option<&Path>,
        folder: Option<PathBuf>,
        project_folder: Option<PathBuf>,
    ) -> Self {
        let lister = Lister::new().ok();
        let name = start
            .filter(|_| mode == Mode::Save)
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let first = start
            .and_then(Path::parent)
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_owned)
            .or(folder)
            .or_else(|| project_folder.clone())
            .or_else(home);
        let places = project_folder
            .map(|path| Place {
                name: "Project".to_owned(),
                path,
            })
            .into_iter()
            .collect();
        let mut browser = Self {
            mode,
            filters,
            filter: 0,
            lister,
            looking: None,
            folder: None,
            location: String::new(),
            entries: Vec::new(),
            shown: Vec::new(),
            sort: (SortBy::Name, false),
            selected: Vec::new(),
            anchor: None,
            name,
            name_from_selection: false,
            places,
            back: Vec::new(),
            error: None,
            replacing: None,
            focus_name: true,
            scroll_to: None,
            view: (0.0, 0.0),
        };
        match &browser.lister {
            Some(lister) => lister.find_places(),
            None => browser.error = Some("Cannot list folders: no thread for it".to_owned()),
        }
        if let Some(first) = first {
            browser.look(first, Intent::Show);
        }
        browser
    }

    /// The folder the browser shows, to open in next time.
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    fn filter(&self) -> FileFilter {
        self.filters.get(self.filter).copied().unwrap_or(ALL_FILES)
    }

    fn look(&mut self, path: PathBuf, intent: Intent) {
        if let Some(lister) = &self.lister {
            lister.look(path.clone());
            self.looking = Some((path, intent));
        }
    }

    fn go_up(&mut self) {
        if let Some(parent) = self.folder.as_deref().and_then(Path::parent) {
            self.look(parent.to_owned(), Intent::Show);
        }
    }

    fn go_back(&mut self) {
        if let Some(previous) = self.back.pop() {
            // Going back shouldn't add the folder left to the history again.
            self.folder = None;
            self.look(previous, Intent::Show);
        }
    }

    /// Takes in what the lister found; a choice it settles comes back.
    fn take_found(&mut self) -> Option<Outcome> {
        let mut chosen = None;
        while let Some(found) = self.lister.as_ref().and_then(Lister::poll) {
            match found {
                Found::Places(places) => self.places.extend(places),
                Found::Folder(path, entries) => {
                    if self
                        .looking
                        .as_ref()
                        .is_some_and(|(asked, _)| *asked == path)
                    {
                        self.looking = None;
                        self.show_folder(path, entries);
                    }
                }
                Found::File(path) => {
                    if self.looking.take_if(|(asked, _)| *asked == path).is_some() {
                        chosen = self.choose(vec![path], true);
                    }
                }
                Found::Missing(path, error) => {
                    let Some((_, intent)) = self.looking.take_if(|(asked, _)| *asked == path)
                    else {
                        continue;
                    };
                    if intent == Intent::Choose && self.mode == Mode::Save {
                        chosen = Some(Outcome::Chosen(vec![path]));
                    } else if self.folder.is_none() && intent == Intent::Show {
                        // Where it was to open is gone, so it opens at home instead.
                        match home().filter(|home| *home != path) {
                            Some(home) => self.look(home, Intent::Show),
                            None => self.error = Some(error),
                        }
                    } else {
                        self.error = Some(format!("{}: {error}", path.display()));
                    }
                }
            }
        }
        chosen
    }

    fn show_folder(&mut self, path: PathBuf, entries: Vec<Entry>) {
        if let Some(left) = self.folder.take().filter(|left| *left != path) {
            self.back.push(left);
        }
        self.location = path.display().to_string();
        self.folder = Some(path);
        self.entries = entries;
        self.selected.clear();
        self.anchor = None;
        self.error = None;
        self.replacing = None;
        if self.name_from_selection {
            self.name.clear();
            self.name_from_selection = false;
        }
        self.focus_name = true;
        self.scroll_to = Some(0);
        self.sort_entries();
    }

    fn sort_entries(&mut self) {
        let filter = self.filter();
        let selected: Vec<PathBuf> = self.selected_paths().map(Path::to_owned).collect();
        let (by, descending) = self.sort;
        let order = |a: &Entry, b: &Entry| {
            let order = match by {
                SortBy::Name => natural_order(&a.name, &b.name),
                SortBy::Size => a.size.cmp(&b.size),
                SortBy::Modified => a.modified.cmp(&b.modified),
            };
            if descending {
                order.reverse()
            } else {
                order
            }
        };
        let entries = &mut self.entries;
        entries.sort_by(|a, b| {
            b.folder
                .cmp(&a.folder)
                .then_with(|| order(a, b))
                .then_with(|| natural_order(&a.name, &b.name))
        });
        self.shown = (0..entries.len())
            .filter(|&index| entries[index].folder || filter.shows(&entries[index].path))
            .collect();
        self.selected = self
            .shown
            .iter()
            .copied()
            .filter(|&index| selected.contains(&entries[index].path))
            .collect();
    }

    fn selected_paths(&self) -> impl Iterator<Item = &Path> {
        self.selected
            .iter()
            .filter_map(|&index| self.entries.get(index))
            .map(|entry| entry.path.as_path())
    }

    /// Selects the entry listed at `row`, alone, toggled into or out of the selection,
    /// or with all those from the last one picked.
    fn select(&mut self, row: usize, toggle: bool, range: bool) {
        let Some(&index) = self.shown.get(row) else {
            return;
        };
        let many = self.mode == Mode::Open { many: true };
        match self.anchor {
            Some(anchor) if range && many => {
                let (from, to) = (anchor.min(row), anchor.max(row));
                self.selected = self.shown[from..=to].to_vec();
            }
            _ if toggle && many => {
                match self.selected.iter().position(|&selected| selected == index) {
                    Some(at) => {
                        self.selected.remove(at);
                    }
                    None => self.selected.push(index),
                }
                self.anchor = Some(row);
            }
            _ => {
                self.selected = vec![index];
                self.anchor = Some(row);
            }
        }
        let files: Vec<&str> = self
            .selected
            .iter()
            .map(|&index| &self.entries[index])
            .filter(|entry| !entry.folder)
            .map(|entry| entry.name.as_str())
            .collect();
        match files.as_slice() {
            [] => {}
            [one] => self.name = (*one).to_owned(),
            several => {
                let quoted: Vec<String> =
                    several.iter().map(|name| format!("\"{name}\"")).collect();
                self.name = quoted.join(" ");
            }
        }
        self.name_from_selection = !files.is_empty();
        self.replacing = None;
    }

    fn open_row(&mut self, row: usize) -> Option<Outcome> {
        let entry = self.entries.get(*self.shown.get(row)?)?;
        if entry.folder {
            self.look(entry.path.clone(), Intent::Show);
            None
        } else {
            self.choose(vec![entry.path.clone()], true)
        }
    }

    /// What Enter or the action button does with what's typed or selected.
    fn submit(&mut self) -> Option<Outcome> {
        let typed = self.name.trim().trim_matches('"').to_owned();
        if self.name_from_selection {
            let files: Vec<PathBuf> = self
                .selected
                .iter()
                .map(|&index| &self.entries[index])
                .filter(|entry| !entry.folder)
                .map(|entry| entry.path.clone())
                .collect();
            if !files.is_empty() {
                return self.choose(files, true);
            }
        }
        if typed.is_empty() {
            let folder = self
                .selected
                .iter()
                .map(|&index| &self.entries[index])
                .find(|entry| entry.folder)?;
            self.look(folder.path.clone(), Intent::Show);
            return None;
        }
        let listed = self
            .entries
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(&typed));
        if let Some(entry) = listed.filter(|_| Path::new(&typed).is_relative()) {
            if entry.folder {
                self.look(entry.path.clone(), Intent::Show);
                return None;
            }
            return self.choose(vec![entry.path.clone()], true);
        }
        let mut path = match &self.folder {
            Some(folder) => folder.join(&typed),
            None => PathBuf::from(&typed),
        };
        if self.mode == Mode::Save && path.extension().is_none() {
            if let Some(extension) = self.filter().default_extension() {
                path.set_extension(extension);
            }
        }
        let parent = path.parent().map(Path::to_owned);
        if parent.as_ref() == self.folder.as_ref() {
            let listed = self.entries.iter().find(|entry| entry.path == path);
            match listed {
                Some(entry) if !entry.folder => return self.choose(vec![path], true),
                None if self.mode == Mode::Save => return self.choose(vec![path], false),
                _ => {}
            }
        }
        self.look(path, Intent::Choose);
        None
    }

    /// Settles on `paths`, unless saving over a file the user hasn't agreed to replace.
    fn choose(&mut self, paths: Vec<PathBuf>, exist: bool) -> Option<Outcome> {
        if self.mode == Mode::Save && exist {
            let path = paths.first()?;
            if self.replacing.as_ref() != Some(path) {
                self.replacing = Some(path.clone());
                return None;
            }
        }
        Some(Outcome::Chosen(paths))
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}
