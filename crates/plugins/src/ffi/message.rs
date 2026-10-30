use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;
use std::sync::{Mutex, MutexGuard, PoisonError};

use vst3::Steinberg::Vst::IAttributeList_::AttrID;
use vst3::Steinberg::Vst::{IAttributeList, IAttributeListTrait, IMessage, IMessageTrait, TChar};
use vst3::Steinberg::{kInvalidArgument, kResultFalse, kResultOk};
use vst3::Steinberg::{tresult, uint32, FIDString};
use vst3::{Class, ComWrapper};

/// Messages are exchanged between a separate component and controller through
/// `IConnectionPoint`; plugins may use them from any thread, hence the mutexes.
pub(crate) struct HostMessage {
    id: Mutex<Option<CString>>,
    attributes: ComWrapper<AttributeList>,
}

impl Default for HostMessage {
    fn default() -> Self {
        Self {
            id: Mutex::new(None),
            attributes: ComWrapper::new(AttributeList::default()),
        }
    }
}

impl Class for HostMessage {
    type Interfaces = (IMessage,);
}

impl IMessageTrait for HostMessage {
    unsafe fn getMessageID(&self) -> FIDString {
        // The pointer stays valid until the ID is replaced or the message is released,
        // which is the lifetime the VST3 API promises.
        lock(&self.id)
            .as_ref()
            .map_or(ptr::null(), |id| id.as_ptr())
    }

    unsafe fn setMessageID(&self, id: FIDString) {
        // SAFETY: a non-null ID is a NUL-terminated string supplied by the plugin.
        let id = (!id.is_null()).then(|| unsafe { CStr::from_ptr(id) }.to_owned());
        *lock(&self.id) = id;
    }

    unsafe fn getAttributes(&self) -> *mut IAttributeList {
        self.attributes
            .as_com_ref::<IAttributeList>()
            .map_or(ptr::null_mut(), |list| list.as_ptr())
    }
}

enum Attribute {
    Int(i64),
    Float(f64),
    String(Vec<TChar>),
    Binary(Vec<u8>),
}

#[derive(Default)]
pub(crate) struct AttributeList {
    values: Mutex<HashMap<CString, Attribute>>,
}

impl Class for AttributeList {
    type Interfaces = (IAttributeList,);
}

impl AttributeList {
    fn set(&self, id: AttrID, value: Attribute) -> tresult {
        let Some(key) = attribute_key(id) else {
            return kInvalidArgument;
        };
        lock(&self.values).insert(key.to_owned(), value);
        kResultOk
    }

    fn with<R>(&self, id: AttrID, read: impl FnOnce(&Attribute) -> Option<R>) -> Option<R> {
        let key = attribute_key(id)?;
        lock(&self.values).get(key).and_then(read)
    }
}

impl IAttributeListTrait for AttributeList {
    unsafe fn setInt(&self, id: AttrID, value: i64) -> tresult {
        self.set(id, Attribute::Int(value))
    }

    unsafe fn getInt(&self, id: AttrID, value: *mut i64) -> tresult {
        let found = self.with(id, |attribute| match attribute {
            Attribute::Int(int) => Some(*int),
            _ => None,
        });
        // SAFETY: `value` is null or a writable out-pointer supplied by the plugin.
        unsafe { write_out(found, value) }
    }

    unsafe fn setFloat(&self, id: AttrID, value: f64) -> tresult {
        self.set(id, Attribute::Float(value))
    }

    unsafe fn getFloat(&self, id: AttrID, value: *mut f64) -> tresult {
        let found = self.with(id, |attribute| match attribute {
            Attribute::Float(float) => Some(*float),
            _ => None,
        });
        // SAFETY: `value` is null or a writable out-pointer supplied by the plugin.
        unsafe { write_out(found, value) }
    }

    unsafe fn setString(&self, id: AttrID, string: *const TChar) -> tresult {
        if string.is_null() {
            return kInvalidArgument;
        }
        let mut len = 0;
        // SAFETY: the plugin passes a NUL-terminated UTF-16 string.
        while unsafe { *string.add(len) } != 0 {
            len += 1;
        }
        // SAFETY: `len` units before the terminator were just read successfully.
        let units = unsafe { std::slice::from_raw_parts(string, len) }.to_vec();
        self.set(id, Attribute::String(units))
    }

    unsafe fn getString(&self, id: AttrID, string: *mut TChar, size_in_bytes: uint32) -> tresult {
        let capacity = size_in_bytes as usize / size_of::<TChar>();
        if string.is_null() || capacity == 0 {
            return kInvalidArgument;
        }
        let copied = self.with(id, |attribute| match attribute {
            Attribute::String(units) => {
                let count = units.len().min(capacity - 1);
                // SAFETY: the plugin provides `size_in_bytes` writable bytes, which hold
                // `capacity` units; `count + 1 <= capacity`.
                unsafe {
                    ptr::copy_nonoverlapping(units.as_ptr(), string, count);
                    *string.add(count) = 0;
                }
                Some(())
            }
            _ => None,
        });
        if copied.is_some() {
            kResultOk
        } else {
            kResultFalse
        }
    }

    unsafe fn setBinary(&self, id: AttrID, data: *const c_void, size_in_bytes: uint32) -> tresult {
        let bytes = if size_in_bytes == 0 {
            Vec::new()
        } else if data.is_null() {
            return kInvalidArgument;
        } else {
            // SAFETY: the plugin provides `data` readable for `size_in_bytes` bytes.
            unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size_in_bytes as usize) }
                .to_vec()
        };
        self.set(id, Attribute::Binary(bytes))
    }

    unsafe fn getBinary(
        &self,
        id: AttrID,
        data: *mut *const c_void,
        size_in_bytes: *mut uint32,
    ) -> tresult {
        if data.is_null() || size_in_bytes.is_null() {
            return kInvalidArgument;
        }
        // The heap buffer outlives the lock; the API promises the pointer stays valid
        // until the attribute is replaced or the list is released.
        let found = self.with(id, |attribute| match attribute {
            Attribute::Binary(bytes) => Some((bytes.as_ptr().cast::<c_void>(), bytes.len())),
            _ => None,
        });
        let Some((bytes, len)) = found else {
            return kResultFalse;
        };
        // SAFETY: both out-pointers are non-null and supplied by the plugin.
        unsafe {
            *data = bytes;
            *size_in_bytes = len as uint32;
        }
        kResultOk
    }
}

fn attribute_key<'a>(id: AttrID) -> Option<&'a CStr> {
    // SAFETY: a non-null attribute ID is a NUL-terminated string supplied by the plugin
    // that lives for the duration of the call.
    (!id.is_null()).then(|| unsafe { CStr::from_ptr(id.cast::<c_char>()) })
}

/// # Safety
/// `out` must be null or valid for a write of `T`.
unsafe fn write_out<T>(found: Option<T>, out: *mut T) -> tresult {
    match found {
        Some(value) if !out.is_null() => {
            // SAFETY: non-null and writable per this function's contract.
            unsafe { *out = value };
            kResultOk
        }
        Some(_) => kInvalidArgument,
        None => kResultFalse,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
