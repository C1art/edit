use std::path::{Path, PathBuf};

use crossbeam::channel::Receiver;

/// The Whisper models offered, from the whisper.cpp releases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Model {
    FastEnglish,
    AccurateEnglish,
    FastAnyLanguage,
    AccurateAnyLanguage,
}

impl Model {
    pub const ALL: [Self; 4] = [
        Self::FastEnglish,
        Self::AccurateEnglish,
        Self::FastAnyLanguage,
        Self::AccurateAnyLanguage,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::FastEnglish => "English, fast",
            Self::AccurateEnglish => "English, accurate",
            Self::FastAnyLanguage => "Any language, fast",
            Self::AccurateAnyLanguage => "Any language, accurate",
        }
    }

    pub fn english_only(self) -> bool {
        matches!(self, Self::FastEnglish | Self::AccurateEnglish)
    }

    fn file_name(self) -> &'static str {
        match self {
            Self::FastEnglish => "ggml-base.en.bin",
            Self::AccurateEnglish => "ggml-small.en.bin",
            Self::FastAnyLanguage => "ggml-base.bin",
            Self::AccurateAnyLanguage => "ggml-large-v3-turbo-q5_0.bin",
        }
    }

    /// Close enough to show download progress against.
    pub fn approximate_bytes(self) -> u64 {
        match self {
            Self::FastEnglish | Self::FastAnyLanguage => 148_000_000,
            Self::AccurateEnglish => 488_000_000,
            Self::AccurateAnyLanguage => 547_000_000,
        }
    }

    pub(crate) fn url(self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            self.file_name()
        )
    }

    pub fn path(self, models: &Path) -> PathBuf {
        models.join(self.file_name())
    }
}

/// The languages a multilingual model understands, as `(code, English name)`.
pub fn languages() -> Vec<(&'static str, &'static str)> {
    (0..=whisper_rs::get_lang_max_id())
        .filter_map(|id| {
            Some((
                whisper_rs::get_lang_str(id)?,
                whisper_rs::get_lang_str_full(id)?,
            ))
        })
        .collect()
}

/// The models already downloaded into `models`, found on a worker thread so the
/// interface never waits on the disk.
pub fn downloaded(models: PathBuf) -> Receiver<Vec<Model>> {
    let (sender, found) = crossbeam::channel::bounded(1);
    let spawned = std::thread::Builder::new()
        .name("model scan".into())
        .spawn(move || {
            let present = Model::ALL
                .into_iter()
                .filter(|model| model.path(&models).is_file())
                .collect();
            let _ = sender.send(present);
        });
    // Without the worker nothing is known to be downloaded, and downloading again
    // replaces the file.
    drop(spawned);
    found
}
