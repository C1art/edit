use std::path::PathBuf;
use std::thread;

use crossbeam::channel::{bounded, Receiver};

use super::releases::{self, Release};
use super::{ApplyOnExit, Checked, ProgressReceiver};

/// The installer starts Cledit again with this after an update.
const UPDATED: &str = "--updated";

/// Takes the installer's flag out of the arguments; returns the version just updated to,
/// if started by an update.
pub(crate) fn start_up(args: &mut Vec<std::ffi::OsString>) -> Option<String> {
    let before = args.len();
    args.retain(|arg| arg != UPDATED);
    (args.len() != before).then(|| env!("CARGO_PKG_VERSION").to_owned())
}

fn on_worker<T: Send + 'static>(
    name: &str,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Receiver<Result<T, String>> {
    let (reply, replies) = bounded(1);
    // A thread that can't start drops `reply`, which the poller reads as a failure.
    let _ = thread::Builder::new()
        .name(name.into())
        .spawn(move || reply.send(work()));
    replies
}

pub(super) fn check() -> Receiver<Result<Checked, String>> {
    on_worker("update-check", || {
        if !releases::installed() {
            return Ok(Checked::NotInstalled);
        }
        let current = env!("CARGO_PKG_VERSION");
        let latest = releases::latest()?;
        Ok(if releases::is_newer(&latest.version, current) {
            Checked::Available(Box::new(latest))
        } else {
            Checked::UpToDate(current.to_owned())
        })
    })
}

pub(super) fn download(
    release: &Release,
) -> (ProgressReceiver<u8>, Receiver<Result<PathBuf, String>>) {
    let (progress, percent) = std::sync::mpsc::channel();
    let installer = release.installer.clone();
    let done = on_worker("update-download", move || {
        releases::download(&installer, |done| {
            let _ = progress.send(done);
        })
    });
    (percent, done)
}

/// After the window has closed: starts the installer, which replaces Cledit once it has
/// exited.
pub(crate) fn apply_on_exit(handed_over: &Receiver<ApplyOnExit>) {
    if let Some(apply) = handed_over.try_iter().last() {
        let _ = releases::install(&apply.installer, apply.restart);
    }
}
