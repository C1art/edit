use vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kCanAutomate;
use vst3::Steinberg::Vst::{ParamID, ParameterInfo, String128, TChar};
use vst3::Steinberg::{int32, kInvalidArgument, kResultFalse, kResultOk, tresult, IBStream};

use crate::processor::{linear_gain, read_gain, AtomicF64, DEFAULT_GAIN, GAIN_ID};
use crate::strings::{copy_utf16, read_utf16};

/// The edit-controller half shared by the combined and the split classes.
pub(crate) struct ControllerCore {
    gain: AtomicF64,
}

impl ControllerCore {
    pub(crate) fn new() -> Self {
        Self {
            gain: AtomicF64::new(DEFAULT_GAIN),
        }
    }

    /// # Safety
    /// `state` must be null or a live IBStream.
    pub(crate) unsafe fn set_component_state(&self, state: *mut IBStream) -> tresult {
        // SAFETY: per this function's contract.
        match unsafe { read_gain(state) } {
            Some(gain) => {
                self.gain.set(gain);
                kResultOk
            }
            None => kResultFalse,
        }
    }

    pub(crate) fn parameter_count() -> int32 {
        1
    }

    /// # Safety
    /// `info` must be null or valid for writes.
    pub(crate) unsafe fn parameter_info(index: int32, info: *mut ParameterInfo) -> tresult {
        if index != 0 || info.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: non-null and writable per this function's contract.
        let info = unsafe { &mut *info };
        info.id = GAIN_ID;
        copy_utf16("Gain", &mut info.title);
        copy_utf16("Gain", &mut info.shortTitle);
        copy_utf16("", &mut info.units);
        info.stepCount = 0;
        info.defaultNormalizedValue = DEFAULT_GAIN;
        info.unitId = 0;
        info.flags = kCanAutomate;
        kResultOk
    }

    /// # Safety
    /// `text` must be null or valid for writes.
    pub(crate) unsafe fn display(id: ParamID, normalized: f64, text: *mut String128) -> tresult {
        if id != GAIN_ID || text.is_null() {
            return kInvalidArgument;
        }
        let display = format!("{:.2}x", linear_gain(normalized));
        // SAFETY: non-null and writable per this function's contract.
        copy_utf16(&display, unsafe { &mut *text });
        kResultOk
    }

    /// # Safety
    /// `text` must be null or NUL-terminated; `normalized` null or valid for writes.
    pub(crate) unsafe fn parse(id: ParamID, text: *mut TChar, normalized: *mut f64) -> tresult {
        if id != GAIN_ID || text.is_null() || normalized.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: per this function's contract.
        let text = unsafe { read_utf16(text) };
        match text.trim().trim_end_matches('x').parse::<f64>() {
            Ok(linear) => {
                // SAFETY: non-null and writable per this function's contract.
                unsafe { *normalized = (linear / 2.0).clamp(0.0, 1.0) };
                kResultOk
            }
            Err(_) => kResultFalse,
        }
    }

    pub(crate) fn to_plain(id: ParamID, normalized: f64) -> f64 {
        if id == GAIN_ID {
            linear_gain(normalized)
        } else {
            0.0
        }
    }

    pub(crate) fn to_normalized(id: ParamID, plain: f64) -> f64 {
        if id == GAIN_ID {
            plain / 2.0
        } else {
            0.0
        }
    }

    pub(crate) fn normalized(&self, id: ParamID) -> f64 {
        if id == GAIN_ID {
            self.gain.get()
        } else {
            0.0
        }
    }

    pub(crate) fn set_normalized(&self, id: ParamID, value: f64) -> tresult {
        if id != GAIN_ID {
            return kInvalidArgument;
        }
        self.gain.set(value);
        kResultOk
    }
}
