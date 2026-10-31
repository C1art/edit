use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam::channel::{self, Receiver, Sender};
use timeline::{Sequence, SourceId};

/// How often files are looked for again, so ones moved or restored while the project is
/// open are noticed.
const RECHECK: Duration = Duration::from_secs(5);

/// Which project a request was asked for, as source ids start over in each.
type Generation = u64;

enum Request {
    Check(Vec<(SourceId, PathBuf)>),
    /// Looks in a folder for files named as the missing ones were.
    Find {
        folder: PathBuf,
        wanted: Vec<(SourceId, PathBuf)>,
    },
}

enum Reply {
    Missing(HashSet<SourceId>),
    Found(Vec<(SourceId, PathBuf)>),
}

/// Which media files are missing, found out on a worker thread so the UI never touches
/// the disk.
pub struct OfflineMedia {
    requests: Sender<(Generation, Request)>,
    replies: Receiver<(Generation, Reply)>,
    missing: HashSet<SourceId>,
    /// The files last asked about, so a changed pool is checked straight away.
    checked: Vec<(SourceId, PathBuf)>,
    checked_at: Option<Instant>,
    checking: bool,
    generation: Generation,
    /// Missing media the user chose to leave offline, so Link Media stops asking.
    pub(crate) left_offline: HashSet<SourceId>,
}

impl OfflineMedia {
    pub fn new() -> std::io::Result<Self> {
        let (request_tx, request_rx) = channel::unbounded();
        let (reply_tx, reply_rx) = channel::unbounded();
        thread::Builder::new()
            .name("offline media".to_owned())
            .spawn(move || {
                for (generation, request) in request_rx {
                    let reply = match request {
                        Request::Check(files) => Reply::Missing(
                            files
                                .into_iter()
                                .filter(|(_, path)| !media::source_exists(path))
                                .map(|(id, _)| id)
                                .collect(),
                        ),
                        Request::Find { folder, wanted } => Reply::Found(find(&folder, wanted)),
                    };
                    if reply_tx.send((generation, reply)).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests: request_tx,
            replies: reply_rx,
            missing: HashSet::new(),
            checked: Vec::new(),
            checked_at: None,
            checking: false,
            generation: 0,
            left_offline: HashSet::new(),
        })
    }

    pub fn is_offline(&self, id: SourceId) -> bool {
        self.missing.contains(&id)
    }

    pub fn missing(&self) -> &HashSet<SourceId> {
        &self.missing
    }

    /// Counts `id` as found until the next check says otherwise, so it isn't searched
    /// for again meanwhile.
    pub fn relinked(&mut self, id: SourceId) {
        self.missing.remove(&id);
    }

    /// The missing media not yet left offline, in pool order.
    pub fn unresolved<'a>(&'a self, sequence: &'a Sequence) -> impl Iterator<Item = SourceId> + 'a {
        sequence
            .media
            .iter()
            .map(|item| item.id)
            .filter(|id| self.missing.contains(id) && !self.left_offline.contains(id))
    }

    /// Forgets what was known, for a project that was just opened.
    pub fn forget(&mut self) {
        self.missing.clear();
        self.left_offline.clear();
        self.checked.clear();
        self.checked_at = None;
        self.checking = false;
        self.generation += 1;
    }

    /// Asks for the other missing files in `folder`, where one of them that was in
    /// `old_folder` was just found. Cameras reuse names such as `C0001.MP4`, so a name
    /// more than one item has only matches the items that were kept beside the found one.
    pub fn find_in(
        &mut self,
        folder: PathBuf,
        old_folder: Option<&Path>,
        sequence: &Sequence,
        except: SourceId,
    ) {
        let shared = |name: Option<&std::ffi::OsStr>| {
            let named = |item: &&timeline::MediaItem| item.path.file_name() == name;
            sequence.media.iter().filter(named).nth(1).is_some()
        };
        let wanted = sequence
            .media
            .iter()
            .filter(|item| item.id != except && self.missing.contains(&item.id))
            .filter(|item| item.path.parent() == old_folder || !shared(item.path.file_name()))
            .map(|item| (item.id, item.path.clone()))
            .collect();
        // The worker answers in order, so a check already asked for can't undo this.
        let _ = self
            .requests
            .send((self.generation, Request::Find { folder, wanted }));
    }

    /// Takes the worker's answers and asks again when the pool changed or a while has
    /// passed. Returns files found for missing media.
    pub fn poll(&mut self, sequence: &Sequence) -> Vec<(SourceId, PathBuf)> {
        let mut found = Vec::new();
        let current = self.generation;
        let replies = self.replies.try_iter();
        for (_, reply) in replies.filter(|(generation, _)| *generation == current) {
            match reply {
                Reply::Missing(missing) => {
                    self.missing = missing;
                    self.checking = false;
                }
                Reply::Found(files) => {
                    for (id, _) in &files {
                        self.missing.remove(id);
                    }
                    found.extend(files);
                }
            }
        }
        if self.checking {
            return found;
        }
        let files: Vec<(SourceId, PathBuf)> = sequence
            .media
            .iter()
            .filter(|item| item.is_file() && !item.offline)
            .map(|item| (item.id, item.path.clone()))
            .collect();
        let stale = self
            .checked_at
            .is_none_or(|checked_at| checked_at.elapsed() >= RECHECK);
        if (stale || files != self.checked)
            && self
                .requests
                .send((self.generation, Request::Check(files.clone())))
                .is_ok()
        {
            self.checked = files;
            self.checked_at = Some(Instant::now());
            self.checking = true;
        }
        found
    }
}

/// The files in `folder` named as the `wanted` ones were.
fn find(folder: &Path, wanted: Vec<(SourceId, PathBuf)>) -> Vec<(SourceId, PathBuf)> {
    wanted
        .into_iter()
        .filter_map(|(id, old)| {
            let candidate = folder.join(old.file_name()?);
            (candidate != old && media::source_exists(&candidate)).then_some((id, candidate))
        })
        .collect()
}
