use std::collections::HashMap;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use crossbeam::channel::{self, Receiver, Sender};
use timeline::{SetProxy, SourceId};

use crate::editor::Editor;

/// Tall enough to judge focus and framing, small enough to decode several at once.
const PROXY_HEIGHT: u32 = 540;
/// Progress reports per job, so the UI hears about each percent at most.
const PROGRESS_STEPS: f32 = 100.0;

struct Request {
    source: SourceId,
    path: PathBuf,
}

pub enum ProxyEvent {
    Progress(SourceId, f32),
    Finished(SourceId, Result<PathBuf, String>),
}

/// Builds proxies one at a time on a worker thread.
pub struct ProxyMaker {
    requests: Sender<Request>,
    events: Receiver<ProxyEvent>,
    /// Fraction done of each queued or running job.
    jobs: HashMap<SourceId, f32>,
    cancel: Arc<AtomicBool>,
}

impl ProxyMaker {
    pub fn new() -> std::io::Result<Self> {
        let (request_tx, request_rx) = channel::unbounded::<Request>();
        let (event_tx, event_rx) = channel::unbounded();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&cancel);
        thread::Builder::new()
            .name("proxies".to_owned())
            .spawn(move || {
                for request in request_rx {
                    let result = make(&request, &stop, &event_tx);
                    if event_tx
                        .send(ProxyEvent::Finished(request.source, result))
                        .is_err()
                    {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests: request_tx,
            events: event_rx,
            jobs: HashMap::new(),
            cancel,
        })
    }

    pub fn request(&mut self, source: SourceId, path: PathBuf) {
        if self.jobs.contains_key(&source) {
            return;
        }
        if self.requests.send(Request { source, path }).is_ok() {
            self.jobs.insert(source, 0.0);
        }
    }

    /// The fraction done, while a proxy for `source` is queued or being made.
    pub fn progress(&self, source: SourceId) -> Option<f32> {
        self.jobs.get(&source).copied()
    }

    pub fn is_busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// Finished jobs since the last poll.
    pub fn poll(&mut self) -> Vec<(SourceId, Result<PathBuf, String>)> {
        let mut finished = Vec::new();
        for event in self.events.try_iter() {
            match event {
                ProxyEvent::Progress(source, done) => {
                    if let Some(progress) = self.jobs.get_mut(&source) {
                        *progress = done;
                    }
                }
                ProxyEvent::Finished(source, result) => {
                    self.jobs.remove(&source);
                    finished.push((source, result));
                }
            }
        }
        finished
    }
}

impl Editor {
    pub(crate) fn poll_proxies(&mut self, ctx: &egui::Context) {
        let Some(maker) = &mut self.proxies else {
            return;
        };
        let finished = maker.poll();
        if maker.is_busy() {
            ctx.request_repaint();
        }
        for (source, result) in finished {
            match result {
                Ok(path) => {
                    self.apply(SetProxy {
                        source,
                        proxy: Some(path),
                    });
                }
                Err(error) => self.notice(format!("Cannot create proxy: {error}")),
            }
        }
    }
}

impl Drop for ProxyMaker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Where the proxy of `media` goes: a `Proxies` folder beside it, as other editors do.
pub fn proxy_path(media: &Path) -> PathBuf {
    let stem = media
        .file_stem()
        .map_or_else(|| "media".into(), |stem| stem.to_string_lossy());
    let folder = media.parent().unwrap_or(Path::new("")).join("Proxies");
    folder.join(format!("{stem}_Proxy.mp4"))
}

fn make(
    request: &Request,
    cancel: &AtomicBool,
    events: &Sender<ProxyEvent>,
) -> Result<PathBuf, String> {
    let destination = proxy_path(&request.path);
    if let Some(folder) = destination.parent() {
        std::fs::create_dir_all(folder)
            .map_err(|error| format!("cannot create {}: {error}", folder.display()))?;
    }
    let mut reported = 0.0;
    media::create_proxy(&request.path, &destination, PROXY_HEIGHT, |done| {
        if cancel.load(Ordering::Relaxed) {
            return ControlFlow::Break(());
        }
        let step = (done * PROGRESS_STEPS).floor() / PROGRESS_STEPS;
        if step > reported {
            reported = step;
            let _ = events.send(ProxyEvent::Progress(request.source, step));
        }
        ControlFlow::Continue(())
    })
    .map_err(|error| error.to_string())?;
    Ok(destination)
}
