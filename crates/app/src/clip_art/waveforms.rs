use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use crossbeam::channel::{self, Receiver, Sender};
use media::Peaks;

use super::peak_cache::PeakCache;

/// Files read at once, so one long file doesn't hold up the short ones behind it.
const READS_AT_ONCE: usize = 3;

/// Reads audio peaks on worker threads, from the peak file left by an earlier session
/// where there is one.
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
        let cache = PeakCache::new(crate::project_store::app_folder().join("Peak Files"));
        for index in 0..READS_AT_ONCE {
            let (requests, replies) = (request_rx.clone(), reply_tx.clone());
            let (cache, stop) = (cache.clone(), Arc::clone(&cancel));
            let spawned = thread::Builder::new()
                .name(format!("waveforms {index}"))
                .spawn(move || {
                    if index == 0 {
                        cache.clear_unused();
                    }
                    for path in requests {
                        let peaks = read(&cache, &path, &stop);
                        if replies.send((path, peaks)).is_err() {
                            break;
                        }
                    }
                });
            // One worker is enough to draw waveforms; more only make it quicker.
            if let Err(error) = spawned {
                if index == 0 {
                    return Err(error);
                }
                break;
            }
        }
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

fn read(cache: &PeakCache, path: &Path, stop: &AtomicBool) -> Option<Peaks> {
    cache.read(path).or_else(|| {
        let peaks = media::read_peaks(path, |_| {
            if stop.load(Ordering::Relaxed) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .ok()?;
        cache.write(path, &peaks);
        Some(peaks)
    })
}

impl Drop for WaveformWorker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
