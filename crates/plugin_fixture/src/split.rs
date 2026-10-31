use std::ffi::{c_void, CStr};
use std::ptr;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, PoisonError};

use vst3::Steinberg::Vst::{
    IAttributeListTrait, IAudioProcessor, IComponent, IConnectionPoint, IConnectionPointTrait,
    IEditController, IHostApplication, IHostApplicationTrait, IMessage, IMessageTrait,
};
use vst3::Steinberg::{kResultFalse, kResultOk, tresult, FUnknown, IPluginBaseTrait, TUID};
use vst3::{Class, ComPtr, ComRef, Interface};

use crate::controller::ControllerCore;
use crate::delegate::{impl_component, impl_edit_controller};
use crate::processor::ProcessorCore;

const HELLO: &CStr = c"hello";
const MAGIC_KEY: &CStr = c"magic";
const MAGIC: i64 = 42;

/// The processor half of the split class. On `connect` it sends a message built through
/// the host's `IHostApplication::createInstance` to the controller.
pub(crate) struct SplitProcessor {
    processor: ProcessorCore,
    host: Mutex<Option<ComPtr<IHostApplication>>>,
}

impl SplitProcessor {
    pub(crate) fn new(controller_class: TUID) -> Self {
        Self {
            processor: ProcessorCore::new(Some(controller_class)),
            host: Mutex::new(None),
        }
    }

    fn say_hello(&self, peer: ComRef<'_, IConnectionPoint>) -> bool {
        let Some(host) = lock(&self.host).clone() else {
            return false;
        };
        let mut object: *mut c_void = ptr::null_mut();
        let mut message_id: TUID = IMessage::IID.map(|byte| byte as i8);
        // SAFETY: both IDs are 16-byte buffers and `object` is a valid out-pointer.
        let result = unsafe { host.createInstance(&mut message_id, &mut message_id, &mut object) };
        // SAFETY: on success `object` is an IMessage carrying one reference for us.
        let message = unsafe { ComPtr::from_raw(object.cast::<IMessage>()) };
        let Some(message) = message.filter(|_| result == kResultOk) else {
            return false;
        };
        // SAFETY: plain calls on live objects; strings are NUL-terminated.
        unsafe {
            message.setMessageID(HELLO.as_ptr());
            let Some(attributes) = ComRef::from_raw(message.getAttributes()) else {
                return false;
            };
            attributes.setInt(MAGIC_KEY.as_ptr(), MAGIC);
            peer.notify(message.as_ptr()) == kResultOk
        }
    }
}

impl Class for SplitProcessor {
    type Interfaces = (IComponent, IAudioProcessor, IConnectionPoint);
}

impl IPluginBaseTrait for SplitProcessor {
    unsafe fn initialize(&self, context: *mut FUnknown) -> tresult {
        // SAFETY: a non-null context is a live host object.
        let host = unsafe { ComRef::from_raw(context) }.and_then(|c| c.cast::<IHostApplication>());
        *lock(&self.host) = host;
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        *lock(&self.host) = None;
        kResultOk
    }
}

impl IConnectionPointTrait for SplitProcessor {
    unsafe fn connect(&self, other: *mut IConnectionPoint) -> tresult {
        // SAFETY: a non-null peer is a live connection point for the duration of the call.
        let Some(peer) = (unsafe { ComRef::from_raw(other) }) else {
            return kResultFalse;
        };
        let connected = self.say_hello(peer);
        self.processor.connected.store(connected, Ordering::Relaxed);
        kResultOk
    }

    unsafe fn disconnect(&self, _other: *mut IConnectionPoint) -> tresult {
        self.processor.connected.store(false, Ordering::Relaxed);
        kResultOk
    }

    unsafe fn notify(&self, _message: *mut IMessage) -> tresult {
        kResultOk
    }
}

impl_component!(SplitProcessor, processor);

/// The controller half of the split class. It only acknowledges the processor's message.
pub(crate) struct SplitController {
    controller: ControllerCore,
}

impl SplitController {
    pub(crate) fn new() -> Self {
        Self {
            controller: ControllerCore::new(),
        }
    }
}

impl Class for SplitController {
    type Interfaces = (IEditController, IConnectionPoint);
}

impl IPluginBaseTrait for SplitController {
    unsafe fn initialize(&self, _context: *mut FUnknown) -> tresult {
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}

impl IConnectionPointTrait for SplitController {
    unsafe fn connect(&self, _other: *mut IConnectionPoint) -> tresult {
        kResultOk
    }

    unsafe fn disconnect(&self, _other: *mut IConnectionPoint) -> tresult {
        kResultOk
    }

    unsafe fn notify(&self, message: *mut IMessage) -> tresult {
        // SAFETY: a non-null message is live for the duration of the call; its ID and
        // attribute list are owned by it.
        let acknowledged = unsafe {
            ComRef::from_raw(message).is_some_and(|message| {
                let id = message.getMessageID();
                let mut magic = 0;
                !id.is_null()
                    && CStr::from_ptr(id) == HELLO
                    && ComRef::from_raw(message.getAttributes()).is_some_and(|attributes| {
                        attributes.getInt(MAGIC_KEY.as_ptr(), &mut magic) == kResultOk
                    })
                    && magic == MAGIC
            })
        };
        if acknowledged {
            kResultOk
        } else {
            kResultFalse
        }
    }
}

impl_edit_controller!(SplitController, controller);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
