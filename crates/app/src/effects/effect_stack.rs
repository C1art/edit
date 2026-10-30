//! The clip's effects in Effect Controls, each with its own masks, as Premiere shows
//! them under every effect.

use egui::RichText;
use timeline::{Look, MaskOwner};

use super::look::Rows;

/// Edits `look`'s effects in `rows`, of a clip among `video_tracks` video tracks.
pub(super) fn controls(ui: &mut egui::Ui, rows: &mut Rows, look: &mut Look, video_tracks: usize) {
    let mut removed = None;
    for index in 0..look.effects.len() {
        ui.push_id(index, |ui| {
            let Some(effect) = look.effects.get_mut(index) else {
                return;
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(effect.name()).strong());
                if ui.small_button("Remove").clicked() {
                    removed = Some(index);
                }
            });
            rows.effect(ui, effect, video_tracks);
            if std::mem::take(&mut rows.browse_matte) {
                rows.output.choose_matte = Some(index);
            }
            let owner = MaskOwner::Effect(index);
            if rows.simple && look.masks_of(owner).is_empty() {
                return;
            }
            if let Some(masks) = look.masks_of_mut(owner) {
                super::masks::controls(ui, rows, owner, masks);
            }
        });
    }
    look.tidy_effect_masks();
    if let Some(index) = removed {
        look.remove_effect(index);
        rows.commit();
    }
    super::video_effects::add_menu(ui, rows, &mut look.effects);
}
