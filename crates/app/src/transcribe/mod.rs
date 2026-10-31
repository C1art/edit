//! Speech to text: transcribing the media heard in the sequence with a downloaded
//! Whisper model, then captions and editing by text from what was said.

mod dialog;
mod panel;
mod text_layout;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use speech::{DownloadEvent, DownloadJob, Model, TranscribeEvent, TranscribeJob};
use timeline::{SetTranscript, SourceId};

use crate::editor::Editor;

pub(crate) use dialog::{dialog, TranscribeDialog};
pub(crate) use panel::{take_keys, ui, TextPanel};

/// Transcribing a queue of media, after downloading the model if it's missing.
pub(crate) struct Transcribing {
    model: Model,
    language: Option<String>,
    captions_after: bool,
    queue: VecDeque<SourceId>,
    count: usize,
    /// None between one media's job and the next.
    stage: Option<Stage>,
    progress: f32,
}

enum Stage {
    Downloading(DownloadJob),
    Hearing {
        source: SourceId,
        job: TranscribeJob,
    },
}

pub(crate) fn models_folder() -> PathBuf {
    crate::project_store::app_folder().join("models")
}

impl Transcribing {
    /// What is happening, for the Text panel.
    pub(crate) fn status(&self, editor: &Editor) -> (String, f32) {
        let text = match &self.stage {
            None => "Transcribing…".to_owned(),
            Some(Stage::Downloading(_)) => {
                format!("Downloading the {} model…", self.model.label())
            }
            Some(Stage::Hearing { source, .. }) => {
                let name = editor
                    .timeline
                    .sequence()
                    .media_item(*source)
                    .map_or("media", |item| item.name.as_str());
                let at = self.count - self.queue.len();
                format!("Transcribing {at} of {}: {name}", self.count)
            }
        };
        (text, self.progress)
    }
}

impl Editor {
    pub(crate) fn start_transcribing(
        &mut self,
        sources: Vec<SourceId>,
        (model, language): (Model, Option<String>),
        downloaded: bool,
        captions_after: bool,
    ) {
        let mut transcribing = Transcribing {
            model,
            language,
            captions_after,
            count: sources.len(),
            queue: sources.into(),
            stage: None,
            progress: 0.0,
        };
        if downloaded {
            self.hear_next(transcribing);
            return;
        }
        match DownloadJob::start(model, &models_folder()) {
            Ok(job) => {
                transcribing.stage = Some(Stage::Downloading(job));
                self.transcribing = Some(transcribing);
            }
            Err(error) => self.notice(error.to_string()),
        }
    }

    pub(crate) fn cancel_transcribing(&mut self) {
        self.transcribing = None;
    }

    pub(crate) fn poll_transcribing(&mut self, ctx: &egui::Context) {
        let Some(transcribing) = &mut self.transcribing else {
            return;
        };
        ctx.request_repaint_after(Duration::from_millis(200));
        match &transcribing.stage {
            None => {}
            Some(Stage::Downloading(job)) => {
                for event in job.poll() {
                    match event {
                        DownloadEvent::Progress(done) => transcribing.progress = done,
                        DownloadEvent::Done => {
                            if let Some(transcribing) = self.transcribing.take() {
                                self.hear_next(transcribing);
                            }
                            return;
                        }
                        DownloadEvent::Failed(error) => {
                            self.transcribing = None;
                            self.notice(error.to_string());
                            return;
                        }
                    }
                }
            }
            Some(Stage::Hearing { source, job }) => {
                let source = *source;
                let mut heard = None;
                let mut notices = Vec::new();
                for event in job.poll() {
                    match event {
                        TranscribeEvent::Progress(done) => transcribing.progress = done,
                        TranscribeEvent::Warning(warning) => {
                            notices.push(format!("Heard as silence: {warning}"));
                        }
                        TranscribeEvent::Done(transcript) => heard = Some(Ok(transcript)),
                        TranscribeEvent::Failed(error) => heard = Some(Err(error)),
                    }
                }
                for notice in notices {
                    self.notice(notice);
                }
                match heard {
                    Some(Ok(transcript)) => {
                        let transcript = Some(Arc::new(transcript));
                        self.apply_levels(SetTranscript { source, transcript });
                        if let Some(transcribing) = self.transcribing.take() {
                            self.hear_next(transcribing);
                        }
                    }
                    Some(Err(error)) => {
                        self.transcribing = None;
                        self.notice(error.to_string());
                    }
                    None => {}
                }
            }
        }
    }

    /// Starts on the next media in the queue, or finishes.
    fn hear_next(&mut self, mut transcribing: Transcribing) {
        while let Some(source) = transcribing.queue.pop_front() {
            let Some(piece) = playback::media_alone(self.timeline.sequence(), source) else {
                continue;
            };
            let model = transcribing.model.path(&models_folder());
            let language = if transcribing.model.english_only() {
                Some("en".to_owned())
            } else {
                transcribing.language.clone()
            };
            match TranscribeJob::start(piece, model, language) {
                Ok(job) => {
                    transcribing.stage = Some(Stage::Hearing { source, job });
                    transcribing.progress = 0.0;
                    self.transcribing = Some(transcribing);
                    return;
                }
                Err(error) => {
                    self.notice(error.to_string());
                    return;
                }
            }
        }
        if transcribing.captions_after {
            if self.timeline.sequence().captions.items.is_empty() {
                self.create_captions();
            } else {
                self.notice(
                    "Transcribed. The sequence already has captions; Create Captions in the \
                     Text panel replaces them",
                );
            }
        } else {
            self.notice("Transcribed");
        }
    }
}
