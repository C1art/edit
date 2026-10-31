use std::thread;

use crossbeam::channel::{bounded, Receiver};
use velopack::sources::GithubSource;
use velopack::{UpdateCheck, UpdateInfo, UpdateManager, VelopackApp};

use super::{ApplyOnExit, Checked, ProgressReceiver};

const RELEASES: &str = "https://github.com/C1art/edit";

fn manager() -> Result<UpdateManager, velopack::Error> {
    UpdateManager::new(GithubSource::new(RELEASES, None, false), None, None)
}

/// Runs first in `main`, because the installer and updater start Cledit to set it up
/// and expect it to exit here. Returns the version just updated to, if restarted by an
/// update.
pub(crate) fn start_up() -> Option<String> {
    let mut updated_to = None;
    let app = VelopackApp::build().on_restarted(|version| updated_to = Some(version.to_string()));
    #[cfg(windows)]
    let app = app
        .on_after_install_fast_callback(|_| super::file_type::register())
        .on_after_update_fast_callback(|_| super::file_type::register())
        .on_before_uninstall_fast_callback(|_| super::file_type::unregister());
    let mut app = app;
    app.run();
    drop(app);
    updated_to
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
        let manager = match manager() {
            Err(velopack::Error::NotInstalled(_)) => return Ok(Checked::NotInstalled),
            found => found.map_err(|error| error.to_string())?,
        };
        Ok(
            match manager
                .check_for_updates()
                .map_err(|error| error.to_string())?
            {
                UpdateCheck::UpdateAvailable(info) => Checked::Available(info),
                UpdateCheck::NoUpdateAvailable | UpdateCheck::RemoteIsEmpty => {
                    Checked::UpToDate(manager.get_current_version_as_string())
                }
            },
        )
    })
}

pub(super) fn download(info: &UpdateInfo) -> (ProgressReceiver<i16>, Receiver<Result<(), String>>) {
    let (progress, percent) = std::sync::mpsc::channel();
    let info = info.clone();
    let done = on_worker("update-download", move || {
        manager()
            .and_then(|manager| manager.download_updates(&info, Some(progress)))
            .map_err(|error| error.to_string())
    });
    (percent, done)
}

/// After the window has closed: starts the updater, which waits for Cledit to exit
/// before replacing it.
pub(crate) fn apply_on_exit(handed_over: &Receiver<ApplyOnExit>) {
    let Some(apply) = handed_over.try_iter().last() else {
        return;
    };
    if let Ok(manager) = manager() {
        let _ = manager.wait_exit_then_apply_updates(
            &apply.asset,
            false,
            apply.restart,
            Vec::<String>::new(),
        );
    }
}
