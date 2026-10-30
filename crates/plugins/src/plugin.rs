use std::path::Path;

use crate::ffi::Instance;
use crate::scan::bundle_binary;
use crate::{state, ClassId, ParamId, ParameterInfo, PluginError, PluginInfo, Tail};

/// A loaded, active VST3 audio effect with a stereo main input and output.
///
/// `Plugin` is `Send` but not `Sync`: it may be built on one thread and moved to an audio
/// worker, but calls never overlap. VST3 expects controller and state calls on the main
/// thread and `process` on the audio thread; most plugins tolerate everything on one
/// thread while no editor is open, but see the crate docs before opening one.
pub struct Plugin {
    pub(crate) instance: Instance,
}

impl Plugin {
    /// Loads class `class_id` from the bundle at `path` (as found by [`scan`](crate::scan))
    /// and starts it processing 32-bit stereo audio in realtime mode.
    ///
    /// Runs foreign code: never call it on the UI thread.
    pub fn load(
        path: &Path,
        class_id: ClassId,
        sample_rate: u32,
        max_block: usize,
    ) -> Result<Self, PluginError> {
        let binary = bundle_binary(path)?;
        let instance = Instance::load(&binary, class_id, sample_rate, max_block)?;
        Ok(Self { instance })
    }

    pub fn from_info(
        info: &PluginInfo,
        sample_rate: u32,
        max_block: usize,
    ) -> Result<Self, PluginError> {
        Self::load(&info.path, info.class_id, sample_rate, max_block)
    }

    /// Processes any number of frames, split internally into blocks of at most the
    /// `max_block` given to [`load`](Self::load). Parameter changes queued by
    /// [`set_parameter`](Self::set_parameter) reach the processor at the start of the
    /// first block. Does not allocate.
    pub fn process(
        &mut self,
        inputs: [&[f32]; 2],
        outputs: [&mut [f32]; 2],
    ) -> Result<(), PluginError> {
        let frames = inputs[0].len();
        let lengths = [inputs[1].len(), outputs[0].len(), outputs[1].len()];
        if lengths.iter().any(|&len| len != frames) {
            return Err(PluginError::BufferLengthMismatch);
        }
        self.instance.process(inputs, outputs)
    }

    pub fn parameters(&self) -> Vec<ParameterInfo> {
        self.instance.parameters()
    }

    /// The controller's current normalized value.
    pub fn parameter(&self, id: ParamId) -> f64 {
        self.instance.parameter(id)
    }

    /// Sets the controller's normalized value (clamped to `0..=1`) and queues it for the
    /// processor's next `process` call. Does not allocate.
    pub fn set_parameter(&mut self, id: ParamId, normalized: f64) -> Result<(), PluginError> {
        self.instance.set_parameter(id, normalized)
    }

    /// The plugin's own text for `normalized`, e.g. "-6.0 dB".
    pub fn display_value(&self, id: ParamId, normalized: f64) -> Result<String, PluginError> {
        self.instance.display_value(id, normalized)
    }

    /// Saves component and controller state as one blob for [`set_state`](Self::set_state).
    ///
    /// Takes `&mut self` because pending parameter changes are first flushed to the
    /// processor (a zero-length `process` call), so the saved state includes them.
    pub fn state(&mut self) -> Result<Vec<u8>, PluginError> {
        let (component, controller) = self.instance.save_state()?;
        Ok(state::encode(&component, &controller))
    }

    pub fn set_state(&mut self, blob: &[u8]) -> Result<(), PluginError> {
        let (component, controller) = state::decode(blob)?;
        self.instance.restore_state(component, controller)
    }

    pub fn latency_samples(&self) -> u32 {
        self.instance.latency_samples()
    }

    pub fn tail(&self) -> Tail {
        match self.instance.tail_samples() {
            u32::MAX => Tail::Infinite,
            samples => Tail::Samples(samples),
        }
    }

    /// Clears delay lines and reverb tails by deactivating and reactivating the plugin.
    pub fn reset(&mut self) -> Result<(), PluginError> {
        self.instance.reset()
    }

    /// Whether the plugin provides an editor view. Creates and immediately releases one.
    pub fn has_editor(&self) -> bool {
        self.instance.has_editor()
    }
}
