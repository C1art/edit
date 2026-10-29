use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use crossbeam::channel::{self, Receiver, Sender, TryRecvError};
use timeline::MatteChannel;

use crate::blit::{render_pipeline, DISPLAY_FORMAT};
use crate::layer_shader::{Features, LayerShader};
use crate::WORKING_FORMAT;

/// What a layer pipeline draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LayerPass {
    /// Blends a layer over the working texture.
    Over,
    /// Draws a layer over the working texture by its blend mode.
    Blended,
    /// Draws a group combined with another, bound as the backdrop.
    Combined,
    /// Keeps the part of a fill its track matte shows, by channel, then reversed or not.
    Matte(MatteChannel, bool),
    /// Copies the working texture to the display texture.
    Display,
}

/// The matte entry points, in `MatteChannel`'s order.
const MATTE_ENTRIES: [&str; 7] = [
    "fs_matte_alpha",
    "fs_matte_luma",
    "fs_matte_red",
    "fs_matte_green",
    "fs_matte_blue",
    "fs_matte_saturation",
    "fs_matte_lightness",
];

type Key = (Features, LayerPass);

/// The layer pipelines built so far, and the means to build the rest: where they're
/// asked for, or on a worker, so the thread drawing never waits on the driver's
/// shader compiler.
pub(crate) struct LayerPipelines {
    shader: Arc<LayerShader>,
    builder: Arc<Mutex<Builder>>,
    built: RefCell<HashMap<Key, wgpu::RenderPipeline>>,
    worker: Option<Worker>,
    /// Whether a pipeline was asked for that the worker hasn't built yet.
    missed: Cell<bool>,
}

struct Worker {
    requests: Sender<Key>,
    finished: Receiver<(Key, wgpu::RenderPipeline)>,
    asked: RefCell<HashSet<Key>>,
}

/// Compiles the layer shader cut down to each set of effects, once per set.
struct Builder {
    device: wgpu::Device,
    shader: Arc<LayerShader>,
    blit_layout: wgpu::PipelineLayout,
    blended_layout: wgpu::PipelineLayout,
    modules: HashMap<Features, wgpu::ShaderModule>,
}

impl LayerPipelines {
    pub(crate) fn new(
        device: &wgpu::Device,
        blit_layout: wgpu::PipelineLayout,
        blended_layout: wgpu::PipelineLayout,
    ) -> Self {
        let shader = Arc::new(LayerShader::new());
        let builder = Builder {
            device: device.clone(),
            shader: Arc::clone(&shader),
            blit_layout,
            blended_layout,
            modules: HashMap::new(),
        };
        Self {
            shader,
            builder: Arc::new(Mutex::new(builder)),
            built: RefCell::default(),
            worker: None,
            missed: Cell::new(false),
        }
    }

    pub(crate) fn features(&self, placement: &[u8]) -> Features {
        self.shader.features(placement)
    }

    /// From now on, builds pipelines on a worker thread. Stays as it was if the thread
    /// can't be started.
    pub(crate) fn build_in_background(&mut self) {
        if self.worker.is_some() {
            return;
        }
        let (requests, asked_for) = channel::unbounded::<Key>();
        let (done, finished) = channel::unbounded();
        let builder = Arc::clone(&self.builder);
        let spawned = std::thread::Builder::new()
            .name("shader compiler".into())
            .spawn(move || {
                for key in asked_for {
                    let pipeline = lock(&builder).build(key);
                    if done.send((key, pipeline)).is_err() {
                        break;
                    }
                }
            });
        if spawned.is_ok() {
            self.worker = Some(Worker {
                requests,
                finished,
                asked: RefCell::default(),
            });
        }
    }

    /// The pipeline drawing `pass` with only the effects in `features`. Built here the
    /// first time it's asked for, or, when building in the background, `None` until
    /// the worker has built it.
    pub(crate) fn get(&self, features: Features, pass: LayerPass) -> Option<wgpu::RenderPipeline> {
        let key = (features, pass);
        let worker_alive = self.worker.as_ref().map(|worker| self.collect(worker));
        if let Some(pipeline) = self.built.borrow().get(&key) {
            return Some(pipeline.clone());
        }
        if let (Some(worker), Some(true)) = (&self.worker, worker_alive) {
            let fresh = worker.asked.borrow_mut().insert(key);
            // A worker that stopped taking requests leaves the building to this thread.
            if !fresh || worker.requests.send(key).is_ok() {
                self.missed.set(true);
                return None;
            }
        }
        let pipeline = lock(&self.builder).build(key);
        self.built.borrow_mut().insert(key, pipeline.clone());
        Some(pipeline)
    }

    /// Whether any pipeline was missing since this was last asked.
    pub(crate) fn take_missed(&self) -> bool {
        self.missed.replace(false)
    }

    /// Whether the worker is still building pipelines that were asked for.
    pub(crate) fn building(&self) -> bool {
        let Some(worker) = &self.worker else {
            return false;
        };
        if !self.collect(worker) {
            return false;
        }
        let built = self.built.borrow();
        worker
            .asked
            .borrow()
            .iter()
            .any(|key| !built.contains_key(key))
    }

    /// Takes in what the worker has finished; false once the worker is gone.
    fn collect(&self, worker: &Worker) -> bool {
        let mut built = self.built.borrow_mut();
        loop {
            match worker.finished.try_recv() {
                Ok((key, pipeline)) => {
                    built.insert(key, pipeline);
                }
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => return false,
            }
        }
    }
}

impl Builder {
    fn build(&mut self, (features, pass): Key) -> wgpu::RenderPipeline {
        let module = self.modules.entry(features).or_insert_with(|| {
            self.device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("layer.wgsl"),
                    source: wgpu::ShaderSource::Wgsl(self.shader.source(features).into()),
                })
        });
        let over = Some(wgpu::BlendState::ALPHA_BLENDING);
        let (layout, target, entry) = match pass {
            LayerPass::Over => (&self.blit_layout, (WORKING_FORMAT, over), "fs_main"),
            LayerPass::Blended => (&self.blended_layout, (WORKING_FORMAT, over), "fs_blend"),
            LayerPass::Combined => (&self.blended_layout, (WORKING_FORMAT, over), "fs_combine"),
            LayerPass::Matte(channel, reverse) => {
                let keep = wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: if reverse {
                        wgpu::BlendFactor::OneMinusSrcAlpha
                    } else {
                        wgpu::BlendFactor::SrcAlpha
                    },
                    operation: wgpu::BlendOperation::Add,
                };
                let blend = wgpu::BlendState {
                    color: keep,
                    alpha: keep,
                };
                (
                    &self.blit_layout,
                    (WORKING_FORMAT, Some(blend)),
                    MATTE_ENTRIES[channel as usize],
                )
            }
            LayerPass::Display => (&self.blit_layout, (DISPLAY_FORMAT, None), "fs_main"),
        };
        render_pipeline(&self.device, module, layout, target, entry)
    }
}

/// A builder that panicked mid-build still holds only finished modules.
fn lock(builder: &Mutex<Builder>) -> std::sync::MutexGuard<'_, Builder> {
    builder.lock().unwrap_or_else(PoisonError::into_inner)
}
