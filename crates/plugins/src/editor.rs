//! A plugin's own editor, in a window of its own.
//!
//! Each window runs on its own thread with its own instance of the plugin: VST3 editors
//! must be called from the thread that owns their window, and that thread must pump the
//! window's messages, which the UI thread and the audio worker cannot do. The window's
//! edits come back as [`EditorEvent`]s for the host to apply to the instance it plays.

use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use crate::{ClassId, ParamId, PluginError};

/// Called on the editor's thread after each event, e.g. to wake the interface.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

pub struct EditorSetup {
    pub path: PathBuf,
    pub class_id: ClassId,
    pub title: String,
    /// A blob from [`Plugin::state`](crate::Plugin::state); empty for the plugin's defaults.
    pub state: Vec<u8>,
    /// Normalized values set after the state.
    pub params: Vec<(ParamId, f64)>,
    pub notify: Notify,
}

#[derive(Debug)]
pub enum EditorEvent {
    /// A control was grabbed: changes until [`End`](Self::End) are one gesture.
    Begin(ParamId),
    Change(ParamId, f64),
    End(ParamId),
    /// The plugin's whole state once a gesture, or a change made outside one, is done.
    State(Vec<u8>),
    /// The plugin changed many values at once, as loading one of its own presets does.
    Reloaded {
        state: Vec<u8>,
        values: Vec<(ParamId, f64)>,
    },
    /// The window closed, with the plugin's state then unless saving it failed.
    Closed(Option<Vec<u8>>),
    /// The plugin could not be loaded or its editor shown; no window is open.
    Failed(PluginError),
}

// Only the Windows editor thread reads commands.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) enum Command {
    SetParameter(ParamId, f64),
    Close,
}

/// An open editor window. Dropping it closes the window without waiting for it.
pub struct EditorWindow {
    commands: Sender<Command>,
    events: Receiver<EditorEvent>,
    /// The window's handle once it exists, to wake its thread.
    window: Arc<AtomicIsize>,
}

impl EditorWindow {
    /// Starts loading the plugin and opening its editor on a new thread; failures from
    /// then on arrive as [`EditorEvent::Failed`]. Only Windows can show editors.
    pub fn open(setup: EditorSetup) -> Result<Self, PluginError> {
        let (commands, received) = mpsc::channel();
        let (sent, events) = mpsc::channel();
        let window = Arc::new(AtomicIsize::new(0));
        spawn(setup, received, sent, Arc::clone(&window))?;
        Ok(Self {
            commands,
            events,
            window,
        })
    }

    /// Moves the editor's control for `id`, as when the host changes the value elsewhere.
    pub fn set_parameter(&self, id: ParamId, normalized: f64) {
        if self
            .commands
            .send(Command::SetParameter(id, normalized))
            .is_ok()
        {
            self.wake();
        }
    }

    /// What has happened since the last call, without waiting.
    pub fn events(&self) -> impl Iterator<Item = EditorEvent> + '_ {
        self.events.try_iter()
    }

    fn wake(&self) {
        let window = self.window.load(Ordering::Acquire);
        #[cfg(windows)]
        crate::ffi::editor::wake(window);
        #[cfg(not(windows))]
        let _ = window;
    }
}

impl Drop for EditorWindow {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Close);
        self.wake();
    }
}

#[cfg(windows)]
fn spawn(
    setup: EditorSetup,
    commands: Receiver<Command>,
    events: Sender<EditorEvent>,
    window: Arc<AtomicIsize>,
) -> Result<(), PluginError> {
    crate::ffi::editor::spawn(setup, commands, events, window)
}

#[cfg(not(windows))]
fn spawn(
    _: EditorSetup,
    _: Receiver<Command>,
    _: Sender<EditorEvent>,
    _: Arc<AtomicIsize>,
) -> Result<(), PluginError> {
    Err(PluginError::EditorUnsupported)
}
