//! VST3 enum constants as the types the ABI takes. The generated constants are unsigned
//! on Linux and signed on Windows, so the casts are needed on one platform only.
#![allow(clippy::unnecessary_cast)]

use vst3::Steinberg::Vst::BusDirections_::{kInput, kOutput};
use vst3::Steinberg::Vst::MediaTypes_::kAudio;
use vst3::Steinberg::Vst::ProcessContext_::StatesAndFlags_::{kTempoValid, kTimeSigValid};
use vst3::Steinberg::Vst::ProcessModes_::kRealtime;
use vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32;
use vst3::Steinberg::Vst::{BusDirection, MediaType};

pub(super) const AUDIO: MediaType = kAudio as MediaType;
pub(super) const INPUT: BusDirection = kInput as BusDirection;
pub(super) const OUTPUT: BusDirection = kOutput as BusDirection;
pub(super) const REALTIME: i32 = kRealtime as i32;
pub(super) const SAMPLE_32: i32 = kSample32 as i32;
pub(super) const TEMPO_AND_TIME_SIGNATURE_VALID: u32 = (kTempoValid | kTimeSigValid) as u32;
