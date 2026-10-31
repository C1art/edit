use std::time::Instant;

use egui::{Color32, ComboBox, Context, Key, Sense, Ui, Vec2};

use super::{Phase, TrackChoice};
use crate::editor::Editor;

/// The quietest level the meter shows, in dB below full scale.
const METER_FLOOR_DB: f32 = -60.0;
const METER_SIZE: Vec2 = Vec2::new(260.0, 10.0);

pub(crate) fn dialog(editor: &mut Editor, ctx: &Context) {
    if editor.voiceover.take.is_some() {
        editor.tend_voiceover();
        take_window(editor, ctx);
        if editor.voiceover.take.is_some() {
            // The time and the meter move on their own.
            ctx.request_repaint();
        }
    } else if let Some(track) = editor.voiceover.setup {
        setup_window(editor, track, ctx);
    }
}

enum Setup {
    Cancel,
    Record,
}

fn setup_window(editor: &mut Editor, mut track: TrackChoice, ctx: &Context) {
    let audio_tracks: Vec<String> = editor
        .timeline
        .sequence()
        .audio
        .iter()
        .enumerate()
        .map(|(index, track)| match &track.name {
            Some(name) => format!("A{} {name}", index + 1),
            None => format!("A{}", index + 1),
        })
        .collect();
    let voiceover = &mut editor.voiceover;
    let devices = voiceover.devices().to_vec();
    let mut answer = None;
    crate::dialog::window(ctx, "Record Voiceover").show(ctx, |ui| {
        egui::Grid::new("voiceover setup")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Microphone");
                let shown = voiceover
                    .input
                    .as_ref()
                    .map_or("System Default", |device| device.name.as_str());
                ComboBox::from_id_salt("voiceover input")
                    .selected_text(shown)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(voiceover.input.is_none(), "System Default")
                            .clicked()
                        {
                            voiceover.input = None;
                        }
                        for device in devices {
                            let chosen = voiceover.input.as_ref().is_some_and(|c| c.is(&device));
                            if ui.selectable_label(chosen, &device.name).clicked() {
                                voiceover.input = Some(device);
                            }
                        }
                    });
                ui.end_row();
                ui.label("Record onto");
                let shown = match track {
                    TrackChoice::Existing(index) => {
                        audio_tracks.get(index).map_or("New Track", String::as_str)
                    }
                    TrackChoice::New => "New Track",
                };
                ComboBox::from_id_salt("voiceover track")
                    .selected_text(shown)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (index, name) in audio_tracks.iter().enumerate() {
                            ui.selectable_value(&mut track, TrackChoice::Existing(index), name);
                        }
                        ui.selectable_value(&mut track, TrackChoice::New, "New Track");
                    });
                ui.end_row();
            });
        ui.checkbox(&mut voiceover.countdown, "Count down 3 seconds first");
        ui.checkbox(
            &mut voiceover.hear_sequence,
            "Hear the sequence while recording (use headphones)",
        );
        ui.weak("Recording starts at the playhead and plays the sequence along with it.");
        crate::dialog::footer(ui, |ui| {
            if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                answer = Some(Setup::Cancel);
            }
            if ui.add(crate::dialog::primary("Record")).clicked()
                || ui.input(|input| input.key_pressed(Key::Enter))
            {
                answer = Some(Setup::Record);
            }
        });
    });
    match answer {
        None => editor.voiceover.setup = Some(track),
        Some(Setup::Cancel) => editor.voiceover.setup = None,
        Some(Setup::Record) => {
            editor.voiceover.setup = None;
            editor.start_voiceover(track, ctx);
        }
    }
}

enum Action {
    Stop,
    Discard,
}

fn take_window(editor: &mut Editor, ctx: &Context) {
    let Some(take) = &editor.voiceover.take else {
        return;
    };
    let mut action = None;
    crate::dialog::window(ctx, "Record Voiceover").show(ctx, |ui| {
        let status = match take.phase {
            Phase::Opening => "Opening the microphone…".to_owned(),
            Phase::Counting { until } => {
                let left = until.saturating_duration_since(Instant::now());
                format!("Recording in {}…", left.as_secs() + 1)
            }
            Phase::Recording { since, .. } => {
                let seconds = since.elapsed().as_secs();
                format!("● Recording  {}:{:02}", seconds / 60, seconds % 60)
            }
            Phase::Saving { .. } => "Saving the voiceover…".to_owned(),
        };
        ui.heading(status);
        meter(ui, take.meter);
        let escape = ui.input(|input| input.key_pressed(Key::Escape));
        crate::dialog::footer(ui, |ui| match take.phase {
            Phase::Recording { .. } => {
                if ui.button("Discard").clicked() || escape {
                    action = Some(Action::Discard);
                }
                let stop = ui.add(crate::dialog::primary("Stop")).clicked()
                    || ui.input(|input| {
                        input.key_pressed(Key::Space) || input.key_pressed(Key::Enter)
                    });
                if stop {
                    action = Some(Action::Stop);
                }
            }
            Phase::Opening | Phase::Counting { .. } => {
                if ui.button("Cancel").clicked() || escape {
                    action = Some(Action::Discard);
                }
            }
            Phase::Saving { .. } => {}
        });
    });
    match action {
        Some(Action::Stop) => editor.stop_take(),
        Some(Action::Discard) => editor.discard_take(),
        None => {}
    }
}

/// How loud the microphone is, on a dB scale so speech sits mid-way; red near clipping.
fn meter(ui: &mut Ui, peak: f32) {
    let (rect, _) = ui.allocate_exact_size(METER_SIZE, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    let db = 20.0 * peak.max(1e-6).log10();
    let fill = ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0);
    let color = match db {
        db if db > -3.0 => Color32::from_rgb(220, 60, 50),
        db if db > -12.0 => Color32::from_rgb(220, 190, 60),
        _ => Color32::from_rgb(70, 190, 100),
    };
    let mut level = rect;
    level.set_width(rect.width() * fill);
    painter.rect_filled(level, 2.0, color);
}
