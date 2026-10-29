use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::thread;

use crossbeam::channel::{self, Receiver, Sender};

const EXTENSION: &str = "mov";
/// Marks a file still being written, so one cut short is never taken for a preview.
const PARTIAL: &str = ".rendering";

enum Request {
    Scan,
    /// Gives a finished render its preview's name.
    Keep {
        partial: PathBuf,
        fingerprint: u64,
    },
    DeleteAll,
}

pub(super) enum Reply {
    Rendered(HashSet<u64>),
    Kept(u64),
    Deleted,
    Failed(String),
}

/// The preview files on disk, named by the fingerprint of what they show, read and
/// written on a worker thread so the UI never touches the disk.
pub(super) struct PreviewFiles {
    folder: PathBuf,
    requests: Sender<Request>,
    replies: Receiver<Reply>,
}

impl PreviewFiles {
    pub(super) fn new(folder: PathBuf) -> std::io::Result<Self> {
        let (request_tx, request_rx) = channel::unbounded();
        let (reply_tx, reply_rx) = channel::unbounded();
        let worker_folder = folder.clone();
        thread::Builder::new()
            .name("preview files".to_owned())
            .spawn(move || {
                for request in request_rx {
                    let reply = serve(&worker_folder, request)
                        .unwrap_or_else(|error| Reply::Failed(error.to_string()));
                    if reply_tx.send(reply).is_err() {
                        break;
                    }
                }
            })?;
        let files = Self {
            folder,
            requests: request_tx,
            replies: reply_rx,
        };
        let _ = files.requests.send(Request::Scan);
        Ok(files)
    }

    pub(super) fn partial_path(&self, fingerprint: u64) -> PathBuf {
        self.folder
            .join(format!("{fingerprint:016x}{PARTIAL}.{EXTENSION}"))
    }

    pub(super) fn path(&self, fingerprint: u64) -> PathBuf {
        preview_path(&self.folder, fingerprint)
    }

    pub(super) fn keep(&self, partial: PathBuf, fingerprint: u64) {
        let _ = self.requests.send(Request::Keep {
            partial,
            fingerprint,
        });
    }

    pub(super) fn delete_all(&self) {
        let _ = self.requests.send(Request::DeleteAll);
    }

    pub(super) fn poll(&self) -> Vec<Reply> {
        self.replies.try_iter().collect()
    }
}

fn preview_path(folder: &Path, fingerprint: u64) -> PathBuf {
    folder.join(format!("{fingerprint:016x}.{EXTENSION}"))
}

fn serve(folder: &Path, request: Request) -> std::io::Result<Reply> {
    match request {
        Request::Scan => {
            // Renders write straight into the folder, so it's made before any starts.
            std::fs::create_dir_all(folder)?;
            Ok(Reply::Rendered(scan(folder)))
        }
        Request::Keep {
            partial,
            fingerprint,
        } => {
            std::fs::rename(&partial, preview_path(folder, fingerprint))?;
            Ok(Reply::Kept(fingerprint))
        }
        Request::DeleteAll => {
            for entry in std::fs::read_dir(folder)?.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .is_some_and(|extension| extension == EXTENSION)
                {
                    std::fs::remove_file(path)?;
                }
            }
            Ok(Reply::Deleted)
        }
    }
}

/// The fingerprints of the previews in `folder`.
fn scan(folder: &Path) -> HashSet<u64> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return HashSet::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()? != EXTENSION {
                return None;
            }
            let stem = path.file_stem()?.to_str()?;
            (stem.len() == 16)
                .then(|| u64::from_str_radix(stem, 16).ok())
                .flatten()
        })
        .collect()
}
