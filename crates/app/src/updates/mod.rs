//! New versions of Cledit, published as GitHub releases: checked for at start, offered,
//! downloaded in the background and put in place once the window has closed.

mod dialog;
mod file_type;
mod worker;

use std::sync::mpsc::Receiver as ProgressReceiver;

use crossbeam::channel::{Receiver, Sender};
use velopack::{UpdateInfo, VelopackAsset};

pub(crate) use dialog::{help_menu_item, ui};
pub(crate) use worker::{apply_on_exit, start_up};

/// What the window's closing hands to `apply_on_exit`.
pub struct ApplyOnExit {
    asset: VelopackAsset,
    restart: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asked {
    AtStart,
    FromMenu,
}

#[derive(Debug)]
enum Checked {
    UpToDate(String),
    Available(Box<UpdateInfo>),
    /// Run from a build folder rather than installed, so there is nothing to update.
    NotInstalled,
}

#[derive(Default)]
enum State {
    #[default]
    Idle,
    Checking {
        asked: Asked,
        reply: Receiver<Result<Checked, String>>,
    },
    Offered(Box<UpdateInfo>),
    Downloading {
        info: Box<UpdateInfo>,
        percent: i16,
        progress: ProgressReceiver<i16>,
        done: Receiver<Result<(), String>>,
    },
    /// Downloaded; `asking` while the restart question is up.
    Ready {
        info: Box<UpdateInfo>,
        asking: bool,
    },
}

pub(crate) struct Updates {
    /// Whether to look for a new version at start; a preference.
    pub(crate) automatic: bool,
    /// A version the user said to skip, which isn't offered at start again.
    pub(crate) skipped: Option<String>,
    state: State,
    checked_at_start: bool,
    /// Set from Restart Now until the close goes through or is called off.
    restart: bool,
    on_exit: Option<Sender<ApplyOnExit>>,
}

impl Default for Updates {
    fn default() -> Self {
        Self {
            automatic: true,
            skipped: None,
            state: State::Idle,
            checked_at_start: false,
            restart: false,
            on_exit: None,
        }
    }
}

impl Updates {
    pub(crate) fn hand_over_on_exit(&mut self, on_exit: Sender<ApplyOnExit>) {
        self.on_exit = Some(on_exit);
    }

    /// Once the preferences are known, so a skipped version or a switched-off check counts.
    pub(crate) fn check_at_start(&mut self) {
        if std::mem::replace(&mut self.checked_at_start, true) || !self.automatic {
            return;
        }
        self.check(Asked::AtStart);
    }

    fn check(&mut self, asked: Asked) {
        if matches!(self.state, State::Idle) {
            self.state = State::Checking {
                asked,
                reply: worker::check(),
            };
        }
    }

    /// The close asked for by Restart Now was called off.
    pub(crate) fn cancel_restart(&mut self) {
        self.restart = false;
    }

    /// The window is closing for good: a downloaded update goes in once it has.
    pub(crate) fn window_closing(&mut self) {
        let (State::Ready { info, .. }, Some(on_exit)) = (&self.state, &self.on_exit) else {
            return;
        };
        let _ = on_exit.send(ApplyOnExit {
            asset: info.TargetFullRelease.clone(),
            restart: self.restart,
        });
    }
}

/// Whether a check's result is worth interrupting the user with.
fn offer(asked: Asked, version: &str, skipped: Option<&str>) -> bool {
    asked == Asked::FromMenu || skipped != Some(version)
}
