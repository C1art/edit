use std::cell::Cell;
use std::ptr;

use vst3::Steinberg::Vst::{
    IParamValueQueue, IParamValueQueueTrait, IParameterChanges, IParameterChangesTrait, ParamID,
    ParamValue,
};
use vst3::Steinberg::{int32, tresult};
use vst3::Steinberg::{kInvalidArgument, kResultOk};
use vst3::{Class, ComWrapper};

/// Holds at most one point: the host only queues the latest value set between two
/// `process` calls, and output points from the plugin are discarded.
pub(crate) struct ParamQueue {
    id: Cell<ParamID>,
    value: Cell<Option<ParamValue>>,
}

impl Class for ParamQueue {
    type Interfaces = (IParamValueQueue,);
}

impl IParamValueQueueTrait for ParamQueue {
    unsafe fn getParameterId(&self) -> ParamID {
        self.id.get()
    }

    unsafe fn getPointCount(&self) -> int32 {
        int32::from(self.value.get().is_some())
    }

    unsafe fn getPoint(
        &self,
        index: int32,
        sample_offset: *mut int32,
        value: *mut ParamValue,
    ) -> tresult {
        match self.value.get() {
            Some(point) if index == 0 && !sample_offset.is_null() && !value.is_null() => {
                // SAFETY: both out-pointers are non-null and supplied by the plugin.
                unsafe {
                    *sample_offset = 0;
                    *value = point;
                }
                kResultOk
            }
            _ => kInvalidArgument,
        }
    }

    unsafe fn addPoint(
        &self,
        _sample_offset: int32,
        value: ParamValue,
        index: *mut int32,
    ) -> tresult {
        self.value.set(Some(value));
        if !index.is_null() {
            // SAFETY: non-null out-pointer supplied by the plugin.
            unsafe { *index = 0 };
        }
        kResultOk
    }
}

/// A fixed pool of queues allocated at load so `process` never allocates.
pub(crate) struct ParamChanges {
    queues: Vec<ComWrapper<ParamQueue>>,
    used: Cell<usize>,
}

impl Class for ParamChanges {
    type Interfaces = (IParameterChanges,);
}

impl ParamChanges {
    pub(crate) fn with_capacity(capacity: usize) -> ComWrapper<Self> {
        let queues = (0..capacity)
            .map(|_| {
                ComWrapper::new(ParamQueue {
                    id: Cell::new(0),
                    value: Cell::new(None),
                })
            })
            .collect();
        ComWrapper::new(Self {
            queues,
            used: Cell::new(0),
        })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.used.get() == 0
    }

    pub(crate) fn clear(&self) {
        self.used.set(0);
    }

    /// Returns false when the pool is exhausted.
    pub(crate) fn set(&self, id: ParamID, value: ParamValue) -> bool {
        match self.queue_index(id) {
            Some(index) => {
                self.queues[index].value.set(Some(value));
                true
            }
            None => false,
        }
    }

    fn queue_index(&self, id: ParamID) -> Option<usize> {
        let used = self.used.get();
        if let Some(index) = self.queues[..used].iter().position(|q| q.id.get() == id) {
            return Some(index);
        }
        let queue = self.queues.get(used)?;
        queue.id.set(id);
        queue.value.set(None);
        self.used.set(used + 1);
        Some(used)
    }
}

impl IParameterChangesTrait for ParamChanges {
    unsafe fn getParameterCount(&self) -> int32 {
        self.used.get() as int32
    }

    unsafe fn getParameterData(&self, index: int32) -> *mut IParamValueQueue {
        usize::try_from(index)
            .ok()
            .filter(|&index| index < self.used.get())
            .and_then(|index| self.queues[index].as_com_ref::<IParamValueQueue>())
            .map_or(ptr::null_mut(), |queue| queue.as_ptr())
    }

    unsafe fn addParameterData(
        &self,
        id: *const ParamID,
        index: *mut int32,
    ) -> *mut IParamValueQueue {
        if id.is_null() {
            return ptr::null_mut();
        }
        // SAFETY: non-null pointer supplied by the plugin.
        let id = unsafe { *id };
        let Some(queue_index) = self.queue_index(id) else {
            return ptr::null_mut();
        };
        if !index.is_null() {
            // SAFETY: non-null out-pointer supplied by the plugin.
            unsafe { *index = queue_index as int32 };
        }
        self.queues[queue_index]
            .as_com_ref::<IParamValueQueue>()
            .map_or(ptr::null_mut(), |queue| queue.as_ptr())
    }
}
