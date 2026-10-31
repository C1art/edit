use std::ffi::c_void;
use std::ptr;

use vst3::Steinberg::Vst::{
    IAttributeList, IHostApplication, IHostApplicationTrait, IMessage, String128,
};
use vst3::Steinberg::{kInvalidArgument, kResultFalse, kResultOk};
use vst3::Steinberg::{tresult, TUID};
use vst3::{Class, ComWrapper, Interface};

use super::message::{AttributeList, HostMessage};
use super::strings::copy_utf16;

const HOST_NAME: &str = "Cledit";

/// The host context passed to `initialize`. Stateless, so plugins may call it from any thread.
pub(crate) struct HostApplication;

impl Class for HostApplication {
    type Interfaces = (IHostApplication,);
}

impl IHostApplicationTrait for HostApplication {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        if name.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: non-null pointer to a String128 supplied by the plugin.
        copy_utf16(HOST_NAME, unsafe { &mut *name });
        kResultOk
    }

    unsafe fn createInstance(
        &self,
        cid: *mut TUID,
        iid: *mut TUID,
        obj: *mut *mut c_void,
    ) -> tresult {
        if cid.is_null() || iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: non-null pointers to 16-byte IDs supplied by the plugin; TUID and Guid
        // have the same size and layout.
        let (cid, iid) = unsafe { (*cid.cast::<[u8; 16]>(), *iid.cast::<[u8; 16]>()) };
        let instance = if cid == IMessage::IID && iid == IMessage::IID {
            ComWrapper::new(HostMessage::default())
                .to_com_ptr::<IMessage>()
                .map(|message| message.into_raw().cast::<c_void>())
        } else if cid == IAttributeList::IID && iid == IAttributeList::IID {
            ComWrapper::new(AttributeList::default())
                .to_com_ptr::<IAttributeList>()
                .map(|list| list.into_raw().cast::<c_void>())
        } else {
            None
        };
        // SAFETY: non-null out-pointer supplied by the plugin; ownership of the reference
        // returned by `into_raw` passes to the caller.
        unsafe { *obj = instance.unwrap_or(ptr::null_mut()) };
        if instance.is_some() {
            kResultOk
        } else {
            kResultFalse
        }
    }
}
