use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crossbeam::channel::{self, Receiver, Sender};

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub folder: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

/// A folder to jump to from the side of the browser.
#[derive(Debug, Clone)]
pub struct Place {
    pub name: String,
    pub path: PathBuf,
}

pub enum Found {
    Folder(PathBuf, Vec<Entry>),
    File(PathBuf),
    Missing(PathBuf, String),
    Places(Vec<Place>),
}

enum Request {
    Look(PathBuf),
    Places,
}

/// Reads folders on a thread of its own, so a slow disk or network share never holds
/// up the window.
pub struct Lister {
    requests: Sender<Request>,
    found: Receiver<Found>,
}

impl Lister {
    pub fn new() -> io::Result<Self> {
        let (requests, asked) = channel::unbounded();
        let (answer, found) = channel::unbounded();
        std::thread::Builder::new()
            .name("file browser".into())
            .spawn(move || {
                for request in asked {
                    let found = match request {
                        Request::Look(path) => look(path),
                        Request::Places => Found::Places(places()),
                    };
                    if answer.send(found).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self { requests, found })
    }

    /// Lists `path` if it's a folder, or says it's a file or isn't there.
    pub fn look(&self, path: PathBuf) {
        // The thread only stops when this is dropped.
        let _ = self.requests.send(Request::Look(path));
    }

    pub fn find_places(&self) {
        let _ = self.requests.send(Request::Places);
    }

    pub fn poll(&self) -> Option<Found> {
        self.found.try_recv().ok()
    }
}

fn look(path: PathBuf) -> Found {
    match fs::metadata(&path) {
        Ok(metadata) if metadata.is_dir() => match list(&path) {
            Ok(entries) => Found::Folder(path, entries),
            Err(error) => Found::Missing(path, error.to_string()),
        },
        Ok(_) => Found::File(path),
        Err(error) => Found::Missing(path, error.to_string()),
    }
}

fn list(folder: &Path) -> io::Result<Vec<Entry>> {
    let mut entries: Vec<Entry> = fs::read_dir(folder)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            // Links are followed, so a link to a folder opens like one.
            let metadata = fs::metadata(&path).ok()?;
            (!is_hidden(&name, &metadata)).then(|| Entry {
                name,
                path,
                folder: metadata.is_dir(),
                size: metadata.len(),
                modified: metadata.modified().ok(),
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        b.folder
            .cmp(&a.folder)
            .then(natural_order(&a.name, &b.name))
    });
    Ok(entries)
}

#[cfg(windows)]
fn is_hidden(_name: &str, metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const HIDDEN: u32 = 0x2;
    const SYSTEM: u32 = 0x4;
    metadata.file_attributes() & (HIDDEN | SYSTEM) != 0
}

#[cfg(not(windows))]
fn is_hidden(name: &str, _metadata: &fs::Metadata) -> bool {
    name.starts_with('.')
}

fn places() -> Vec<Place> {
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    let mut places: Vec<Place> = home
        .iter()
        .flat_map(|home| {
            let named = [
                "Desktop",
                "Documents",
                "Downloads",
                "Videos",
                "Music",
                "Pictures",
            ]
            .map(|name| Place {
                name: name.to_owned(),
                path: home.join(name),
            });
            let home = Place {
                name: "Home".to_owned(),
                path: home.clone(),
            };
            std::iter::once(home).chain(named)
        })
        .filter(|place| place.path.is_dir())
        .collect();
    places.extend(drives());
    places
}

#[cfg(windows)]
fn drives() -> Vec<Place> {
    // A and B are left out: asking after an empty floppy drive can stall for seconds.
    ('C'..='Z')
        .map(|letter| Place {
            name: format!("{letter}:"),
            path: PathBuf::from(format!("{letter}:\\")),
        })
        .filter(|drive| drive.path.is_dir())
        .collect()
}

#[cfg(not(windows))]
fn drives() -> Vec<Place> {
    vec![Place {
        name: "Computer".to_owned(),
        path: PathBuf::from("/"),
    }]
}

/// Orders names the way Explorer does: ignoring case, and with runs of digits compared
/// as numbers, so `shot 2` comes before `shot 10`.
pub fn natural_order(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (x, y) = (digits(&mut a), digits(&mut b));
                let (x_value, y_value) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                let order = x_value
                    .len()
                    .cmp(&y_value.len())
                    .then_with(|| x_value.cmp(y_value))
                    .then_with(|| x.len().cmp(&y.len()));
                if order.is_ne() {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order.is_ne() {
                    return order;
                }
                a.next();
                b.next();
            }
        }
    }
}

fn digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    std::iter::from_fn(|| chars.next_if(char::is_ascii_digit)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_in_names_order_by_value_and_letters_ignore_case() {
        let mut names = [
            "Shot 10", "shot 2", "Shot 02b", "B-roll", "a-roll", "shot 1",
        ];
        names.sort_by(|a, b| natural_order(a, b));
        assert_eq!(
            names,
            ["a-roll", "B-roll", "shot 1", "shot 2", "Shot 02b", "Shot 10"]
        );
    }
}
