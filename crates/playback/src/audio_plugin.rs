//! VST3 plugins in an effect chain. Loading one runs its own code for a while, so loaded
//! instances are kept per thread and reused when a chain moves on to the next clip, and
//! live playback has them loaded and unloaded on a worker so its sound never waits.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::sync::OnceLock;

use crossbeam::channel::{self, Receiver, Sender, TryRecvError};

use audio::SampleRate;
use plugins::{ClassId, ParamId, Plugin};
use timeline::{PluginEffect, PluginState, Ticks};

/// Plenty for one chunk of the mix; longer runs are split by the plugin host.
const MAX_BLOCK: usize = 4096;
/// Idle instances kept per plugin, enough for a few clips with it playing at once.
const SPARE_INSTANCES: usize = 4;

#[derive(Clone, PartialEq)]
struct Key {
    path: PathBuf,
    class_id: [u8; 16],
    rate: u32,
}

/// How a mixing thread gets its plugins loaded.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum PluginLoading {
    /// On the thread itself, so every sample is processed: for exports and analysis.
    InPlace,
    /// On the plugin worker, the sound passing unchanged until the plugin is ready: for
    /// live playback, which mustn't stall while a plugin starts or a DLL loads.
    Background,
}

struct Loaded {
    plugin: Plugin,
    /// What the plugin saved of itself when loaded, to undo another chain's settings.
    defaults: Vec<u8>,
}

thread_local! {
    static SPARE: RefCell<Vec<(Key, Loaded)>> = const { RefCell::new(Vec::new()) };
    static LOADING: Cell<PluginLoading> = const { Cell::new(PluginLoading::InPlace) };
}

/// Sets how the thread loads plugins, and unloads its spare plugins when dropped. Left to
/// the thread's end, they would unload while Windows holds the loader lock, which
/// deadlocks against a plugin being loaded elsewhere and hangs plugins that wait for their
/// own threads.
pub(crate) struct SparePlugins;

impl SparePlugins {
    pub(crate) fn new(loading: PluginLoading) -> Self {
        LOADING.set(loading);
        Self
    }
}

impl Drop for SparePlugins {
    fn drop(&mut self) {
        let spares = SPARE.try_with(|spare| std::mem::take(&mut *spare.borrow_mut()));
        for (_, loaded) in spares.into_iter().flatten() {
            unload(loaded);
        }
    }
}

enum Job {
    Load {
        key: Key,
        reply: Sender<Option<Loaded>>,
    },
    Unload(Box<Loaded>),
}

/// The one thread that loads and unloads live playback's plugins. It runs for the rest of
/// the process once started; None if it couldn't be started.
fn worker() -> Option<&'static Sender<Job>> {
    static WORKER: OnceLock<Option<Sender<Job>>> = OnceLock::new();
    WORKER
        .get_or_init(|| {
            let (jobs, incoming) = channel::unbounded::<Job>();
            std::thread::Builder::new()
                .name("vst3-loader".into())
                .spawn(move || {
                    for job in incoming {
                        match job {
                            // A unit gone before its plugin was ready drops it here.
                            Job::Load { key, reply } => drop(reply.send(load(&key))),
                            Job::Unload(loaded) => drop(loaded),
                        }
                    }
                })
                .ok()
                .map(|_| jobs)
        })
        .as_ref()
}

pub(crate) struct PluginUnit {
    key: Key,
    /// None when the plugin couldn't be loaded, or isn't loaded yet; its sound then passes
    /// unchanged.
    loaded: Option<Loaded>,
    /// The plugin being loaded on the worker.
    pending: Option<Receiver<Option<Loaded>>>,
    /// The effect's saved state the plugin was last given; empty for its defaults.
    restored: PluginState,
    /// The last value given to each of the effect's changed parameters.
    sent: Vec<f32>,
    input: [Vec<f32>; 2],
    output: [Vec<f32>; 2],
}

impl PluginUnit {
    pub(crate) fn new(effect: &PluginEffect, rate: SampleRate) -> Self {
        let key = Key {
            path: effect.path.clone(),
            class_id: effect.class_id,
            rate: rate.0,
        };
        let mut unit = Self {
            key,
            loaded: None,
            pending: None,
            restored: PluginState::default(),
            sent: Vec::new(),
            input: Default::default(),
            output: Default::default(),
        };
        match take_spare(&unit.key) {
            Some(spare) => unit.loaded = Some(spare),
            None => unit.start_loading(),
        }
        unit
    }

    fn start_loading(&mut self) {
        let worker = worker().filter(|_| LOADING.get() == PluginLoading::Background);
        match worker {
            Some(worker) => {
                let (reply, ready) = channel::bounded(1);
                let job = Job::Load {
                    key: self.key.clone(),
                    reply,
                };
                self.pending = worker.send(job).is_ok().then_some(ready);
            }
            None => self.loaded = load(&self.key),
        }
    }

    /// The plugin, once it has finished loading.
    fn ready(&mut self) -> Option<&mut Loaded> {
        if let Some(pending) = &self.pending {
            match pending.try_recv() {
                Ok(loaded) => {
                    self.loaded = loaded;
                    self.pending = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.pending = None,
            }
        }
        self.loaded.as_mut()
    }

    pub(crate) fn reset(&mut self) {
        let failed = (self.loaded.as_mut()).is_some_and(|loaded| loaded.plugin.reset().is_err());
        if failed {
            // It may be left inactive, which plugins aren't made to process in; a fresh
            // instance is, once given the effect's state again.
            if let Some(broken) = self.loaded.take() {
                unload(broken);
            }
            self.restored = PluginState::default();
            self.sent.clear();
            self.start_loading();
        }
    }

    /// Runs the plugin over interleaved stereo `samples`, with `effect`'s parameters as
    /// they are at `time`.
    pub(crate) fn process(
        &mut self,
        effect: &PluginEffect,
        time: Ticks,
        rate: SampleRate,
        samples: &mut [f32],
    ) {
        if self.key.path != effect.path || self.key.class_id != effect.class_id {
            *self = Self::new(effect, rate);
        }
        if self.ready().is_none() {
            return;
        }
        let Some(loaded) = &mut self.loaded else {
            return;
        };
        if effect.state != self.restored {
            let state = if effect.state.is_empty() {
                &loaded.defaults
            } else {
                effect.state.bytes()
            };
            // A state the plugin refuses leaves it as it was, as other hosts do.
            let _ = loaded.plugin.set_state(state);
            self.restored = effect.state.clone();
            // The state may have moved any parameter, so all are sent again.
            self.sent.clear();
        }
        if self.sent.len() != effect.params.len() {
            // Rounded as the model holds values, so a parameter that matches the restored
            // state to the hundredth leaves the state's finer value alone.
            self.sent = effect
                .params
                .iter()
                .map(|param| {
                    (loaded.plugin.parameter(ParamId(param.id)) * 100.0).round() as f32 / 100.0
                })
                .collect();
        }
        for (param, sent) in effect.params.iter().zip(&mut self.sent) {
            let value = param.value.value_at(time).clamp(0.0, 1.0);
            if value != *sent
                && loaded
                    .plugin
                    .set_parameter(ParamId(param.id), f64::from(value))
                    .is_ok()
            {
                *sent = value;
            }
        }
        let frames = samples.len() / 2;
        for buffer in self.input.iter_mut().chain(&mut self.output) {
            buffer.resize(frames, 0.0);
        }
        let [input_left, input_right] = &mut self.input;
        for (frame, (left, right)) in samples
            .as_chunks::<2>()
            .0
            .iter()
            .zip(input_left.iter_mut().zip(input_right.iter_mut()))
        {
            (*left, *right) = (frame[0], frame[1]);
        }
        let [output_left, output_right] = &mut self.output;
        let inputs = [input_left.as_slice(), input_right.as_slice()];
        if loaded
            .plugin
            .process(inputs, [output_left, output_right])
            .is_err()
        {
            return;
        }
        for (frame, (left, right)) in samples
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(output_left.iter().zip(output_right.iter()))
        {
            *frame = [*left, *right];
        }
    }
}

impl Drop for PluginUnit {
    fn drop(&mut self) {
        let Some(mut loaded) = self.loaded.take() else {
            return;
        };
        if loaded.plugin.set_state(&loaded.defaults).is_err() || loaded.plugin.reset().is_err() {
            unload(loaded);
            return;
        }
        let key = self.key.clone();
        // A thread ending drops its spares itself, so `try_with` failing loses nothing.
        let surplus = SPARE.try_with(|spare| {
            let mut spare = spare.borrow_mut();
            let kept = spare
                .iter()
                .filter(|(spare_key, _)| *spare_key == key)
                .count();
            if kept < SPARE_INSTANCES {
                spare.push((key, loaded));
                None
            } else {
                Some(loaded)
            }
        });
        if let Ok(Some(surplus)) = surplus {
            unload(surplus);
        }
    }
}

/// Unloading runs the plugin's own code too, so live playback hands it to the worker.
fn unload(loaded: Loaded) {
    let background = LOADING
        .try_with(Cell::get)
        .is_ok_and(|loading| loading == PluginLoading::Background);
    match worker().filter(|_| background) {
        // A worker that's gone hands the plugin back, to be dropped here.
        Some(worker) => drop(worker.send(Job::Unload(Box::new(loaded)))),
        None => drop(loaded),
    }
}

fn take_spare(key: &Key) -> Option<Loaded> {
    SPARE
        .try_with(|spare| {
            let mut spare = spare.borrow_mut();
            let index = spare.iter().position(|(spare_key, _)| spare_key == key)?;
            Some(spare.swap_remove(index).1)
        })
        .ok()
        .flatten()
}

fn load(key: &Key) -> Option<Loaded> {
    let mut plugin = Plugin::load(&key.path, ClassId(key.class_id), key.rate, MAX_BLOCK).ok()?;
    let defaults = plugin.state().ok()?;
    Some(Loaded { plugin, defaults })
}

#[cfg(test)]
#[path = "audio_plugin_tests.rs"]
mod tests;
