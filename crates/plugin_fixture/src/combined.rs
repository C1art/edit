use vst3::Class;
use vst3::Steinberg::Vst::{IAudioProcessor, IComponent, IEditController};
use vst3::Steinberg::{kResultOk, tresult, FUnknown, IPluginBaseTrait};

use crate::controller::ControllerCore;
use crate::delegate::{impl_component, impl_edit_controller};
use crate::processor::ProcessorCore;

/// Component and controller in one object. The two halves keep separate gain values so
/// tests can tell a value delivered through `IParameterChanges` from one set on the
/// controller.
pub(crate) struct CombinedGain {
    processor: ProcessorCore,
    controller: ControllerCore,
}

impl CombinedGain {
    pub(crate) fn new() -> Self {
        Self {
            processor: ProcessorCore::new(None),
            controller: ControllerCore::new(),
        }
    }
}

impl Class for CombinedGain {
    type Interfaces = (IComponent, IAudioProcessor, IEditController);
}

impl IPluginBaseTrait for CombinedGain {
    unsafe fn initialize(&self, _context: *mut FUnknown) -> tresult {
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}

impl_component!(CombinedGain, processor);
impl_edit_controller!(CombinedGain, controller);
