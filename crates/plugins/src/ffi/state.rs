use vst3::Steinberg::kNotImplemented;
use vst3::Steinberg::Vst::{IComponent, IComponentTrait, IEditController, IEditControllerTrait};
use vst3::Steinberg::{tresult, IBStream};
use vst3::{ComPtr, ComWrapper};

use super::check;
use super::instance::Instance;
use super::stream::MemoryStream;
use crate::PluginError;

pub(super) fn component_state(component: &ComPtr<IComponent>) -> Result<Vec<u8>, PluginError> {
    let stream = MemoryStream::new(Vec::new());
    // SAFETY: `stream` is a live IBStream for the duration of the call.
    let result = unsafe { component.getState(stream_ptr(&stream)) };
    optional(result, "IComponent::getState")?;
    Ok(stream.take())
}

pub(super) fn send_component_state(
    controller: &ComPtr<IEditController>,
    state: Vec<u8>,
) -> Result<(), PluginError> {
    let stream = MemoryStream::new(state);
    // SAFETY: `stream` is a live IBStream for the duration of the call.
    let result = unsafe { controller.setComponentState(stream_ptr(&stream)) };
    optional(result, "IEditController::setComponentState")
}

impl Instance {
    /// Returns the component state and the controller state. Queued parameter changes
    /// are flushed to the processor first so the component state includes them.
    pub(crate) fn save_state(&mut self) -> Result<(Vec<u8>, Vec<u8>), PluginError> {
        if self.buffers.has_pending_parameters() {
            self.buffers.flush_parameters(&self.processor)?;
        }
        let component = component_state(&self.component.0)?;
        let stream = MemoryStream::new(Vec::new());
        // SAFETY: `stream` is a live IBStream for the duration of the call.
        let result = unsafe { self.controller.getState(stream_ptr(&stream)) };
        optional(result, "IEditController::getState")?;
        Ok((component, stream.take()))
    }

    pub(crate) fn restore_state(
        &mut self,
        component: &[u8],
        controller: &[u8],
    ) -> Result<(), PluginError> {
        // Changes queued before would reach the processor after the state and undo it.
        self.buffers.discard_pending_parameters();
        // A plugin without component state saved none, and may refuse an empty one.
        if !component.is_empty() {
            let stream = MemoryStream::new(component.to_vec());
            // SAFETY: `stream` is a live IBStream for the duration of the call.
            let result = unsafe { self.component.0.setState(stream_ptr(&stream)) };
            optional(result, "IComponent::setState")?;
            stream.rewind();
            // SAFETY: as above.
            let result = unsafe { self.controller.setComponentState(stream_ptr(&stream)) };
            optional(result, "IEditController::setComponentState")?;
        }
        if controller.is_empty() {
            return Ok(());
        }
        let stream = MemoryStream::new(controller.to_vec());
        // SAFETY: as above.
        let result = unsafe { self.controller.setState(stream_ptr(&stream)) };
        optional(result, "IEditController::setState")
    }
}

fn stream_ptr(stream: &ComWrapper<MemoryStream>) -> *mut IBStream {
    stream
        .as_com_ref::<IBStream>()
        .map_or(std::ptr::null_mut(), |stream| stream.as_ptr())
}

/// Plugins without a particular kind of state report it as not implemented.
fn optional(result: tresult, method: &'static str) -> Result<(), PluginError> {
    if result == kNotImplemented {
        return Ok(());
    }
    check(result, method)
}
