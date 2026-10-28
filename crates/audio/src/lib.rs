mod error;

pub use error::AudioError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SampleRate(pub u32);
