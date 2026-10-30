//! Speech recognition: transcribing media with a downloaded Whisper model.

mod download;
mod error;
mod models;
mod transcribe;
mod words;

pub use download::{DownloadEvent, DownloadJob};
pub use error::SpeechError;
pub use models::{downloaded, languages, Model};
pub use transcribe::{TranscribeEvent, TranscribeJob};
