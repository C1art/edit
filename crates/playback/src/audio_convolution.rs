use audio::{synthesize_impulse, widen, Convolver, ImpulseShape, SampleRate};
use timeline::ReverbSpace;

/// Frames mixed at a time, so the dry sound fits on the stack.
const BLOCK_FRAMES: usize = 256;

/// What the space's response is built from; a change rebuilds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpaceSettings {
    pub(crate) space: ReverbSpace,
    pub(crate) room_size: u32,
    pub(crate) damping: u32,
    pub(crate) pre_delay: u32,
}

pub(crate) struct ReverbLevels {
    pub(crate) width: f32,
    pub(crate) gain: f32,
    pub(crate) mix: f32,
}

/// Convolution Reverb: the sound through a synthesized space's response.
pub(crate) struct ConvolutionReverb {
    built: SpaceSettings,
    convolver: Convolver,
}

impl ConvolutionReverb {
    pub(crate) fn new(settings: SpaceSettings, rate: SampleRate) -> Self {
        Self {
            built: settings,
            convolver: Convolver::new(&synthesize_impulse(&shape(settings), rate)),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.convolver.reset();
    }

    pub(crate) fn process(
        &mut self,
        settings: SpaceSettings,
        levels: &ReverbLevels,
        rate: SampleRate,
        samples: &mut [f32],
    ) {
        if settings != self.built {
            *self = Self::new(settings, rate);
        }
        let mix = levels.mix.clamp(0.0, 1.0);
        for block in samples.chunks_mut(BLOCK_FRAMES * 2) {
            let mut dry = [0.0; BLOCK_FRAMES * 2];
            dry[..block.len()].copy_from_slice(block);
            self.convolver.process(block);
            widen(levels.width, block);
            for (wet, dry) in block.iter_mut().zip(dry) {
                *wet = dry + (*wet * levels.gain - dry) * mix;
            }
        }
    }
}

fn shape(settings: SpaceSettings) -> ImpulseShape {
    let (decay_seconds, early_ms) = match settings.space {
        ReverbSpace::SmallRoom => (0.4, 20.0),
        ReverbSpace::MediumRoom => (0.9, 35.0),
        ReverbSpace::ConcertHall => (2.0, 60.0),
        ReverbSpace::Cathedral => (3.5, 90.0),
        ReverbSpace::Plate => (1.6, 0.0),
    };
    // Half size is the space as named; each quarter either way halves or doubles it.
    let size = 2.0_f32.powf((settings.room_size.min(100) as f32 - 50.0) / 25.0);
    ImpulseShape {
        decay_seconds: decay_seconds * size,
        early_ms: early_ms * size.sqrt(),
        damping: settings.damping.min(100) as f32 / 100.0,
        pre_delay_ms: settings.pre_delay as f32,
    }
}
