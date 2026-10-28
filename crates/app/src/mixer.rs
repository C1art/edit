use egui::{Color32, Rect, RichText, Sense, Slider};
use timeline::{
    preview, Edit, Gain, Pan, SetMasterGain, SetTrackMix, SetTrackSwitch, TrackRef, TrackSwitch,
};

use crate::editor::Editor;

/// Fader travel shown, in dB; the bottom stop means silence.
const FADER_RANGE: std::ops::RangeInclusive<f32> = -60.0..=15.0;
/// Meter scale floor in dB.
const METER_FLOOR: f32 = -60.0;
/// How fast meters fall back, in dB per second.
const METER_FALL: f32 = 24.0;
const STRIP_HEIGHT: f32 = 180.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Channel {
    Track(TrackRef),
    Master,
}

/// A fader or knob being moved: heard live, committed to the undo stack on release.
#[derive(Debug, Clone, Copy)]
struct Adjusting {
    channel: Channel,
    volume: Gain,
    pan: Pan,
}

/// Meter ballistics and the control being dragged.
#[derive(Debug, Default)]
pub struct Mixer {
    /// Displayed meter levels in dB: one pair per audio track, then the master.
    meters: Vec<[f32; 2]>,
    adjusting: Option<Adjusting>,
}

impl Mixer {
    fn update_meters(&mut self, editor_levels: Option<&playback::Levels>, tracks: usize, dt: f32) {
        self.meters.resize(tracks + 1, [METER_FLOOR; 2]);
        let fall = METER_FALL * dt;
        let readings = editor_levels.map(|levels| {
            levels
                .tracks
                .iter()
                .copied()
                .chain(std::iter::once(levels.master))
                .collect::<Vec<_>>()
        });
        for (index, meter) in self.meters.iter_mut().enumerate() {
            let reading = readings
                .as_ref()
                .and_then(|readings| readings.get(index).copied())
                .unwrap_or([0.0; 2]);
            for side in 0..2 {
                let db = to_db(reading[side]);
                meter[side] = db.max(meter[side] - fall).max(METER_FLOOR);
            }
        }
    }

    fn falling(&self) -> bool {
        self.meters
            .iter()
            .any(|meter| meter.iter().any(|&db| db > METER_FLOOR))
    }
}

fn to_db(peak: f32) -> f32 {
    if peak <= 0.0 {
        METER_FLOOR
    } else {
        20.0 * peak.log10()
    }
}

pub fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    let dt = ui.input(|input| input.stable_dt).min(0.1);
    let tracks = editor.timeline.sequence().audio.len();
    let levels = editor
        .program
        .as_ref()
        .and_then(|program| program.player.levels());
    editor.mixer.update_meters(levels, tracks, dt);
    if editor.mixer.falling() {
        ui.ctx().request_repaint();
    }
    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for index in 0..tracks {
                strip(editor, ui, Channel::Track(TrackRef::audio(index)));
                ui.add_space(6.0);
            }
            strip(editor, ui, Channel::Master);
        });
    });
}

fn strip(editor: &mut Editor, ui: &mut egui::Ui, channel: Channel) {
    let sequence = editor.timeline.sequence();
    let (name, mut volume, mut pan, switches, meter) = match channel {
        Channel::Track(track_ref) => {
            let Some(track) = sequence.track(track_ref) else {
                return;
            };
            let meter = editor.mixer.meters.get(track_ref.index).copied();
            (
                track_ref.to_string(),
                track.volume,
                track.pan,
                Some([
                    ("Mute", TrackSwitch::Mute, track.muted),
                    ("Solo", TrackSwitch::Solo, track.solo),
                ]),
                meter,
            )
        }
        Channel::Master => (
            "Master".to_owned(),
            sequence.master,
            Pan::CENTER,
            None,
            editor.mixer.meters.last().copied(),
        ),
    };
    if let Some(adjusting) = editor.mixer.adjusting.filter(|a| a.channel == channel) {
        volume = adjusting.volume;
        pan = adjusting.pan;
    }
    let mut changed = None;
    ui.vertical(|ui| {
        ui.set_width(60.0);
        ui.label(RichText::new(&name).strong());
        ui.horizontal(|ui| {
            ui.spacing_mut().slider_width = STRIP_HEIGHT;
            meter_bar(ui, meter.unwrap_or([METER_FLOOR; 2]));
            let mut db = volume.db().max(*FADER_RANGE.start());
            let fader = ui.add(
                Slider::new(&mut db, FADER_RANGE)
                    .vertical()
                    .show_value(false)
                    .step_by(0.1),
            );
            let fader = fader.on_hover_text("Double-click for 0 dB");
            if fader.double_clicked() {
                db = 0.0;
            }
            if fader.changed() || fader.double_clicked() {
                volume = if db <= *FADER_RANGE.start() {
                    Gain::SILENT
                } else {
                    Gain::from_db(db)
                };
            }
            changed = response_state(&fader, changed);
        });
        let label = if volume <= Gain::SILENT {
            "-inf".to_owned()
        } else {
            format!("{:+.1}", volume.db())
        };
        ui.monospace(label).on_hover_text("dB");
        if let Channel::Track(track) = channel {
            let mut value = pan.value();
            ui.spacing_mut().slider_width = 56.0;
            let knob = ui
                .add(Slider::new(&mut value, -100..=100).show_value(false))
                .on_hover_text(format!("Balance {value} (double-click to centre)"));
            if knob.double_clicked() {
                value = 0;
            }
            pan = Pan::new(value);
            changed = response_state(&knob, changed);
            let mut switched = None;
            ui.horizontal(|ui| {
                for (label, switch, on) in switches.into_iter().flatten() {
                    if ui.selectable_label(on, label).clicked() {
                        switched = Some((switch, on));
                    }
                }
            });
            if let Some((switch, on)) = switched {
                editor.apply(SetTrackSwitch {
                    track,
                    switch,
                    on: !on,
                });
            }
        }
    });
    match changed {
        Some(Change::Live) => {
            editor.mixer.adjusting = Some(Adjusting {
                channel,
                volume,
                pan,
            });
            match channel {
                Channel::Track(track) => live(editor, &SetTrackMix { track, volume, pan }),
                Channel::Master => live(editor, &SetMasterGain { gain: volume }),
            }
        }
        Some(Change::Commit) => {
            editor.mixer.adjusting = None;
            match channel {
                Channel::Track(track) => editor.apply_levels(SetTrackMix { track, volume, pan }),
                Channel::Master => editor.apply_levels(SetMasterGain { gain: volume }),
            }
        }
        None => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Change {
    Live,
    Commit,
}

/// A drag in progress is heard live; a release, click or double-click commits.
pub(crate) fn response_state(
    response: &egui::Response,
    previous: Option<Change>,
) -> Option<Change> {
    let state = if response.drag_stopped() || response.double_clicked() {
        Some(Change::Commit)
    } else if response.changed() && response.dragged() {
        Some(Change::Live)
    } else if response.changed() {
        Some(Change::Commit)
    } else {
        None
    };
    state.max(previous)
}

fn live(editor: &mut Editor, edit: &impl Edit) {
    let Ok(sequence) = preview(editor.timeline.sequence(), edit) else {
        return;
    };
    editor.mix_program(sequence);
}

fn meter_bar(ui: &mut egui::Ui, levels: [f32; 2]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, STRIP_HEIGHT), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 1.0, Color32::from_gray(20));
    let range = -METER_FLOOR + *FADER_RANGE.end();
    for (side, db) in levels.into_iter().enumerate() {
        let fraction = ((db - METER_FLOOR) / range).clamp(0.0, 1.0);
        let left = rect.left() + 1.0 + side as f32 * 6.5;
        let column = Rect::from_x_y_ranges(
            left..=left + 5.5,
            rect.bottom() - rect.height() * fraction..=rect.bottom(),
        );
        let color = if db > 0.0 {
            Color32::from_rgb(230, 60, 50)
        } else if db > -12.0 {
            Color32::from_rgb(230, 200, 60)
        } else {
            Color32::from_rgb(70, 200, 90)
        };
        painter.rect_filled(column, 0.0, color);
    }
    let zero = rect.bottom() - rect.height() * (-METER_FLOOR / range);
    painter.hline(rect.x_range(), zero, (1.0, Color32::from_gray(140)));
}
