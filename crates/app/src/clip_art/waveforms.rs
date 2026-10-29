use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use crossbeam::channel::{self, Receiver, Sender};
use media::Peaks;

/// Reads the audio peaks of one file at a time on a worker thread.
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
        thread::Builder::new()
            .name("waveforms".to_owned())
            .spawn(move || {
                for path in request_rx {
                    let peaks = media::read_peaks(&path, |_| {
                        if stop.load(Ordering::Relaxed) {
                            ControlFlow::Break(())
                        } else {
                            ControlFlow::Continue(())
                        }
                    });
                    if reply_tx.send((path, peaks.ok())).is_err() {
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
