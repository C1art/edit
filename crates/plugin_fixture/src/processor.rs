use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use vst3::ComRef;
use vst3::Steinberg::Vst::BusDirections_::{kInput, kOutput};
use vst3::Steinberg::Vst::BusInfo_::BusFlags_::kDefaultActive;
use vst3::Steinberg::Vst::BusTypes_::kMain;
use vst3::Steinberg::Vst::MediaTypes_::kAudio;
use vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32;
use vst3::Steinberg::Vst::{
    BusDirection, BusInfo, IParamValueQueueTrait, IParameterChanges, IParameterChangesTrait,
    MediaType, ProcessData, SpeakerArr, SpeakerArrangement,
};
use vst3::Steinberg::{
    int32, kInvalidArgument, kResultFalse, kResultOk, tresult, IBStream, IBStreamTrait, TUID,
};

use crate::strings::copy_utf16;

pub(crate) const GAIN_ID: u32 = 0;
pub(crate) const DEFAULT_GAIN: f64 = 0.5;

pub(crate) fn linear_gain(normalized: f64) -> f64 {
    normalized * 2.0
}

pub(crate) struct AtomicF64(AtomicU64);

impl AtomicF64 {
    pub(crate) fn new(value: f64) -> Self {
        Self(AtomicU64::new(value.to_bits()))
    }

    pub(crate) fn get(&self) -> f64 {
        f64::from_bits(self.0.load(Ordering::Relaxed))
    }

    pub(crate) fn set(&self, value: f64) {
        self.0.store(value.to_bits(), Ordering::Relaxed);
    }
}

/// The processing half shared by the combined and the split classes.
pub(crate) struct ProcessorCore {
    gain: AtomicF64,
    pub(crate) controller_class: Option<TUID>,
    /// The split processor refuses to activate until its controller acknowledged the
    /// connection message, which proves the host connected them.
    pub(crate) requires_connection: bool,
    pub(crate) connected: AtomicBool,
}

impl ProcessorCore {
    pub(crate) fn new(controller_class: Option<TUID>) -> Self {
        Self {
            gain: AtomicF64::new(DEFAULT_GAIN),
            controller_class,
            requires_connection: controller_class.is_some(),
            connected: AtomicBool::new(false),
        }
    }

    pub(crate) fn can_activate(&self) -> bool {
        !self.requires_connection || self.connected.load(Ordering::Relaxed)
    }

    pub(crate) fn bus_count(media_type: MediaType, _direction: BusDirection) -> int32 {
        int32::from(media_type == kAudio as MediaType)
    }

    /// # Safety
    /// `bus` must be null or valid for writes.
    pub(crate) unsafe fn bus_info(
        media_type: MediaType,
        direction: BusDirection,
        index: int32,
        bus: *mut BusInfo,
    ) -> tresult {
        if media_type != kAudio as MediaType || index != 0 || bus.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: non-null and writable per this function's contract.
        let bus = unsafe { &mut *bus };
        bus.mediaType = media_type;
        bus.direction = direction;
        bus.channelCount = 2;
        let name = if direction == kInput as BusDirection {
            "Input"
        } else {
            "Output"
        };
        copy_utf16(name, &mut bus.name);
        bus.busType = kMain as _;
        bus.flags = kDefaultActive as _;
        kResultOk
    }

    /// # Safety
    /// Each array must be valid for reads of its count.
    pub(crate) unsafe fn accepts_arrangements(
        inputs: *mut SpeakerArrangement,
        input_count: int32,
        outputs: *mut SpeakerArrangement,
        output_count: int32,
    ) -> tresult {
        if input_count != 1 || output_count != 1 {
            return kResultFalse;
        }
        // SAFETY: each array holds one element per the counts checked above.
        let stereo = unsafe { *inputs == SpeakerArr::kStereo && *outputs == SpeakerArr::kStereo };
        if stereo {
            kResultOk
        } else {
            kResultFalse
        }
    }

    /// # Safety
    /// `arrangement` must be null or valid for writes.
    pub(crate) unsafe fn bus_arrangement(
        direction: BusDirection,
        index: int32,
        arrangement: *mut SpeakerArrangement,
    ) -> tresult {
        let known = direction == kInput as BusDirection || direction == kOutput as BusDirection;
        if !known || index != 0 || arrangement.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: non-null and writable per this function's contract.
        unsafe { *arrangement = SpeakerArr::kStereo };
        kResultOk
    }

    pub(crate) fn can_process(sample_size: int32) -> tresult {
        if sample_size == kSample32 as int32 {
            kResultOk
        } else {
            kResultFalse
        }
    }

    /// # Safety
    /// `data` must point to a ProcessData whose buffers are valid for `numSamples`.
    pub(crate) unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        // SAFETY: valid for the duration of the call per this function's contract.
        let data = unsafe { &*data };
        // SAFETY: a non-null change list is valid for the duration of the call.
        if let Some(changes) = unsafe { ComRef::from_raw(data.inputParameterChanges) } {
            self.apply(changes);
        }
        if data.numSamples <= 0 || data.numInputs < 1 || data.numOutputs < 1 {
            return kResultOk;
        }
        let gain = linear_gain(self.gain.get()) as f32;
        let frames = data.numSamples as usize;
        // SAFETY: the host provides at least one bus each way with valid channel tables.
        let (input, output) = unsafe { (&*data.inputs, &*data.outputs) };
        if input.numChannels != 2 || output.numChannels != 2 {
            return kResultFalse;
        }
        for channel in 0..2 {
            // SAFETY: both buses have two channels whose buffers hold `frames` samples.
            // Raw pointers keep in-place processing (input == output) sound.
            unsafe {
                let source = *input.__field0.channelBuffers32.add(channel);
                let destination = *output.__field0.channelBuffers32.add(channel);
                for frame in 0..frames {
                    destination
                        .add(frame)
                        .write(source.add(frame).read() * gain);
                }
            }
        }
        kResultOk
    }

    fn apply(&self, changes: ComRef<'_, IParameterChanges>) {
        // SAFETY: plain calls on a change list valid for the duration of `process`.
        unsafe {
            for index in 0..changes.getParameterCount() {
                let Some(queue) = ComRef::from_raw(changes.getParameterData(index)) else {
                    continue;
                };
                let points = queue.getPointCount();
                if queue.getParameterId() != GAIN_ID || points == 0 {
                    continue;
                }
                let (mut offset, mut value) = (0, 0.0);
                if queue.getPoint(points - 1, &mut offset, &mut value) == kResultOk {
                    self.gain.set(value);
                }
            }
        }
    }

    /// # Safety
    /// `stream` must be null or a live IBStream.
    pub(crate) unsafe fn save(&self, stream: *mut IBStream) -> tresult {
        // SAFETY: per this function's contract.
        unsafe { write_gain(stream, self.gain.get()) }
    }

    /// # Safety
    /// `stream` must be null or a live IBStream.
    pub(crate) unsafe fn load(&self, stream: *mut IBStream) -> tresult {
        // SAFETY: per this function's contract.
        match unsafe { read_gain(stream) } {
            Some(gain) => {
                self.gain.set(gain);
                kResultOk
            }
            None => kResultFalse,
        }
    }
}

/// # Safety
/// `stream` must be null or a live IBStream.
pub(crate) unsafe fn write_gain(stream: *mut IBStream, gain: f64) -> tresult {
    // SAFETY: per this function's contract.
    let Some(stream) = (unsafe { ComRef::from_raw(stream) }) else {
        return kInvalidArgument;
    };
    let mut bytes = gain.to_le_bytes();
    let mut written = 0;
    // SAFETY: `bytes` is readable for its full length.
    let result = unsafe {
        stream.write(
            bytes.as_mut_ptr().cast::<c_void>(),
            bytes.len() as int32,
            &mut written,
        )
    };
    if result == kResultOk && written == bytes.len() as int32 {
        kResultOk
    } else {
        kResultFalse
    }
}

/// # Safety
/// `stream` must be null or a live IBStream.
pub(crate) unsafe fn read_gain(stream: *mut IBStream) -> Option<f64> {
    // SAFETY: per this function's contract.
    let stream = unsafe { ComRef::from_raw(stream) }?;
    let mut bytes = [0u8; 8];
    let mut read = 0;
    // SAFETY: `bytes` is writable for its full length.
    let result = unsafe {
        stream.read(
            bytes.as_mut_ptr().cast::<c_void>(),
            bytes.len() as int32,
            &mut read,
        )
    };
    (result == kResultOk && read == bytes.len() as int32).then(|| f64::from_le_bytes(bytes))
}
