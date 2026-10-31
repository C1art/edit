//! Simplify Sequence: what to tidy away from the open sequence, none of it changing what
//! the sequence plays.

use egui::Key;
use timeline::SimplifySequence;

use crate::editor::Editor;

impl Editor {
    pub(crate) fn open_simplify(&mut self) {
        if !self.timeline.sequence().has_clips() {
            self.notice("The sequence has no clips to simplify");
            return;
        }
        self.simplify_dialog = Some(SimplifySequence {
            remove_disabled: true,
            remove_on_tracks_off: false,
            lower_clips: true,
            delete_empty_tracks: true,
        });
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(settings) = &mut editor.simplify_dialog else {
        return;
    };
    let mut done = None;
    crate::dialog::window(ctx, "Simplify Sequence").show(ctx, |ui| {
        ui.checkbox(&mut settings.remove_disabled, "Remove disabled clips");
        ui.checkbox(
            &mut settings.remove_on_tracks_off,
            "Remove clips on tracks turned off",
        )
        .on_hover_text("Hidden video tracks and silenced audio tracks");
        ui.checkbox(
            &mut settings.lower_clips,
            "Move clips to the lowest track they fit on",
        )
        .on_hover_text(
            "Only past free tracks that play alike, so the picture and mix stay the same",
        );
        ui.checkbox(&mut settings.delete_empty_tracks, "Delete empty tracks");
        ui.weak("Locked tracks are left as they are. Undo restores the sequence.");
        crate::dialog::footer(ui, |ui| {
            if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                done = Some(false);
            }
            if ui.add(crate::dialog::primary("Simplify")).clicked()
                || ui.input(|input| input.key_pressed(Key::Enter))
            {
                done = Some(true);
            }
        });
    });
    let Some(simplify) = done else {
        return;
    };
    let Some(settings) = editor.simplify_dialog.take() else {
        return;
    };
    if simplify && !editor.apply(settings) {
        editor.notice("The sequence is already as simple as those choices make it");
    }
}
