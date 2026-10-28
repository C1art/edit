use serde::{Deserialize, Serialize};

use crate::Param;

/// An effect on an audio clip's sound, applied in list order before its volume.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioEffect {
    /// A shelf below 100 Hz, a bell at `mid_frequency` hertz and a shelf above 8 kHz,
    /// each boosting or cutting by its gain in decibels.
    Equalizer {
        low: Param,
        mid: Param,
        mid_frequency: Param,
        high: Param,
    },
    /// Removes what lies below `frequency` hertz, such as rumble and handling noise.
    HighPass { frequency: Param },
    /// Removes what lies above `frequency` hertz, such as hiss.
    LowPass { frequency: Param },
    /// Turns down what rises above `threshold` decibels to `1 / ratio` of its excess,
    /// reacting over `attack` and `release` milliseconds, then lifts the result by
    /// `makeup` decibels.
    Compressor {
        threshold: Param,
        ratio: Param,
        attack: Param,
        release: Param,
        makeup: Param,
    },
    /// Repeats the sound every `time` milliseconds, each repeat `feedback` percent of the
    /// one before, with the repeats mixed in at `mix` percent.
    Delay {
        time: Param,
        feedback: Param,
        mix: Param,
    },
}

impl AudioEffect {
    pub fn params_mut(&mut self) -> Vec<&mut Param> {
        match self {
            Self::Equalizer {
                low,
                mid,
                mid_frequency,
                high,
            } => vec![low, mid, mid_frequency, high],
            Self::HighPass { frequency } | Self::LowPass { frequency } => vec![frequency],
            Self::Compressor {
                threshold,
                ratio,
                attack,
                release,
                makeup,
            } => vec![threshold, ratio, attack, release, makeup],
            Self::Delay {
                time,
                feedback,
                mix,
            } => vec![time, feedback, mix],
        }
    }

    /// Each effect with its default settings, in the order the effects menu lists them.
    pub fn catalog() -> [Self; 5] {
        [
            Self::Equalizer {
                low: Param::default(),
                mid: Param::default(),
                mid_frequency: Param::from_value(1_000.0),
                high: Param::default(),
            },
            Self::HighPass {
                frequency: Param::from_value(80.0),
            },
            Self::LowPass {
                frequency: Param::from_value(12_000.0),
            },
            Self::Compressor {
                threshold: Param::from_value(-20.0),
                ratio: Param::from_value(4.0),
                attack: Param::from_value(10.0),
                release: Param::from_value(100.0),
                makeup: Param::default(),
            },
            Self::Delay {
                time: Param::from_value(300.0),
                feedback: Param::from_value(30.0),
                mix: Param::from_value(30.0),
            },
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Equalizer { .. } => "Parametric Equalizer",
            Self::HighPass { .. } => "Highpass",
            Self::LowPass { .. } => "Lowpass",
            Self::Compressor { .. } => "Dynamics Compressor",
            Self::Delay { .. } => "Delay",
        }
    }
}
