use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::ptr;

use vst3::Steinberg::IBStream_::IStreamSeekMode;
use vst3::Steinberg::IBStream_::IStreamSeekMode_::{kIBSeekCur, kIBSeekEnd, kIBSeekSet};
use vst3::Steinberg::{int32, int64, tresult, IBStream, IBStreamTrait};
use vst3::Steinberg::{kInvalidArgument, kResultOk};
use vst3::{Class, ComWrapper};

/// Host-side `IBStream` over an in-memory buffer, used for state save and restore.
pub(crate) struct MemoryStream {
    data: RefCell<Vec<u8>>,
    position: Cell<usize>,
}

impl Class for MemoryStream {
    type Interfaces = (IBStream,);
}

impl MemoryStream {
    pub(crate) fn new(data: Vec<u8>) -> ComWrapper<Self> {
        ComWrapper::new(Self {
            data: RefCell::new(data),
            position: Cell::new(0),
        })
    }

    pub(crate) fn rewind(&self) {
        self.position.set(0);
    }

    pub(crate) fn take(&self) -> Vec<u8> {
        self.position.set(0);
        self.data.take()
    }
}

impl IBStreamTrait for MemoryStream {
    unsafe fn read(
        &self,
        buffer: *mut c_void,
        num_bytes: int32,
        num_bytes_read: *mut int32,
    ) -> tresult {
        let Ok(requested) = usize::try_from(num_bytes) else {
            return kInvalidArgument;
        };
        if buffer.is_null() && requested > 0 {
            return kInvalidArgument;
        }
        let data = self.data.borrow();
        let position = self.position.get().min(data.len());
        let count = requested.min(data.len() - position);
        // SAFETY: the caller provides `buffer` writable for `num_bytes` bytes and
        // `count <= num_bytes`; `position + count <= data.len()`.
        unsafe { ptr::copy_nonoverlapping(data.as_ptr().add(position), buffer.cast(), count) };
        self.position.set(position + count);
        if !num_bytes_read.is_null() {
            // SAFETY: non-null out-pointer supplied by the caller; `count` fits as it is
            // bounded by `num_bytes`.
            unsafe { *num_bytes_read = count as int32 };
        }
        kResultOk
    }

    unsafe fn write(
        &self,
        buffer: *mut c_void,
        num_bytes: int32,
        num_bytes_written: *mut int32,
    ) -> tresult {
        let Ok(count) = usize::try_from(num_bytes) else {
            return kInvalidArgument;
        };
        if buffer.is_null() && count > 0 {
            return kInvalidArgument;
        }
        let source: &[u8] = if count == 0 {
            &[]
        } else {
            // SAFETY: the caller provides `buffer` readable for `num_bytes` bytes.
            unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), count) }
        };
        let mut data = self.data.borrow_mut();
        let position = self.position.get();
        let Some(end) = position.checked_add(count) else {
            return kInvalidArgument;
        };
        if data.len() < end {
            data.resize(end, 0);
        }
        data[position..end].copy_from_slice(source);
        self.position.set(end);
        if !num_bytes_written.is_null() {
            // SAFETY: non-null out-pointer supplied by the caller.
            unsafe { *num_bytes_written = num_bytes };
        }
        kResultOk
    }

    unsafe fn seek(&self, pos: int64, mode: int32, result: *mut int64) -> tresult {
        let base = match mode as IStreamSeekMode {
            mode if mode == kIBSeekSet => 0,
            mode if mode == kIBSeekCur => self.position.get() as int64,
            mode if mode == kIBSeekEnd => self.data.borrow().len() as int64,
            _ => return kInvalidArgument,
        };
        let Some(Ok(target)) = base.checked_add(pos).map(usize::try_from) else {
            return kInvalidArgument;
        };
        // Clamping keeps a later write from growing the buffer to an arbitrary size.
        let target = target.min(self.data.borrow().len());
        self.position.set(target);
        if !result.is_null() {
            // SAFETY: non-null out-pointer supplied by the caller.
            unsafe { *result = target as int64 };
        }
        kResultOk
    }

    unsafe fn tell(&self, pos: *mut int64) -> tresult {
        if pos.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: non-null out-pointer supplied by the caller.
        unsafe { *pos = self.position.get() as int64 };
        kResultOk
    }
}
