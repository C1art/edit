use egui::TextEdit;
use timeline::{BinId, BinSearch, SearchField};

use super::{Pool, PoolAction};

impl Pool<'_> {
    /// The panel's search field, with the button that keeps its search as a search bin.
    pub(super) fn search_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let keep = ui
                .add_enabled(
                    !self.view.search.trim().is_empty(),
                    egui::Button::new("🔍+"),
                )
                .on_hover_text("New Search Bin: keep this search as a bin that stays up to date")
                .on_disabled_hover_text("Type a search to keep it as a search bin");
            if keep.clicked() {
                let search = BinSearch {
                    text: self.view.search.trim().to_owned(),
                    field: SearchField::All,
                };
                self.action = Some(PoolAction::NewSearchBin(search));
            }
            ui.add(
                TextEdit::singleline(&mut self.view.search)
                    .hint_text("Search")
                    .desired_width(f32::INFINITY),
            );
        });
    }

    /// The search bin's own menu entries: which field it looks in and what for.
    pub(super) fn search_bin_menu(&mut self, ui: &mut egui::Ui, bin: BinId, search: &BinSearch) {
        ui.label(format!("Finds \"{}\"", search.text));
        ui.menu_button(format!("In: {}", search.field.name()), |ui| {
            for field in SearchField::ALL {
                if ui
                    .selectable_label(search.field == field, field.name())
                    .clicked()
                {
                    let search = BinSearch {
                        field,
                        ..search.clone()
                    };
                    self.action = Some(PoolAction::SetBinSearch(bin, search));
                }
            }
        });
        let typed = self.view.search.trim();
        let use_typed = ui
            .add_enabled(
                !typed.is_empty(),
                egui::Button::new("Search For Panel Text"),
            )
            .on_hover_text("Finds what is typed in the panel's search field instead");
        if use_typed.clicked() {
            let search = BinSearch {
                text: typed.to_owned(),
                ..search.clone()
            };
            self.action = Some(PoolAction::SetBinSearch(bin, search));
        }
        ui.separator();
    }
}
