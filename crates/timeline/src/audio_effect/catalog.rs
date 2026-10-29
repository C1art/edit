use super::{AudioEffect, Dynamics, VocalMode};
use crate::Param;

impl AudioEffect {
    /// Each effect with its default settings, in the order the effects menu lists them.
    pub fn catalog() -> [Self; 41] {
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
            Self::ChannelVolume {
                left: Param::default(),
                right: Param::default(),
            },
            Self::GraphicEqualizer {
                bands: Default::default(),
            },
            Self::GraphicEqualizer20 {
                bands: Default::default(),
            },
            Self::GraphicEqualizer30 {
                bands: Default::default(),
            },
            Self::MultibandCompressor {
                low_crossover: Param::from_value(200.0),
                high_crossover: Param::from_value(2_000.0),
                low_threshold: Param::from_value(-20.0),
                mid_threshold: Param::from_value(-20.0),
                high_threshold: Param::from_value(-20.0),
                ratio: Param::from_value(3.0),
                attack: Param::from_value(10.0),
                release: Param::from_value(150.0),
                makeup: Param::default(),
            },
            Self::BandPass {
                frequency: Param::from_value(1_000.0),
                q: Param::from_value(2.0),
            },
            Self::Phaser {
                stages: Param::from_value(4.0),
                frequency: Param::from_value(800.0),
                depth: Param::from_value(50.0),
                rate: Param::from_value(0.5),
                feedback: Param::from_value(30.0),
                mix: Param::from_value(50.0),
            },
            Self::Distortion {
                drive: Param::from_value(12.0),
                mix: Param::from_value(100.0),
            },
            Self::StereoExpander {
                width: Param::from_value(150.0),
            },
            Self::MultitapDelay {
                times: [125.0, 250.0, 375.0, 500.0].map(Param::from_value),
                levels: [50.0, 35.0, 25.0, 15.0].map(Param::from_value),
            },
            Self::AnalogDelay {
                time: Param::from_value(350.0),
                feedback: Param::from_value(40.0),
                saturation: Param::from_value(30.0),
                tone: Param::from_value(3_000.0),
                mix: Param::from_value(35.0),
            },
            Self::Dynamics(Dynamics::default()),
            Self::AutomaticClickRemover {
                threshold: Param::from_value(30.0),
            },
            Self::VocalEnhancer {
                mode: VocalMode::default(),
            },
            Self::TubeCompressor {
                threshold: Param::from_value(-20.0),
                ratio: Param::from_value(4.0),
                attack: Param::from_value(10.0),
                release: Param::from_value(150.0),
                makeup: Param::default(),
                warmth: Param::from_value(40.0),
            },
            Self::DeReverb {
                amount: Param::from_value(40.0),
                gain: Param::default(),
            },
            Self::SpeechVolumeLeveler {
                target: Param::from_value(-18.0),
                amount: Param::from_value(50.0),
                noise_threshold: Param::from_value(-50.0),
            },
            Self::SimpleParametricEq {
                frequency: Param::from_value(1000.0),
                gain: Param::default(),
                q: Param::from_value(1.0),
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
            Self::ChannelVolume { .. } => "Channel Volume",
            Self::GraphicEqualizer { .. } => "Graphic Equalizer (10 Bands)",
            Self::GraphicEqualizer20 { .. } => "Graphic Equalizer (20 Bands)",
            Self::GraphicEqualizer30 { .. } => "Graphic Equalizer (30 Bands)",
            Self::MultibandCompressor { .. } => "Multiband Compressor",
            Self::BandPass { .. } => "Bandpass",
            Self::Phaser { .. } => "Phaser",
            Self::Distortion { .. } => "Distortion",
            Self::StereoExpander { .. } => "Stereo Expander",
            Self::MultitapDelay { .. } => "Multitap Delay",
            Self::AnalogDelay { .. } => "Analog Delay",
            Self::Dynamics(_) => "Dynamics",
            Self::AutomaticClickRemover { .. } => "Automatic Click Remover",
            Self::VocalEnhancer { .. } => "Vocal Enhancer",
            Self::TubeCompressor { .. } => "Tube-modeled Compressor",
            Self::DeReverb { .. } => "DeReverb",
            Self::SpeechVolumeLeveler { .. } => "Speech Volume Leveler",
            Self::SimpleParametricEq { .. } => "Simple Parametric EQ",
        }
    }
}
