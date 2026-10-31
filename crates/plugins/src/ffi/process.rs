use std::ptr;

use vst3::Steinberg::Vst::{
    AudioBusBuffers, AudioBusBuffers__type0, IAudioProcessor, IAudioProcessorTrait,
    IParameterChanges, ParamID, ProcessContext, ProcessData,
};
use vst3::{ComPtr, ComWrapper};

use super::check;
use super::consts::{REALTIME, SAMPLE_32, TEMPO_AND_TIME_SIGNATURE_VALID};
use super::params::ParamChanges;
use crate::PluginError;

const MAIN_CHANNELS: usize = 2;

/// Everything `process` hands to the plugin, allocated once so the audio path never
/// allocates. Pointer tables are rebuilt on every block, so no pointer into these buffers
/// outlives a borrow of them.
pub(crate) struct ProcessBuffers {
    max_block: usize,
    input_copy: Vec<f32>,
    aux_input_silence: Vec<f32>,
    aux_output_discard: Vec<f32>,
    input_channels: Vec<*mut f32>,
    output_channels: Vec<*mut f32>,
    input_buses: Vec<AudioBusBuffers>,
    output_buses: Vec<AudioBusBuffers>,
    input_changes: ComWrapper<ParamChanges>,
    output_changes: ComWrapper<ParamChanges>,
    context: Box<ProcessContext>,
}

impl ProcessBuffers {
    /// Bus 0 in each direction is the stereo main bus; the rest are inactive auxiliaries
    /// that still need valid (silent or discarded) buffers.
    pub(crate) fn new(
        sample_rate: f64,
        max_block: usize,
        input_bus_channels: &[usize],
        output_bus_channels: &[usize],
        parameter_count: usize,
    ) -> Self {
        Self {
            max_block,
            input_copy: vec![0.0; MAIN_CHANNELS * max_block],
            aux_input_silence: vec![0.0; max_block],
            aux_output_discard: vec![0.0; max_block],
            input_channels: vec![ptr::null_mut(); input_bus_channels.iter().sum()],
            output_channels: vec![ptr::null_mut(); output_bus_channels.iter().sum()],
            input_buses: input_bus_channels.iter().map(|&n| empty_bus(n)).collect(),
            output_buses: output_bus_channels.iter().map(|&n| empty_bus(n)).collect(),
            input_changes: ParamChanges::with_capacity(parameter_count),
            output_changes: ParamChanges::with_capacity(parameter_count),
            context: Box::new(initial_context(sample_rate)),
        }
    }

    pub(crate) fn queue_parameter(&self, id: ParamID, value: f64) -> bool {
        self.input_changes.set(id, value)
    }

    pub(crate) fn has_pending_parameters(&self) -> bool {
        !self.input_changes.is_empty()
    }

    pub(crate) fn discard_pending_parameters(&self) {
        self.input_changes.clear();
    }

    /// Lengths must already be validated as equal.
    pub(crate) fn process(
        &mut self,
        processor: &ComPtr<IAudioProcessor>,
        inputs: [&[f32]; 2],
        outputs: [&mut [f32]; 2],
    ) -> Result<(), PluginError> {
        let [left_in, right_in] = inputs;
        let [left_out, right_out] = outputs;
        let chunks = left_in
            .chunks(self.max_block)
            .zip(right_in.chunks(self.max_block))
            .zip(left_out.chunks_mut(self.max_block))
            .zip(right_out.chunks_mut(self.max_block));
        for (((left_in, right_in), left_out), right_out) in chunks {
            let (left_copy, right_copy) = self.input_copy.split_at_mut(self.max_block);
            left_copy[..left_in.len()].copy_from_slice(left_in);
            right_copy[..right_in.len()].copy_from_slice(right_in);
            self.bind_buffers([left_out.as_mut_ptr(), right_out.as_mut_ptr()]);
            self.run(processor, left_in.len(), true)?;
        }
        Ok(())
    }

    /// Delivers queued parameter changes without audio, as the VST3 spec allows with
    /// `numSamples == 0` and no buses.
    pub(crate) fn flush_parameters(
        &mut self,
        processor: &ComPtr<IAudioProcessor>,
    ) -> Result<(), PluginError> {
        self.run(processor, 0, false)
    }

    fn bind_buffers(&mut self, main_outputs: [*mut f32; MAIN_CHANNELS]) {
        let (left_copy, right_copy) = self.input_copy.split_at_mut(self.max_block);
        let main_inputs = [left_copy.as_mut_ptr(), right_copy.as_mut_ptr()];
        let silence = self.aux_input_silence.as_mut_ptr();
        let discard = self.aux_output_discard.as_mut_ptr();
        fill_pointer_table(&mut self.input_channels, main_inputs, silence);
        fill_pointer_table(&mut self.output_channels, main_outputs, discard);
        point_buses_at(&mut self.input_buses, &mut self.input_channels);
        point_buses_at(&mut self.output_buses, &mut self.output_channels);
    }

    fn run(
        &mut self,
        processor: &ComPtr<IAudioProcessor>,
        samples: usize,
        with_buses: bool,
    ) -> Result<(), PluginError> {
        self.output_changes.clear();
        let bus_count = |buses: &[AudioBusBuffers]| if with_buses { buses.len() } else { 0 };
        let mut data = ProcessData {
            processMode: REALTIME,
            symbolicSampleSize: SAMPLE_32,
            numSamples: samples as i32,
            numInputs: bus_count(&self.input_buses) as i32,
            numOutputs: bus_count(&self.output_buses) as i32,
            inputs: self.input_buses.as_mut_ptr(),
            outputs: self.output_buses.as_mut_ptr(),
            inputParameterChanges: changes_ptr(&self.input_changes),
            outputParameterChanges: changes_ptr(&self.output_changes),
            inputEvents: ptr::null_mut(),
            outputEvents: ptr::null_mut(),
            processContext: &mut *self.context,
        };
        // SAFETY: every pointer in `data` refers to buffers owned by `self` or by the
        // caller's output slices, sized for `samples` frames and alive for this call.
        let result = unsafe { processor.process(&mut data) };
        self.input_changes.clear();
        self.context.projectTimeSamples += samples as i64;
        self.context.continousTimeSamples += samples as i64;
        check(result, "IAudioProcessor::process")
    }
}

fn fill_pointer_table(
    table: &mut [*mut f32],
    main: [*mut f32; MAIN_CHANNELS],
    auxiliary: *mut f32,
) {
    for (index, slot) in table.iter_mut().enumerate() {
        *slot = main.get(index).copied().unwrap_or(auxiliary);
    }
}

fn point_buses_at(buses: &mut [AudioBusBuffers], channels: &mut [*mut f32]) {
    let mut remaining = channels;
    for bus in buses {
        let (own, rest) = remaining.split_at_mut(bus.numChannels as usize);
        bus.__field0.channelBuffers32 = own.as_mut_ptr();
        bus.silenceFlags = 0;
        remaining = rest;
    }
}

fn empty_bus(channels: usize) -> AudioBusBuffers {
    AudioBusBuffers {
        numChannels: channels as i32,
        silenceFlags: 0,
        __field0: AudioBusBuffers__type0 {
            channelBuffers32: ptr::null_mut(),
        },
    }
}

fn changes_ptr(changes: &ComWrapper<ParamChanges>) -> *mut IParameterChanges {
    changes
        .as_com_ref::<IParameterChanges>()
        .map_or(ptr::null_mut(), |changes| changes.as_ptr())
}

fn initial_context(sample_rate: f64) -> ProcessContext {
    // SAFETY: ProcessContext is plain data for which all-zero bytes are valid.
    let mut context: ProcessContext = unsafe { std::mem::zeroed() };
    context.state = TEMPO_AND_TIME_SIGNATURE_VALID;
    context.sampleRate = sample_rate;
    context.tempo = 120.0;
    context.timeSigNumerator = 4;
    context.timeSigDenominator = 4;
    context
}
