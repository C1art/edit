use serde::{Deserialize, Serialize};

use crate::Param;

mod catalog;
mod convolution;
mod dynamics;
mod guitar;
mod plugin;
mod scientific;
mod vocal;

pub use convolution::ReverbSpace;
pub use dynamics::{CompressorSection, Dynamics, ExpanderSection, GateSection, LimiterSection};
pub use guitar::{AmpModel, GuitarFilter};
pub use plugin::{PluginEffect, PluginParam};
pub use scientific::{FilterMode, FilterType};
pub use vocal::VocalMode;

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
    /// Raises or lowers each side on its own, by `left` and `right` decibels.
    ChannelVolume {
        left: Param,
        right: Param,
    },
    /// Ten bells an octave apart from 31 Hz to 16 kHz, each boosting or cutting by its
    /// gain in decibels.
    GraphicEqualizer {
        bands: [Param; 10],
    },
    /// Twenty bells half an octave apart from 31 Hz to 22 kHz.
    GraphicEqualizer20 {
        bands: Box<[Param; 20]>,
    },
    /// Thirty bells a third of an octave apart from 25 Hz to 20 kHz.
    GraphicEqualizer30 {
        bands: Box<[Param; 30]>,
    },
    /// Compresses the lows under `low_crossover` hertz, the mids up to `high_crossover`
    /// and the highs above it each on its own, against its own threshold in decibels,
    /// as the Dynamics Compressor does, then lifts the sum by `makeup` decibels.
    MultibandCompressor {
        low_crossover: Param,
        high_crossover: Param,
        low_threshold: Param,
        mid_threshold: Param,
        high_threshold: Param,
        ratio: Param,
        attack: Param,
        release: Param,
        makeup: Param,
    },
    /// Passes only a band around `frequency` hertz; a higher `q` narrows it.
    BandPass {
        frequency: Param,
        q: Param,
    },
    /// Sweeps notches through the sound: `stages` all-pass filters turning around
    /// `frequency` hertz, swept `depth` percent of two octaves each way `rate` times a
    /// second, fed back by `feedback` percent and mixed in at `mix` percent.
    Phaser {
        stages: Param,
        frequency: Param,
        depth: Param,
        rate: Param,
        feedback: Param,
        mix: Param,
    },
    /// Soft-clips the sound after `drive` decibels of gain, mixed in at `mix` percent.
    Distortion {
        drive: Param,
        mix: Param,
    },
    /// Spreads the sides to `width` percent of their difference: 0 makes mono and 100
    /// leaves the sound as it is.
    StereoExpander {
        width: Param,
    },
    /// Four echoes, each after its `times` milliseconds at its `levels` percent.
    MultitapDelay {
        times: [Param; 4],
        levels: [Param; 4],
    },
    /// Echoes `time` milliseconds apart, each `feedback` percent of the one before and
    /// darker above `tone` hertz and more saturated by `saturation` percent, as tape
    /// echoes are, mixed in at `mix` percent.
    AnalogDelay {
        time: Param,
        feedback: Param,
        saturation: Param,
        tone: Param,
        mix: Param,
    },
    Dynamics(Dynamics),
    /// Repairs single-sample clicks and pops; a lower `threshold`, in percent, catches
    /// fainter ones.
    AutomaticClickRemover {
        threshold: Param,
    },
    /// Shapes a voice for clarity in one go.
    VocalEnhancer {
        mode: VocalMode,
    },
    /// The Dynamics Compressor followed by tube-like saturation mixed in at `warmth`
    /// percent.
    TubeCompressor {
        threshold: Param,
        ratio: Param,
        attack: Param,
        release: Param,
        makeup: Param,
        warmth: Param,
    },
    /// Turns a room's echo down by `amount` percent, then adds `gain` decibels to make up
    /// for the level it takes away.
    DeReverb {
        amount: Param,
        gain: Param,
    },
    /// Rides a voice's level toward `target` decibels, `amount` percent of the way; sound
    /// under `noise_threshold` decibels is taken for pauses and left as it is.
    SpeechVolumeLeveler {
        target: Param,
        amount: Param,
        noise_threshold: Param,
    },
    /// One bell at `frequency` hertz, `gain` decibels up or down, `q` narrow.
    SimpleParametricEq {
        frequency: Param,
        gain: Param,
        q: Param,
    },
    /// A filter of a chosen design and steepness: `order` is 1 to 8, `cutoff` the corner
    /// in hertz or a band's lower edge, `high_cutoff` a band's upper edge, `ripple` the
    /// decibels a Chebyshev passband ripples by and `gain` decibels after.
    ScientificFilter {
        filter_type: FilterType,
        mode: FilterMode,
        order: u32,
        cutoff: Param,
        high_cutoff: Param,
        ripple: Param,
        gain: Param,
    },
    /// Plays the sound through a space's recorded response: `room_size` and `damping`
    /// (of the high end) in percent and `pre_delay` in milliseconds reshape the space, then
    /// `width` percent spreads it, `gain` decibels sets its level and `mix` percent of it
    /// goes over the dry sound.
    ConvolutionReverb {
        space: ReverbSpace,
        room_size: u32,
        damping: u32,
        pre_delay: u32,
        width: Param,
        gain: Param,
        mix: Param,
    },
    /// A guitar rig: `compressor` and `distortion` amounts in percent, an amp and its
    /// cabinet, then a filter at `frequency` hertz that rings more as `resonance` (percent)
    /// rises, with `mix` percent of the result over the dry sound.
    GuitarSuite {
        compressor: Param,
        distortion: Param,
        amp: AmpModel,
        filter: GuitarFilter,
        frequency: Param,
        resonance: Param,
        mix: Param,
    },
    /// Finishes a mix in one go: low, mid and high bands in decibels, then reverb,
    /// exciter, width (100 leaves it as it is) and loudness maximizer in percent, then
    /// `output` decibels.
    Mastering {
        low: Param,
        mid: Param,
        high: Param,
        reverb: Param,
        exciter: Param,
        width: Param,
        maximizer: Param,
        output: Param,
    },
    /// Plays the right side on both.
    FillLeftWithRight,
    /// Plays the left side on both.
    FillRightWithLeft,
    SwapChannels,
    /// Flips the polarity of both sides.
    Invert,
    /// A VST3 plugin's effect.
    Plugin(Box<PluginEffect>),
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
            Self::ChannelVolume { left, right } => vec![left, right],
            Self::GraphicEqualizer { bands } => bands.iter_mut().collect(),
            Self::GraphicEqualizer20 { bands } => bands.iter_mut().collect(),
            Self::GraphicEqualizer30 { bands } => bands.iter_mut().collect(),
            Self::MultibandCompressor {
                low_crossover,
                high_crossover,
                low_threshold,
                mid_threshold,
                high_threshold,
                ratio,
                attack,
                release,
                makeup,
            } => vec![
                low_crossover,
                high_crossover,
                low_threshold,
                mid_threshold,
                high_threshold,
                ratio,
                attack,
                release,
                makeup,
            ],
            Self::BandPass { frequency, q } => vec![frequency, q],
            Self::Phaser {
                stages,
                frequency,
                depth,
                rate,
                feedback,
                mix,
            } => vec![stages, frequency, depth, rate, feedback, mix],
            Self::Distortion { drive, mix } => vec![drive, mix],
            Self::StereoExpander { width } => vec![width],
            Self::MultitapDelay { times, levels } => times.iter_mut().chain(levels).collect(),
            Self::AnalogDelay {
                time,
                feedback,
                saturation,
                tone,
                mix,
            } => vec![time, feedback, saturation, tone, mix],
            Self::Dynamics(dynamics) => dynamics.params_mut(),
            Self::Plugin(plugin) => plugin
                .params
                .iter_mut()
                .map(|param| &mut param.value)
                .collect(),
            Self::AutomaticClickRemover { threshold } => vec![threshold],
            Self::TubeCompressor {
                threshold,
                ratio,
                attack,
                release,
                makeup,
                warmth,
            } => vec![threshold, ratio, attack, release, makeup, warmth],
            Self::DeReverb { amount, gain } => vec![amount, gain],
            Self::Mastering {
                low,
                mid,
                high,
                reverb,
                exciter,
                width,
                maximizer,
                output,
            } => vec![low, mid, high, reverb, exciter, width, maximizer, output],
            Self::SpeechVolumeLeveler {
                target,
                amount,
                noise_threshold,
            } => vec![target, amount, noise_threshold],
            Self::SimpleParametricEq { frequency, gain, q } => vec![frequency, gain, q],
            Self::ConvolutionReverb {
                width, gain, mix, ..
            } => vec![width, gain, mix],
            Self::GuitarSuite {
                compressor,
                distortion,
                frequency,
                resonance,
                mix,
                ..
            } => vec![compressor, distortion, frequency, resonance, mix],
            Self::ScientificFilter {
                cutoff,
                high_cutoff,
                ripple,
                gain,
                ..
            } => vec![cutoff, high_cutoff, ripple, gain],
            Self::VocalEnhancer { .. }
            | Self::FillLeftWithRight
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
}
