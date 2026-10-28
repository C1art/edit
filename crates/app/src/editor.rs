use egui::{CentralPanel, MenuBar, Panel, ViewportCommand};
use timeline::{Sequence, Timebase, Timeline};

pub struct Editor {
    timeline: Timeline,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            timeline: Timeline::new(Sequence::new(Timebase::HZ_90K)),
        }
    }
}

impl eframe::App for Editor {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        Panel::top("menu_bar").show(ui, |ui| {
            MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                    }
                });
            });
        });

        Panel::left("media_pool")
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| {
                ui.heading("Media");
            });

        Panel::bottom("timeline")
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| {
                ui.heading("Timeline");
                ui.label(format!("{} tracks", self.timeline.sequence().tracks.len()));
            });

        CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| ui.label("Program monitor"));
        });
    }
}
