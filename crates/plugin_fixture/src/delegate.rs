//! Interface implementations shared by the combined and the split classes. Macros, because
//! the VST3 interface traits are foreign and cannot be implemented generically.

macro_rules! impl_component {
    ($type:ty, $core:ident) => {
        impl vst3::Steinberg::Vst::IComponentTrait for $type {
            unsafe fn getControllerClassId(
                &self,
                class_id: *mut vst3::Steinberg::TUID,
            ) -> vst3::Steinberg::tresult {
                match self.$core.controller_class {
                    Some(id) if !class_id.is_null() => {
                        // SAFETY: non-null out-pointer supplied by the host.
                        unsafe { *class_id = id };
                        vst3::Steinberg::kResultOk
                    }
                    _ => vst3::Steinberg::kNotImplemented,
                }
            }

            unsafe fn setIoMode(
                &self,
                _mode: vst3::Steinberg::Vst::IoMode,
            ) -> vst3::Steinberg::tresult {
                vst3::Steinberg::kResultOk
            }

            unsafe fn getBusCount(
                &self,
                media_type: vst3::Steinberg::Vst::MediaType,
                direction: vst3::Steinberg::Vst::BusDirection,
            ) -> i32 {
                crate::processor::ProcessorCore::bus_count(media_type, direction)
            }

            unsafe fn getBusInfo(
                &self,
                media_type: vst3::Steinberg::Vst::MediaType,
                direction: vst3::Steinberg::Vst::BusDirection,
                index: i32,
                bus: *mut vst3::Steinberg::Vst::BusInfo,
            ) -> vst3::Steinberg::tresult {
                // SAFETY: forwarded from the host's call.
                unsafe {
                    crate::processor::ProcessorCore::bus_info(media_type, direction, index, bus)
                }
            }

            unsafe fn getRoutingInfo(
                &self,
                _input: *mut vst3::Steinberg::Vst::RoutingInfo,
                _output: *mut vst3::Steinberg::Vst::RoutingInfo,
            ) -> vst3::Steinberg::tresult {
                vst3::Steinberg::kNotImplemented
            }

            unsafe fn activateBus(
                &self,
                _media_type: vst3::Steinberg::Vst::MediaType,
                _direction: vst3::Steinberg::Vst::BusDirection,
                _index: i32,
                _state: vst3::Steinberg::TBool,
            ) -> vst3::Steinberg::tresult {
                vst3::Steinberg::kResultOk
            }

            unsafe fn setActive(&self, state: vst3::Steinberg::TBool) -> vst3::Steinberg::tresult {
                if state == 0 || self.$core.can_activate() {
                    vst3::Steinberg::kResultOk
                } else {
                    vst3::Steinberg::kResultFalse
                }
            }

            unsafe fn setState(
                &self,
                state: *mut vst3::Steinberg::IBStream,
            ) -> vst3::Steinberg::tresult {
                // SAFETY: forwarded from the host's call.
                unsafe { self.$core.load(state) }
            }

            unsafe fn getState(
                &self,
                state: *mut vst3::Steinberg::IBStream,
            ) -> vst3::Steinberg::tresult {
                // SAFETY: forwarded from the host's call.
                unsafe { self.$core.save(state) }
            }
        }

        impl vst3::Steinberg::Vst::IAudioProcessorTrait for $type {
            unsafe fn setBusArrangements(
                &self,
                inputs: *mut vst3::Steinberg::Vst::SpeakerArrangement,
                input_count: i32,
                outputs: *mut vst3::Steinberg::Vst::SpeakerArrangement,
                output_count: i32,
            ) -> vst3::Steinberg::tresult {
                // SAFETY: forwarded from the host's call.
                unsafe {
                    crate::processor::ProcessorCore::accepts_arrangements(
                        inputs,
                        input_count,
                        outputs,
                        output_count,
                    )
                }
            }

            unsafe fn getBusArrangement(
                &self,
                direction: vst3::Steinberg::Vst::BusDirection,
                index: i32,
                arrangement: *mut vst3::Steinberg::Vst::SpeakerArrangement,
            ) -> vst3::Steinberg::tresult {
                // SAFETY: forwarded from the host's call.
                unsafe {
                    crate::processor::ProcessorCore::bus_arrangement(direction, index, arrangement)
                }
            }

            unsafe fn canProcessSampleSize(&self, sample_size: i32) -> vst3::Steinberg::tresult {
                crate::processor::ProcessorCore::can_process(sample_size)
            }

            unsafe fn getLatencySamples(&self) -> u32 {
                0
            }

            unsafe fn setupProcessing(
                &self,
                _setup: *mut vst3::Steinberg::Vst::ProcessSetup,
            ) -> vst3::Steinberg::tresult {
                vst3::Steinberg::kResultOk
            }

            unsafe fn setProcessing(
                &self,
                _state: vst3::Steinberg::TBool,
            ) -> vst3::Steinberg::tresult {
                vst3::Steinberg::kResultOk
            }

            unsafe fn process(
                &self,
                data: *mut vst3::Steinberg::Vst::ProcessData,
            ) -> vst3::Steinberg::tresult {
                // SAFETY: forwarded from the host's call.
                unsafe { self.$core.process(data) }
            }

            unsafe fn getTailSamples(&self) -> u32 {
                0
            }
        }
    };
}

macro_rules! impl_edit_controller {
    ($type:ty, $core:ident) => {
        impl vst3::Steinberg::Vst::IEditControllerTrait for $type {
            unsafe fn setComponentState(
                &self,
                state: *mut vst3::Steinberg::IBStream,
            ) -> vst3::Steinberg::tresult {
                // SAFETY: forwarded from the host's call.
                unsafe { self.$core.set_component_state(state) }
            }

            unsafe fn setState(
                &self,
                _state: *mut vst3::Steinberg::IBStream,
            ) -> vst3::Steinberg::tresult {
                vst3::Steinberg::kResultOk
            }

            unsafe fn getState(
                &self,
                _state: *mut vst3::Steinberg::IBStream,
            ) -> vst3::Steinberg::tresult {
                vst3::Steinberg::kResultOk
            }

            unsafe fn getParameterCount(&self) -> i32 {
                crate::controller::ControllerCore::parameter_count()
            }

            unsafe fn getParameterInfo(
                &self,
                index: i32,
                info: *mut vst3::Steinberg::Vst::ParameterInfo,
            ) -> vst3::Steinberg::tresult {
                // SAFETY: forwarded from the host's call.
                unsafe { crate::controller::ControllerCore::parameter_info(index, info) }
            }

            unsafe fn getParamStringByValue(
                &self,
                id: u32,
                normalized: f64,
                text: *mut vst3::Steinberg::Vst::String128,
            ) -> vst3::Steinberg::tresult {
                // SAFETY: forwarded from the host's call.
                unsafe { crate::controller::ControllerCore::display(id, normalized, text) }
            }

            unsafe fn getParamValueByString(
                &self,
                id: u32,
                text: *mut vst3::Steinberg::Vst::TChar,
                normalized: *mut f64,
            ) -> vst3::Steinberg::tresult {
                // SAFETY: forwarded from the host's call.
                unsafe { crate::controller::ControllerCore::parse(id, text, normalized) }
            }

            unsafe fn normalizedParamToPlain(&self, id: u32, normalized: f64) -> f64 {
                crate::controller::ControllerCore::to_plain(id, normalized)
            }

            unsafe fn plainParamToNormalized(&self, id: u32, plain: f64) -> f64 {
                crate::controller::ControllerCore::to_normalized(id, plain)
            }

            unsafe fn getParamNormalized(&self, id: u32) -> f64 {
                self.$core.normalized(id)
            }

            unsafe fn setParamNormalized(&self, id: u32, value: f64) -> vst3::Steinberg::tresult {
                self.$core.set_normalized(id, value)
            }

            unsafe fn setComponentHandler(
                &self,
                _handler: *mut vst3::Steinberg::Vst::IComponentHandler,
            ) -> vst3::Steinberg::tresult {
                vst3::Steinberg::kResultOk
            }

            unsafe fn createView(
                &self,
                _name: vst3::Steinberg::FIDString,
            ) -> *mut vst3::Steinberg::IPlugView {
                std::ptr::null_mut()
            }
        }
    };
}

pub(crate) use {impl_component, impl_edit_controller};
