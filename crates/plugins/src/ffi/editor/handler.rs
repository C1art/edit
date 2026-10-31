use std::sync::mpsc::Sender;
use std::sync::Mutex;

use vst3::Class;
use vst3::Steinberg::Vst::RestartFlags_::{kParamValuesChanged, kReloadComponent};
use vst3::Steinberg::Vst::{IComponentHandler, IComponentHandlerTrait, ParamID, ParamValue};
use vst3::Steinberg::{int32, kResultOk, tresult};

use crate::editor::{EditorEvent, Notify};
use crate::ParamId;

/// What the editor did since the editor's thread last looked, for its own instance.
#[derive(Default)]
pub(super) struct Edits {
    pub(super) values: Vec<(ParamId, f64)>,
    /// A gesture ended, or a value changed outside one.
    pub(super) settled: bool,
    pub(super) reloaded: bool,
}

/// Hears the editor's edits. The editor calls it from inside its own window messages,
/// while the instance is borrowed, so edits wait in `edits` for the thread to apply.
pub(super) struct Handler {
    events: Sender<EditorEvent>,
    notify: Notify,
    edits: Mutex<Edits>,
    /// Controls being dragged.
    held: Mutex<Vec<u32>>,
}

impl Handler {
    pub(super) fn new(events: Sender<EditorEvent>, notify: Notify) -> Self {
        Self {
            events,
            notify,
            edits: Mutex::default(),
            held: Mutex::default(),
        }
    }

    pub(super) fn send(&self, event: EditorEvent) {
        if self.events.send(event).is_ok() {
            (self.notify)();
        }
    }

    pub(super) fn take(&self) -> Edits {
        self.edits
            .lock()
            .map(|mut edits| std::mem::take(&mut *edits))
            .unwrap_or_default()
    }

    fn record(&self, record: impl FnOnce(&mut Edits)) {
        if let Ok(mut edits) = self.edits.lock() {
            record(&mut edits);
        }
    }
}

impl Class for Handler {
    type Interfaces = (IComponentHandler,);
}

impl IComponentHandlerTrait for Handler {
    unsafe fn beginEdit(&self, id: ParamID) -> tresult {
        if let Ok(mut held) = self.held.lock() {
            held.push(id);
        }
        self.send(EditorEvent::Begin(ParamId(id)));
        kResultOk
    }

    unsafe fn performEdit(&self, id: ParamID, value: ParamValue) -> tresult {
        let alone = self.held.lock().is_ok_and(|held| !held.contains(&id));
        let id = ParamId(id);
        self.record(|edits| {
            edits.values.push((id, value));
            edits.settled |= alone;
        });
        self.send(EditorEvent::Change(id, value));
        kResultOk
    }

    unsafe fn endEdit(&self, id: ParamID) -> tresult {
        let released = self.held.lock().is_ok_and(|mut held| {
            if let Some(index) = held.iter().position(|&held| held == id) {
                held.swap_remove(index);
            }
            held.is_empty()
        });
        self.record(|edits| edits.settled |= released);
        self.send(EditorEvent::End(ParamId(id)));
        kResultOk
    }

    unsafe fn restartComponent(&self, flags: int32) -> tresult {
        if flags & (kParamValuesChanged | kReloadComponent) != 0 {
            self.record(|edits| edits.reloaded = true);
        }
        kResultOk
    }
}
