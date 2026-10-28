use serde::{Deserialize, Serialize};

use crate::Param;

/// An effect on an audio clip's sound, applied in list order before its volume.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioEffect {
    /// Raises or lowers the level by `gain` decibels.
    Amplify {
        gain: Param,
    },
    /// Turns one side down, from -100 (left only) to 100 (right only).
    Balance {
        balance: Param,
    },
    /// Boosts or cuts the lows below 200 Hz by `boost` decibels.
    Bass {
        boost: Param,
    },
    /// Boosts or cuts the highs above 4 kHz by `boost` decibels.
    Treble {
        boost: Param,
    },
    /// A shelf below 100 Hz, a bell at `mid_frequency` hertz and a shelf above 8 kHz,
    /// each boosting or cutting by its gain in decibels.
    Equalizer {
        low: Param,
        mid: Param,
        mid_frequency: Param,
        high: Param,
    },
    /// Removes what lies below `frequency` hertz, such as rumble and handling noise.
    HighPass {
        frequency: Param,
    },
    /// Removes what lies above `frequency` hertz, such as hiss.
    LowPass {
        frequency: Param,
    },
    /// Removes a narrow band around `frequency` hertz, such as mains hum; a higher `q`
    /// narrows it.
    Notch {
        frequency: Param,
        q: Param,
    },
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
    /// Lifts the sound by `boost` decibels and keeps its peaks under `ceiling` decibels,
    /// recovering over `release` milliseconds.
    HardLimiter {
        boost: Param,
        ceiling: Param,
        release: Param,
    },
    /// Mutes the sound while it stays under `threshold` decibels, opening over `attack`
    /// and closing over `release` milliseconds.
    NoiseGate {
        threshold: Param,
        attack: Param,
        release: Param,
    },
    /// Repeats the sound every `time` milliseconds, each repeat `feedback` percent of the
    /// one before, with the repeats mixed in at `mix` percent.
    Delay {
        time: Param,
        feedback: Param,
        mix: Param,
    },
    /// A room's reflections: `size`, `damping` and `mix` in percent.
    Reverb {
        size: Param,
        damping: Param,
        mix: Param,
    },
    /// Turns down what lies above `frequency` hertz, by up to `reduction` decibels, while
    /// it is louder than `threshold` decibels: the hiss of s and sh sounds.
    DeEsser {
        threshold: Param,
        reduction: Param,
        frequency: Param,
    },
    /// Thickens the sound with copies whose delay of up to `depth` milliseconds sweeps
    /// `rate` times a second, mixed in at `mix` percent.
    Chorus {
        rate: Param,
        depth: Param,
        mix: Param,
    },
    /// A short sweeping delay fed back on itself by `feedback` percent, for a jet-like
    /// whoosh; `rate` in hertz, `depth` in milliseconds and `mix` in percent.
    Flanger {
        rate: Param,
        depth: Param,
        feedback: Param,
        mix: Param,
    },
    /// Raises the pitch by `semitones`, lowering it when negative, without changing
    /// timing, mixed in at `mix` percent.
    PitchShifter {
        semitones: Param,
        mix: Param,
    },
    /// Lowers steady background noise, such as hiss, fans and room tone, by up to
    /// `amount` percent; it learns the noise from the quietest moments as it plays.
    DeNoise {
        amount: Param,
    },
    /// Removes mains hum at `frequency` hertz, usually 50 or 60, and its first `harmonics`
    /// multiples counting itself, each cut by `reduction` decibels.
    DeHum {
        frequency: Param,
        harmonics: Param,
        reduction: Param,
    },
    /// Plays the right side on both.
    FillLeftWithRight,
    /// Plays the left side on both.
    FillRightWithLeft,
    SwapChannels,
    /// Flips the polarity of both sides.
    Invert,
}

impl AudioEffect {
    pub fn params_mut(&mut self) -> Vec<&mut Param> {
        match self {
            Self::Amplify { gain: level }
            | Self::Balance { balance: level }
            | Self::Bass { boost: level }
            | Self::Treble { boost: level } => vec![level],
            Self::Equalizer {
                low,
                mid,
                mid_frequency,
                high,
            } => vec![low, mid, mid_frequency, high],
            Self::Notch { frequency, q } => vec![frequency, q],
            Self::HardLimiter {
                boost,
                ceiling,
                release,
            } => vec![boost, ceiling, release],
            Self::NoiseGate {
                threshold,
                attack,
                release,
            } => vec![threshold, attack, release],
            Self::Reverb { size, damping, mix } => vec![size, damping, mix],
            Self::DeEsser {
                threshold,
                reduction,
                frequency,
            } => vec![threshold, reduction, frequency],
            Self::Chorus { rate, depth, mix } => vec![rate, depth, mix],
            Self::Flanger {
                rate,
                depth,
                feedback,
                mix,
            } => vec![rate, depth, feedback, mix],
            Self::PitchShifter { semitones, mix } => vec![semitones, mix],
            Self::DeNoise { amount } => vec![amount],
            Self::DeHum {
                frequency,
                harmonics,
                reduction,
            } => vec![frequency, harmonics, reduction],
            Self::FillLeftWithRight
            | Self::FillRightWithLeft
            | Self::SwapChannels
            | Self::Invert => Vec::new(),
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
    pub fn catalog() -> [Self; 23] {
        [
            Self::Amplify {
                gain: Param::default(),
            },
            Self::Balance {
                balance: Param::default(),
            },
            Self::Bass {
                boost: Param::default(),
            },
            Self::Treble {
                boost: Param::default(),
            },
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
            Self::Notch {
                frequency: Param::from_value(60.0),
                q: Param::from_value(10.0),
            },
            Self::Compressor {
                threshold: Param::from_value(-20.0),
                ratio: Param::from_value(4.0),
                attack: Param::from_value(10.0),
                release: Param::from_value(100.0),
                makeup: Param::default(),
            },
            Self::HardLimiter {
                boost: Param::default(),
                ceiling: Param::from_value(-0.1),
                release: Param::from_value(80.0),
            },
            Self::NoiseGate {
                threshold: Param::from_value(-50.0),
                attack: Param::from_value(2.0),
                release: Param::from_value(100.0),
            },
            Self::Delay {
                time: Param::from_value(300.0),
                feedback: Param::from_value(30.0),
                mix: Param::from_value(30.0),
            },
            Self::Reverb {
                size: Param::from_value(50.0),
                damping: Param::from_value(50.0),
                mix: Param::from_value(25.0),
            },
            Self::DeEsser {
                threshold: Param::from_value(-30.0),
                reduction: Param::from_value(12.0),
                frequency: Param::from_value(5_000.0),
            },
            Self::Chorus {
                rate: Param::from_value(1.0),
                depth: Param::from_value(5.0),
                mix: Param::from_value(50.0),
            },
            Self::Flanger {
                rate: Param::from_value(0.3),
                depth: Param::from_value(3.0),
                feedback: Param::from_value(50.0),
                mix: Param::from_value(50.0),
            },
            Self::PitchShifter {
                semitones: Param::default(),
                mix: Param::from_value(100.0),
            },
            Self::DeNoise {
                amount: Param::from_value(40.0),
            },
            Self::DeHum {
                frequency: Param::from_value(60.0),
                harmonics: Param::from_value(4.0),
                reduction: Param::from_value(30.0),
            },
            Self::FillLeftWithRight,
            Self::FillRightWithLeft,
            Self::SwapChannels,
            Self::Invert,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Amplify { .. } => "Amplify",
            Self::Balance { .. } => "Balance",
            Self::Bass { .. } => "Bass",
            Self::Treble { .. } => "Treble",
            Self::Notch { .. } => "Notch Filter",
            Self::HardLimiter { .. } => "Hard Limiter",
            Self::NoiseGate { .. } => "Noise Gate",
            Self::Reverb { .. } => "Studio Reverb",
            Self::FillLeftWithRight => "Fill Left with Right",
            Self::FillRightWithLeft => "Fill Right with Left",
            Self::SwapChannels => "Swap Channels",
            Self::Invert => "Invert",
            Self::Equalizer { .. } => "Parametric Equalizer",
            Self::HighPass { .. } => "Highpass",
            Self::LowPass { .. } => "Lowpass",
            Self::Compressor { .. } => "Dynamics Compressor",
            Self::Delay { .. } => "Delay",
            Self::DeEsser { .. } => "DeEsser",
            Self::Chorus { .. } => "Chorus",
            Self::Flanger { .. } => "Flanger",
            Self::PitchShifter { .. } => "Pitch Shifter",
            Self::DeNoise { .. } => "DeNoise",
            Self::DeHum { .. } => "DeHum",
        }
    }
}
