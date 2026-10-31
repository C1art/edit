use egui::{Key, RichText};
use timeline::{Attribute, Clip, PasteAttributes, RemoveAttributes, TrackKind};

use crate::editor::Editor;

/// The attributes Paste Attributes or Remove Attributes is to work on.
pub(crate) struct AttributesDialog {
    action: Action,
    chosen: Vec<Attribute>,
}

enum Action {
    /// From the first copied clip of each kind.
    Paste {
        video: Option<Box<Clip>>,
        audio: Option<Box<Clip>>,
    },
    Remove,
}

impl Action {
    fn offers(&self, attribute: Attribute) -> bool {
        match self {
            Self::Paste { video, audio } => match attribute.kind() {
                TrackKind::Video => video.is_some(),
                TrackKind::Audio => audio.is_some(),
            },
            Self::Remove => true,
        }
    }
}

impl AttributesDialog {
    fn new(action: Action) -> Self {
        let chosen = Attribute::ALL
            .into_iter()
            .filter(|&attribute| action.offers(attribute))
            .collect();
        Self { action, chosen }
    }
}

impl Editor {
    pub(crate) fn paste_attributes(&mut self) {
        if self.view.selection.is_empty() {
            self.notice("Select clips to paste attributes onto");
            return;
        }
        let Some(clipboard) = &self.clipboard else {
            self.notice("Copy a clip to paste its attributes");
            return;
        };
        let action = Action::Paste {
            video: clipboard
                .first_clip(TrackKind::Video)
                .cloned()
                .map(Box::new),
            audio: clipboard
                .first_clip(TrackKind::Audio)
                .cloned()
                .map(Box::new),
        };
        self.attributes_dialog = Some(AttributesDialog::new(action));
    }

    pub(crate) fn remove_attributes(&mut self) {
        if self.view.selection.is_empty() {
            self.notice("Select clips to remove attributes from");
            return;
        }
        self.attributes_dialog = Some(AttributesDialog::new(Action::Remove));
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(dialog) = &mut editor.attributes_dialog else {
        return;
    };
    let title = match dialog.action {
        Action::Paste { .. } => "Paste Attributes",
        Action::Remove => "Remove Attributes",
    };
    let mut confirmed = false;
    let mut cancelled = false;
    crate::dialog::window(ctx, title).show(ctx, |ui| {
        for (kind, heading) in [
            (TrackKind::Video, "Video Attributes"),
            (TrackKind::Audio, "Audio Attributes"),
        ] {
            ui.label(RichText::new(heading).strong());
            for attribute in Attribute::ALL.into_iter().filter(|a| a.kind() == kind) {
                let mut on = dialog.chosen.contains(&attribute);
                let offered = dialog.action.offers(attribute);
                let checkbox = egui::Checkbox::new(&mut on, attribute.name());
                if ui.add_enabled(offered, checkbox).changed() {
                    if on {
                        dialog.chosen.push(attribute);
                    } else {
                        dialog.chosen.retain(|&chosen| chosen != attribute);
                    }
                }
            }
        }
        crate::dialog::footer(ui, |ui| {
            cancelled =
                ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape));
            confirmed = ui.add(crate::dialog::primary("OK")).clicked()
                || ui.input(|input| input.key_pressed(Key::Enter));
        });
    });
    if cancelled {
        editor.attributes_dialog = None;
    }
    if !confirmed {
        return;
    }
    let Some(dialog) = editor.attributes_dialog.take() else {
        return;
    };
    let clips = editor.view.selection.clone();
    let attributes = dialog.chosen;
    let applied = match dialog.action {
        Action::Paste { video, audio } => editor.apply(PasteAttributes {
            video: video.map(|clip| *clip),
            audio: audio.map(|clip| *clip),
            clips,
            attributes,
        }),
        Action::Remove => editor.apply(RemoveAttributes { clips, attributes }),
    };
    if !applied {
        editor.notice("The selected clips already have those attributes");
    }
}
