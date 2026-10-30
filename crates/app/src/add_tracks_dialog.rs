//! Add Tracks: several video and audio tracks at once, below, above or among those
//! there are.

use egui::{DragValue, Key};
use timeline::{AddTracks, NewTracks, TrackKind};

use crate::editor::Editor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement {
    BeforeFirst,
    AfterTarget,
    AfterLast,
}

impl Placement {
    const ALL: [Self; 3] = [Self::BeforeFirst, Self::AfterTarget, Self::AfterLast];

    fn name(self) -> &'static str {
        match self {
            Self::BeforeFirst => "Before First Track",
            Self::AfterTarget => "After Target Track",
            Self::AfterLast => "After Last Track",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Kind {
    count: usize,
    placement: Placement,
}

pub(crate) struct AddTracksDialog {
    video: Kind,
    audio: Kind,
}

/// More than any sequence needs at once, so a slip of the drag can't add hundreds.
const MOST_AT_ONCE: usize = 99;

impl Editor {
    pub(crate) fn open_add_tracks(&mut self) {
        let one = Kind {
            count: 1,
            placement: Placement::AfterLast,
        };
        self.add_tracks_dialog = Some(AddTracksDialog {
            video: one,
            audio: one,
        });
    }

    fn add_tracks(&mut self, settings: &AddTracksDialog) {
        let video = self.new_tracks(TrackKind::Video, settings.video);
        let audio = self.new_tracks(TrackKind::Audio, settings.audio);
        if !self.apply(AddTracks { video, audio }) {
            return;
        }
        // Targets stay on the tracks they were on, which may have moved up.
        let targets = &mut self.view.targets;
        for (target, new) in [(&mut targets.video, video), (&mut targets.audio, audio)] {
            if let Some(index) = target.as_mut().filter(|index| **index >= new.at) {
                *index += new.count;
            }
        }
    }

    fn new_tracks(&self, kind: TrackKind, Kind { count, placement }: Kind) -> NewTracks {
        let len = self.timeline.sequence().tracks_of(kind).len();
        let target = match kind {
            TrackKind::Video => self.view.targets.video,
            TrackKind::Audio => self.view.targets.audio,
        };
        let at = match placement {
            Placement::BeforeFirst => 0,
            Placement::AfterTarget => target.map_or(len, |index| index + 1),
            Placement::AfterLast => len,
        };
        NewTracks { at, count }
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(settings) = &mut editor.add_tracks_dialog else {
        return;
    };
    let mut done = None;
    crate::dialog::window(ctx, "Add Tracks").show(ctx, |ui| {
        egui::Grid::new("add_tracks")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                kind_rows(ui, "video", &mut settings.video);
                kind_rows(ui, "audio", &mut settings.audio);
            });
        crate::dialog::footer(ui, |ui| {
            if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                done = Some(false);
            }
            if ui.add(crate::dialog::primary("OK")).clicked()
                || ui.input(|input| input.key_pressed(Key::Enter))
            {
                done = Some(true);
            }
        });
    });
    let Some(add) = done else {
        return;
    };
    let Some(settings) = editor.add_tracks_dialog.take() else {
        return;
    };
    if add {
        editor.add_tracks(&settings);
    }
}

fn kind_rows(ui: &mut egui::Ui, noun: &str, kind: &mut Kind) {
    ui.label("Add");
    ui.horizontal(|ui| {
        ui.add(DragValue::new(&mut kind.count).range(0..=MOST_AT_ONCE));
        ui.label(format!("{noun} track(s)"));
    });
    ui.end_row();
    ui.label("Placement");
    egui::ComboBox::from_id_salt(noun)
        .selected_text(kind.placement.name())
        .show_ui(ui, |ui| {
            for placement in Placement::ALL {
                ui.selectable_value(&mut kind.placement, placement, placement.name());
            }
        });
    ui.end_row();
}
