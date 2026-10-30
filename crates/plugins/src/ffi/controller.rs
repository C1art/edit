use vst3::ComPtr;
use vst3::Steinberg::kResultOk;
use vst3::Steinberg::IPlugView;
use vst3::Steinberg::Vst::ViewType::kEditor;
use vst3::Steinberg::Vst::{IEditController, IEditControllerTrait, String128};

use super::instance::Instance;
use super::strings::from_utf16;
use crate::{ParamId, ParameterFlags, ParameterInfo, PluginError};

pub(super) fn parameter_ids(controller: &ComPtr<IEditController>) -> Vec<u32> {
    parameter_infos(controller).map(|info| info.id).collect()
}

fn parameter_infos(
    controller: &ComPtr<IEditController>,
) -> impl Iterator<Item = vst3::Steinberg::Vst::ParameterInfo> + '_ {
    // SAFETY: plain query on an initialized controller.
    let count = unsafe { controller.getParameterCount() };
    (0..count).filter_map(move |index| {
        // SAFETY: ParameterInfo is plain data for which all-zero bytes are valid.
        let mut info: vst3::Steinberg::Vst::ParameterInfo = unsafe { std::mem::zeroed() };
        // SAFETY: `info` is a valid, writable ParameterInfo and `index` is in range.
        let result = unsafe { controller.getParameterInfo(index, &mut info) };
        (result == kResultOk).then_some(info)
    })
}

impl Instance {
    pub(crate) fn parameters(&self) -> Vec<ParameterInfo> {
        parameter_infos(&self.controller)
            .map(|info| ParameterInfo {
                id: ParamId(info.id),
                title: from_utf16(&info.title),
                short_title: from_utf16(&info.shortTitle),
                units: from_utf16(&info.units),
                default_normalized: info.defaultNormalizedValue,
                step_count: info.stepCount.max(0) as u32,
                flags: ParameterFlags(info.flags),
            })
            .collect()
    }

    pub(crate) fn parameter(&self, id: ParamId) -> f64 {
        // SAFETY: plain query on an initialized controller.
        unsafe { self.controller.getParamNormalized(id.0) }
    }

    /// Sets the controller's value and queues the same value for the processor's next
    /// `process` call.
    pub(crate) fn set_parameter(
        &mut self,
        id: ParamId,
        normalized: f64,
    ) -> Result<(), PluginError> {
        if !normalized.is_finite() {
            return Err(PluginError::InvalidParameterValue);
        }
        if !self.parameter_ids.contains(&id.0) {
            return Err(PluginError::UnknownParameter(id));
        }
        let normalized = normalized.clamp(0.0, 1.0);
        // SAFETY: plain call on an initialized controller with a known parameter ID.
        let result = unsafe { self.controller.setParamNormalized(id.0, normalized) };
        super::check(result, "IEditController::setParamNormalized")?;
        if self.buffers.queue_parameter(id.0, normalized) {
            Ok(())
        } else {
            Err(PluginError::UnknownParameter(id))
        }
    }

    pub(crate) fn display_value(
        &self,
        id: ParamId,
        normalized: f64,
    ) -> Result<String, PluginError> {
        let mut text: String128 = [0; 128];
        // SAFETY: `text` is a writable String128.
        let result = unsafe {
            self.controller
                .getParamStringByValue(id.0, normalized, &mut text)
        };
        super::check(result, "IEditController::getParamStringByValue")?;
        Ok(from_utf16(&text))
    }

    pub(crate) fn has_editor(&self) -> bool {
        // SAFETY: `kEditor` is a NUL-terminated view type name.
        let view = unsafe { self.controller.createView(kEditor) };
        // SAFETY: a non-null view carries one reference for us; dropping the pointer
        // releases it immediately.
        unsafe { ComPtr::<IPlugView>::from_raw(view) }.is_some()
    }
}
