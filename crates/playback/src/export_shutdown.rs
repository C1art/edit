use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// The cancel flag of every export thread still running.
static RUNNING: Mutex<Vec<Arc<AtomicBool>>> = Mutex::new(Vec::new());
static ONE_ENDED: Condvar = Condvar::new();

fn running() -> MutexGuard<'static, Vec<Arc<AtomicBool>>> {
    RUNNING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Held by an export thread until it has cleaned up after itself.
pub(crate) struct RunningExport(Arc<AtomicBool>);

impl RunningExport {
    pub(crate) fn new(cancel: &Arc<AtomicBool>) -> Self {
        running().push(Arc::clone(cancel));
        Self(Arc::clone(cancel))
    }
}

impl Drop for RunningExport {
    fn drop(&mut self) {
        running().retain(|cancel| !Arc::ptr_eq(cancel, &self.0));
        ONE_ENDED.notify_all();
    }
}

/// Cancels every export and waits up to `within` for them to remove their partial
/// files, which the process ending first would leave behind. True if all of them ended.
pub fn stop_exports(within: Duration) -> bool {
    let deadline = Instant::now() + within;
    let mut running = running();
    for cancel in running.iter() {
        cancel.store(true, Ordering::Relaxed);
    }
    while !running.is_empty() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        running = ONE_ENDED
            .wait_timeout(running, left)
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
    true
}
