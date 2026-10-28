use std::path::PathBuf;
use std::thread;

use crossbeam::channel::{self, Receiver, Sender};
use project::ProjectFile;
use timeline::{captions_from_cues, parse_srt, to_srt, Caption, Sequence, Timebase};

enum Request {
    Save {
        path: PathBuf,
        sequence: Box<Sequence>,
        revision: u64,
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
}

pub enum StoreEvent {
    /// The timeline revision that was written, so later edits still count as unsaved.
    Saved(PathBuf, u64, Result<(), String>),
    Opened(PathBuf, Result<Box<Sequence>, String>),
    Autosaved(PathBuf, Result<(), String>),
    CaptionsRead(PathBuf, Result<Vec<Caption>, String>),
    CaptionsWritten(PathBuf, Result<(), String>),
}

/// Reads and writes project files on a worker thread so the UI never touches the disk.
pub struct ProjectStore {
    requests: Sender<Request>,
    events: Receiver<StoreEvent>,
    pending: usize,
}

impl ProjectStore {
    pub fn new() -> std::io::Result<Self> {
        let (request_tx, request_rx) = channel::unbounded::<Request>();
        let (event_tx, event_rx) = channel::unbounded();
        thread::Builder::new()
            .name("project".to_owned())
            .spawn(move || {
                for request in request_rx {
                    if event_tx.send(handle(request)).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests: request_tx,
            events: event_rx,
            pending: 0,
        })
    }

    pub fn save(&mut self, path: PathBuf, sequence: Sequence, revision: u64) {
        let request = Request::Save {
            path,
            sequence: Box::new(sequence),
            revision,
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

fn handle(request: Request) -> StoreEvent {
    match request {
        Request::Save {
            path,
            sequence,
            revision,
        } => {
            let result = ProjectFile::new(*sequence)
                .save(&path)
                .map_err(|error| error.to_string());
            StoreEvent::Saved(path, revision, result)
        }
        Request::Open(path) => {
            let result = ProjectFile::load(&path)
                .map(|file| Box::new(file.sequence))
                .map_err(|error| error.to_string());
            StoreEvent::Opened(path, result)
        }
        Request::Autosave { path, sequence } => {
            let result = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .map_err(|error| error.to_string())
                .and_then(|()| {
                    ProjectFile::new(*sequence)
                        .save(&path)
                        .map_err(|error| error.to_string())
                });
            StoreEvent::Autosaved(path, result)
        }
        Request::ReadCaptions(path, timebase) => {
            let result = std::fs::read_to_string(&path)
                .map_err(|error| error.to_string())
                .and_then(|text| parse_srt(&text, timebase).map_err(|error| error.to_string()))
                .map(captions_from_cues);
            StoreEvent::CaptionsRead(path, result)
        }
        Request::WriteCaptions {
            path,
            captions,
            timebase,
        } => {
            let result = std::fs::write(&path, to_srt(&captions, timebase))
                .map_err(|error| error.to_string());
            StoreEvent::CaptionsWritten(path, result)
        }
    }
}
