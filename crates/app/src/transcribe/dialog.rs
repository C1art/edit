//! Choosing what to transcribe, with which model and in which language.

use crossbeam::channel::{Receiver, TryRecvError};
use egui::{ComboBox, Key};
use speech::Model;
use timeline::{SourceId, TrackKind};

use super::models_folder;
use crate::editor::Editor;

pub(crate) struct TranscribeDialog {
    /// Media heard in the sequence without a transcript yet.
    fresh: Vec<SourceId>,
    /// Media heard in the sequence that was transcribed before.
    done: Vec<SourceId>,
    again: bool,
    model: Model,
    /// A language code, or none for the model to tell.
    language: Option<String>,
    languages: Vec<(&'static str, &'static str)>,
    captions_after: bool,
    downloaded: Downloaded,
}

enum Downloaded {
    Looking(Receiver<Vec<Model>>),
    Found(Vec<Model>),
}

impl Editor {
    /// Offers to transcribe the media heard in the sequence.
    pub(crate) fn open_transcribe(&mut self) {
        if self.transcribing.is_some() {
            self.notice("Already transcribing");
            return;
        }
        let sequence = self.timeline.sequence();
        let mut heard: Vec<SourceId> = sequence
            .tracks()
            .filter(|(track, _)| track.kind == TrackKind::Audio)
            .flat_map(|(_, track)| &track.clips)
            .map(|clip| clip.source)
            .collect();
        heard.sort_unstable_by_key(|source| source.0);
        heard.dedup();
        let (done, fresh): (Vec<SourceId>, Vec<SourceId>) = heard
            .into_iter()
            .filter(|&source| {
                sequence
                    .media_item(source)
                    .is_some_and(|item| item.has_audio && item.is_file() && !item.offline)
            })
            .partition(|&source| {
                sequence
                    .media_item(source)
                    .is_some_and(|item| item.transcript.is_some())
            });
        if fresh.is_empty() && done.is_empty() {
            self.notice("There's no sound in the sequence to transcribe");
            return;
        }
        let (model, language) = self.speech_choice.clone();
        self.transcribe_dialog = Some(TranscribeDialog {
            again: fresh.is_empty(),
            fresh,
            done,
            model,
            language,
            languages: speech::languages(),
            captions_after: self.timeline.sequence().captions.items.is_empty(),
            downloaded: Downloaded::Looking(speech::downloaded(models_folder())),
        });
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(dialog) = &mut editor.transcribe_dialog else {
        return;
    };
    if let Downloaded::Looking(found) = &dialog.downloaded {
        match found.try_recv() {
            Ok(models) => dialog.downloaded = Downloaded::Found(models),
            Err(TryRecvError::Disconnected) => dialog.downloaded = Downloaded::Found(Vec::new()),
            Err(TryRecvError::Empty) => ctx.request_repaint(),
        }
    }
    let mut start = false;
    let mut cancelled = false;
    crate::dialog::window(ctx, "Transcribe Sequence").show(ctx, |ui| {
        settings(ui, dialog);
        let sources = dialog.fresh.len() + if dialog.again { dialog.done.len() } else { 0 };
        let ready = sources > 0 && matches!(dialog.downloaded, Downloaded::Found(_));
        crate::dialog::footer(ui, |ui| {
            cancelled =
                ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape));
            let action = if dialog.is_downloaded() {
                "Transcribe"
            } else {
                "Download and Transcribe"
            };
            start = ui
                .add_enabled(ready, crate::dialog::primary(action))
                .clicked()
                || (ready && ui.input(|input| input.key_pressed(Key::Enter)));
        });
    });
    if cancelled {
        editor.transcribe_dialog = None;
    } else if start {
        if let Some(dialog) = editor.transcribe_dialog.take() {
            let downloaded = dialog.is_downloaded();
            let mut sources = dialog.fresh;
            if dialog.again {
                sources.extend(dialog.done);
            }
            let choice = (dialog.model, dialog.language);
            editor.speech_choice.clone_from(&choice);
            editor.start_transcribing(sources, choice, downloaded, dialog.captions_after);
        }
    }
}

impl TranscribeDialog {
    fn is_downloaded(&self) -> bool {
        matches!(&self.downloaded, Downloaded::Found(models) if models.contains(&self.model))
    }
}

fn settings(ui: &mut egui::Ui, dialog: &mut TranscribeDialog) {
    let count = |sources: &[SourceId]| match sources.len() {
        1 => "1 file".to_owned(),
        count => format!("{count} files"),
    };
    if !dialog.fresh.is_empty() {
        ui.label(format!(
            "Listens to the {} heard in the sequence, on this computer.",
            count(&dialog.fresh)
        ));
    }
    if !dialog.done.is_empty() {
        ui.checkbox(
            &mut dialog.again,
            format!("Transcribe the {} already done again", count(&dialog.done)),
        );
    }
    ui.add_space(4.0);
    egui::Grid::new("transcribe")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label("Model");
            let downloaded = match &dialog.downloaded {
                Downloaded::Found(models) => models.as_slice(),
                Downloaded::Looking(_) => &[],
            };
            let describe = |model: Model| {
                if downloaded.contains(&model) {
                    model.label().to_owned()
                } else {
                    let megabytes = model.approximate_bytes() / 1_000_000;
                    format!("{} ({megabytes} MB download)", model.label())
                }
            };
            ComboBox::from_id_salt("transcribe model")
                .selected_text(describe(dialog.model))
                .width(260.0)
                .show_ui(ui, |ui| {
                    for model in Model::ALL {
                        ui.selectable_value(&mut dialog.model, model, describe(model));
                    }
                });
            ui.end_row();
            ui.label("Language");
            if dialog.model.english_only() {
                ui.label("English");
            } else {
                language(ui, dialog);
            }
            ui.end_row();
        });
    ui.checkbox(&mut dialog.captions_after, "Create captions when done");
}

fn language(ui: &mut egui::Ui, dialog: &mut TranscribeDialog) {
    let name = |code: Option<&str>| match code {
        None => "Detect automatically".to_owned(),
        Some(code) => dialog
            .languages
            .iter()
            .find(|(known, _)| *known == code)
            .map_or_else(|| code.to_owned(), |(_, name)| capitalized(name)),
    };
    let chosen = name(dialog.language.as_deref());
    let mut picked = None;
    ComboBox::from_id_salt("transcribe language")
        .selected_text(chosen)
        .width(260.0)
        .height(320.0)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(dialog.language.is_none(), name(None))
                .clicked()
            {
                picked = Some(None);
            }
            for &(code, _) in &dialog.languages {
                let current = dialog.language.as_deref() == Some(code);
                if ui.selectable_label(current, name(Some(code))).clicked() {
                    picked = Some(Some(code.to_owned()));
                }
            }
        });
    if let Some(language) = picked {
        dialog.language = language;
    }
}

fn capitalized(name: &str) -> String {
    let mut letters = name.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(letters).collect()
    })
}
