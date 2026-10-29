use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use crossbeam::channel::{self, Receiver, Sender};
use media::Peaks;

use super::peak_cache::PeakCache;

/// Reads the audio peaks of one file at a time on a worker thread, from the peak file
/// left by an earlier session where there is one.
pub struct WaveformWorker {
    requests: Sender<PathBuf>,
    replies: Receiver<(PathBuf, Option<Peaks>)>,
    cancel: Arc<AtomicBool>,
}

impl WaveformWorker {
    pub fn new() -> std::io::Result<Self> {
        let (request_tx, request_rx) = channel::unbounded::<PathBuf>();
        let (reply_tx, reply_rx) = channel::unbounded();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&cancel);
        let cache = PeakCache::new(crate::project_store::app_folder().join("Peak Files"));
        thread::Builder::new()
            .name("waveforms".to_owned())
            .spawn(move || {
                cache.clear_unused();
                for path in request_rx {
                    let peaks = cache.read(&path).or_else(|| {
                        let peaks = media::read_peaks(&path, |_| {
                            if stop.load(Ordering::Relaxed) {
                                ControlFlow::Break(())
                            } else {
                                ControlFlow::Continue(())
                            }
                        })
                        .ok()?;
                        cache.write(&path, &peaks);
                        Some(peaks)
                    });
                    if reply_tx.send((path, peaks)).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests: request_tx,
            replies: reply_rx,
            cancel,
        })
    }

    /// False once the worker has stopped.
    pub fn request(&self, path: PathBuf) -> bool {
        self.requests.send(path).is_ok()
    }

    pub fn poll(&self) -> impl Iterator<Item = (PathBuf, Option<Peaks>)> + '_ {
        self.replies.try_iter()
    }
}

impl Drop for WaveformWorker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
