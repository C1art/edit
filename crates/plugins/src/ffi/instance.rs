use std::path::Path;
use std::ptr;

use vst3::com_scrape_types::Inherits;
use vst3::Steinberg::Vst::{
    BusDirection, IAudioProcessor, IAudioProcessorTrait, IComponent, IComponentTrait,
    IConnectionPoint, IConnectionPointTrait, IEditController, ProcessSetup, SpeakerArr,
    SpeakerArrangement,
};
use vst3::Steinberg::{kResultOk, kResultTrue};
use vst3::Steinberg::{
    FUnknown, IPluginBase, IPluginBaseTrait, IPluginFactory, IPluginFactoryTrait, TUID,
};
use vst3::{ComPtr, ComWrapper, Interface};

use super::consts::{AUDIO, INPUT, OUTPUT, REALTIME, SAMPLE_32};
use super::host::HostApplication;
use super::process::ProcessBuffers;
use super::{check, SharedModule};
use crate::{ClassId, PluginError};

/// A component and its controller, initialized, activated and processing.
///
/// Field order is teardown order: the connection points are disconnected, then the
/// controller and the component are terminated and released, and the module exits last.
pub(crate) struct Instance {
    pub(super) buffers: ProcessBuffers,
    pub(super) parameter_ids: Vec<u32>,
    _connection: Option<Connection>,
    _separate_controller: Option<Initialized<IEditController>>,
    pub(super) controller: ComPtr<IEditController>,
    pub(super) processor: ComPtr<IAudioProcessor>,
    pub(super) component: Initialized<IComponent>,
    _host: ComWrapper<HostApplication>,
    _module: SharedModule,
}

// SAFETY: every raw pointer the instance holds points into heap memory it owns, and the
// Cell-based host objects in `buffers` are only touched by the plugin synchronously inside
// calls made through `&mut self`. VST3 objects may be called from more than one thread
// as long as calls do not overlap, which `Instance: !Sync` guarantees.
unsafe impl Send for Instance {}

impl Instance {
    pub(crate) fn load(
        binary: &Path,
        class_id: ClassId,
        sample_rate: u32,
        max_block: usize,
    ) -> Result<Self, PluginError> {
        if sample_rate == 0 || max_block == 0 || i32::try_from(max_block).is_err() {
            return Err(PluginError::InvalidProcessSetup);
        }
        let module = SharedModule::open(binary)?;
        let host = ComWrapper::new(HostApplication);
        let component = create::<IComponent>(module.factory(), &class_id.0)?;
        let component = Initialized::new(component, &host, "IComponent::initialize")?;
        let processor = component
            .0
            .cast::<IAudioProcessor>()
            .ok_or(PluginError::MissingInterface("IAudioProcessor"))?;
        let (controller, separate_controller) = match component.0.cast::<IEditController>() {
            Some(controller) => (controller, None),
            None => {
                let controller_id = controller_class_id(&component.0)?;
                let controller = create::<IEditController>(module.factory(), &controller_id)?;
                let controller =
                    Initialized::new(controller, &host, "IEditController::initialize")?;
                (controller.0.clone(), Some(controller))
            }
        };
        let connection = separate_controller
            .as_ref()
            .and_then(|_| Connection::new(&component.0, &controller));
        send_component_state(&component.0, &controller);
        let layout = configure_buses(&component.0, &processor)?;
        setup_processing(&processor, sample_rate, max_block)?;
        // SAFETY: the component is initialized and its processing is set up.
        check(unsafe { component.0.setActive(1) }, "IComponent::setActive")?;
        // The result is ignored because many plugins do not implement setProcessing.
        // SAFETY: the component is active.
        unsafe { processor.setProcessing(1) };
        let parameter_ids = super::controller::parameter_ids(&controller);
        let buffers = ProcessBuffers::new(
            f64::from(sample_rate),
            max_block,
            &layout.input_channels,
            &layout.output_channels,
            parameter_ids.len(),
        );
        Ok(Self {
            buffers,
            parameter_ids,
            _connection: connection,
            _separate_controller: separate_controller,
            controller,
            processor,
            component,
            _host: host,
            _module: module,
        })
    }

    pub(crate) fn process(
        &mut self,
        inputs: [&[f32]; 2],
        outputs: [&mut [f32]; 2],
    ) -> Result<(), PluginError> {
        self.buffers.process(&self.processor, inputs, outputs)
    }

    pub(crate) fn reset(&mut self) -> Result<(), PluginError> {
        // SAFETY: the component is active and processing; this cycles it back to the
        // same state, which drops any internal tails.
        unsafe {
            self.processor.setProcessing(0);
            self.component.0.setActive(0);
            check(self.component.0.setActive(1), "IComponent::setActive")?;
            self.processor.setProcessing(1);
        }
        Ok(())
    }

    pub(crate) fn latency_samples(&self) -> u32 {
        // SAFETY: plain query on an active processor.
        unsafe { self.processor.getLatencySamples() }
    }

    pub(crate) fn tail_samples(&self) -> u32 {
        // SAFETY: plain query on an active processor.
        unsafe { self.processor.getTailSamples() }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        // SAFETY: the instance is active and processing; the remaining teardown happens
        // as the fields drop.
        unsafe {
            self.processor.setProcessing(0);
            self.component.0.setActive(0);
        }
    }
}

/// An object between `initialize` and `terminate`.
pub(super) struct Initialized<I: Inherits<IPluginBase>>(pub(super) ComPtr<I>);

impl<I: Inherits<IPluginBase>> Initialized<I> {
    fn new(
        object: ComPtr<I>,
        host: &ComWrapper<HostApplication>,
        method: &'static str,
    ) -> Result<Self, PluginError> {
        let context = host
            .as_com_ref::<FUnknown>()
            .map_or(ptr::null_mut(), |context| context.as_ptr());
        // SAFETY: `context` is a live host object that the plugin may retain.
        check(unsafe { object.initialize(context) }, method)?;
        Ok(Self(object))
    }
}

impl<I: Inherits<IPluginBase>> Drop for Initialized<I> {
    fn drop(&mut self) {
        // SAFETY: balances the successful `initialize`.
        unsafe { self.0.terminate() };
    }
}

/// Connects a separate component and controller both ways; disconnects on drop.
struct Connection {
    component: ComPtr<IConnectionPoint>,
    controller: ComPtr<IConnectionPoint>,
}

impl Connection {
    fn new(component: &ComPtr<IComponent>, controller: &ComPtr<IEditController>) -> Option<Self> {
        let connection = Self {
            component: component.cast()?,
            controller: controller.cast()?,
        };
        // The results are ignored like other hosts do: a plugin that refuses the
        // connection simply runs without component-controller messages.
        // SAFETY: both connection points are live and stay alive until `drop`.
        unsafe {
            connection.component.connect(connection.controller.as_ptr());
            connection.controller.connect(connection.component.as_ptr());
        }
        Some(connection)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // SAFETY: balances the `connect` calls made in `new`.
        unsafe {
            self.component.disconnect(self.controller.as_ptr());
            self.controller.disconnect(self.component.as_ptr());
        }
    }
}

fn create<I: Interface>(
    factory: &ComPtr<IPluginFactory>,
    class_id: &[u8; 16],
) -> Result<ComPtr<I>, PluginError> {
    let mut object = ptr::null_mut();
    // SAFETY: both IDs are 16-byte buffers and `object` is a valid out-pointer.
    let result = unsafe {
        factory.createInstance(
            class_id.as_ptr().cast(),
            I::IID.as_ptr().cast(),
            &mut object,
        )
    };
    if result != kResultOk {
        return Err(PluginError::ClassNotFound);
    }
    // SAFETY: on success `object` is an `I` carrying one reference for us.
    unsafe { ComPtr::from_raw(object.cast::<I>()) }.ok_or(PluginError::ClassNotFound)
}

fn controller_class_id(component: &ComPtr<IComponent>) -> Result<[u8; 16], PluginError> {
    let mut class_id: TUID = [0; 16];
    // SAFETY: `class_id` is a writable 16-byte buffer.
    let result = unsafe { component.getControllerClassId(&mut class_id) };
    if result != kResultOk || class_id == [0; 16] {
        return Err(PluginError::MissingInterface("IEditController"));
    }
    Ok(class_id.map(|byte| byte as u8))
}

/// The spec asks hosts to hand the component's initial state to the controller so both
/// start in sync. Failures are ignored: plugins without state legitimately refuse.
fn send_component_state(component: &ComPtr<IComponent>, controller: &ComPtr<IEditController>) {
    let Ok(state) = super::state::component_state(component) else {
        return;
    };
    let _ = super::state::send_component_state(controller, state);
}

struct BusLayout {
    input_channels: Vec<usize>,
    output_channels: Vec<usize>,
}

fn configure_buses(
    component: &ComPtr<IComponent>,
    processor: &ComPtr<IAudioProcessor>,
) -> Result<BusLayout, PluginError> {
    let mut inputs = requested_arrangements(component, processor, INPUT);
    let mut outputs = requested_arrangements(component, processor, OUTPUT);
    if inputs.is_empty() || outputs.is_empty() {
        return Err(PluginError::UnsupportedBusLayout);
    }
    // The result is ignored: a plugin may refuse the full set yet still accept stereo
    // main buses, which is verified below.
    // SAFETY: both arrays hold exactly as many arrangements as the counts passed.
    unsafe {
        processor.setBusArrangements(
            inputs.as_mut_ptr(),
            inputs.len() as i32,
            outputs.as_mut_ptr(),
            outputs.len() as i32,
        );
    }
    for (direction, count) in [(INPUT, inputs.len()), (OUTPUT, outputs.len())] {
        if bus_arrangement(processor, direction, 0) != Some(SpeakerArr::kStereo) {
            return Err(PluginError::UnsupportedBusLayout);
        }
        for index in 0..count as i32 {
            // SAFETY: `index` is below the bus count reported by the component.
            unsafe { component.activateBus(AUDIO, direction, index, u8::from(index == 0)) };
        }
    }
    let channels = |direction: BusDirection, count: usize| -> Vec<usize> {
        (0..count as i32)
            .map(|index| {
                let arrangement = bus_arrangement(processor, direction, index).unwrap_or(0);
                arrangement.count_ones() as usize
            })
            .collect()
    };
    Ok(BusLayout {
        input_channels: channels(INPUT, inputs.len()),
        output_channels: channels(OUTPUT, outputs.len()),
    })
}

/// Stereo for the main bus; auxiliary buses keep whatever the plugin currently has.
fn requested_arrangements(
    component: &ComPtr<IComponent>,
    processor: &ComPtr<IAudioProcessor>,
    direction: BusDirection,
) -> Vec<SpeakerArrangement> {
    // SAFETY: plain query on an initialized component.
    let count = unsafe { component.getBusCount(AUDIO, direction) };
    (0..count)
        .map(|index| match index {
            0 => SpeakerArr::kStereo,
            _ => bus_arrangement(processor, direction, index).unwrap_or(0),
        })
        .collect()
}

fn bus_arrangement(
    processor: &ComPtr<IAudioProcessor>,
    direction: BusDirection,
    index: i32,
) -> Option<SpeakerArrangement> {
    let mut arrangement: SpeakerArrangement = 0;
    // SAFETY: `arrangement` is a valid out-pointer.
    let result = unsafe { processor.getBusArrangement(direction, index, &mut arrangement) };
    (result == kResultOk).then_some(arrangement)
}

fn setup_processing(
    processor: &ComPtr<IAudioProcessor>,
    sample_rate: u32,
    max_block: usize,
) -> Result<(), PluginError> {
    // SAFETY: plain query on an initialized processor.
    if unsafe { processor.canProcessSampleSize(SAMPLE_32) } != kResultTrue {
        return Err(PluginError::UnsupportedSampleSize);
    }
    let mut setup = ProcessSetup {
        processMode: REALTIME,
        symbolicSampleSize: SAMPLE_32,
        maxSamplesPerBlock: max_block as i32,
        sampleRate: f64::from(sample_rate),
    };
    // SAFETY: `setup` is a valid ProcessSetup for the duration of the call.
    check(
        unsafe { processor.setupProcessing(&mut setup) },
        "IAudioProcessor::setupProcessing",
    )
}
