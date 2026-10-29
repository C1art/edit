use egui::{Align2, Color32, FontId, Rect, Sense, Stroke};

use crate::editor::Editor;
use crate::theme;

pub(crate) const WIDTH: f32 = 76.0;
/// The quietest level shown, in dB.
const FLOOR: f32 = -60.0;
/// How fast the bars fall back, in dB per second.
const FALL: f32 = 24.0;
/// How long a peak mark stays put before it falls, in seconds.
const PEAK_HOLD: f32 = 1.5;
/// Levels above this read as getting loud.
const LOUD: f32 = -12.0;
const CLIP_LIGHT: f32 = 8.0;
const BAR_WIDTH: f32 = 10.0;
const QUIET_FILL: Color32 = Color32::from_rgb(70, 200, 90);
const LOUD_FILL: Color32 = Color32::from_rgb(230, 200, 60);
const CLIPPED_FILL: Color32 = Color32::from_rgb(230, 60, 50);

/// The master level of whichever monitor is playing, as Premiere's Audio Meters show it
/// beside the timeline.
#[derive(Debug)]
pub(crate) struct MasterMeter {
    levels: [f32; 2],
    peaks: [f32; 2],
    /// Seconds each peak mark has stayed put.
    held: [f32; 2],
    /// Lit once a side goes over 0 dB, until clicked.
    clipped: [bool; 2],
}

impl Default for MasterMeter {
    fn default() -> Self {
        Self {
            levels: [FLOOR; 2],
            peaks: [FLOOR; 2],
            held: [0.0; 2],
            clipped: [false; 2],
        }
    }
}

impl MasterMeter {
    fn update(&mut self, heard: [f32; 2], dt: f32) {
        let fall = FALL * dt;
        for (side, &reading) in heard.iter().enumerate() {
            let db = to_db(reading);
            self.levels[side] = db.max(self.levels[side] - fall).max(FLOOR);
            if db >= self.peaks[side] {
                self.peaks[side] = db;
                self.held[side] = 0.0;
            } else if self.held[side] < PEAK_HOLD {
                self.held[side] += dt;
            } else {
                self.peaks[side] = (self.peaks[side] - fall).max(FLOOR);
            }
            self.clipped[side] |= db > 0.0;
        }
    }

    fn moving(&self) -> bool {
        self.levels.iter().chain(&self.peaks).any(|&db| db > FLOOR)
    }
}

fn to_db(peak: f32) -> f32 {
    if peak <= 0.0 {
        FLOOR
    } else {
        20.0 * peak.log10()
    }
}

pub(crate) fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    let players = [
        editor.source.as_ref().map(|source| &source.viewer.player),
        editor.program.as_ref().map(|program| &program.player),
    ];
    let heard = players
        .into_iter()
        .flatten()
        .find_map(|player| player.levels())
        .map_or([0.0; 2], |levels| levels.master);
    let dt = ui.input(|input| input.stable_dt).min(0.1);
    let meter = &mut editor.master_meter;
    meter.update(heard, dt);
    if meter.moving() {
        ui.ctx().request_repaint();
    }

    let rect = ui
        .available_rect_before_wrap()
        .shrink2(egui::vec2(4.0, 8.0));
    let bars_right = rect.right();
    let bars_left = bars_right - 2.0 * BAR_WIDTH - 2.0;
    let lights =
        Rect::from_x_y_ranges(bars_left..=bars_right, rect.top()..=rect.top() + CLIP_LIGHT);
    let scale = Rect::from_x_y_ranges(
        rect.left()..=bars_right,
        lights.bottom() + 6.0..=rect.bottom() - 14.0,
    );
    let painter = ui.painter_at(ui.max_rect());
    let y_of = |db: f32| scale.bottom() - scale.height() * ((db - FLOOR) / -FLOOR).clamp(0.0, 1.0);

    let lights_response = ui
        .interact(lights, ui.scope_id().with("clip lights"), Sense::click())
        .on_hover_text("Lights when the sound clips; click to reset");
    if lights_response.clicked() {
        meter.clipped = [false; 2];
    }
    for side in 0..2 {
        let left = bars_left + side as f32 * (BAR_WIDTH + 2.0);
        let x_range = left..=left + BAR_WIDTH;
        let light = Rect::from_x_y_ranges(x_range.clone(), lights.y_range());
        let fill = if meter.clipped[side] {
            CLIPPED_FILL
        } else {
            Color32::from_gray(45)
        };
        painter.rect_filled(light, 1.0, fill);

        let bar = Rect::from_x_y_ranges(x_range.clone(), scale.y_range());
        painter.rect_filled(bar, 1.0, Color32::from_gray(20));
        let level = meter.levels[side].min(0.0);
        for (from, to, color) in [(FLOOR, LOUD, QUIET_FILL), (LOUD, 0.0, LOUD_FILL)] {
            if level > from {
                let lit = Rect::from_x_y_ranges(x_range.clone(), y_of(level.min(to))..=y_of(from));
                painter.rect_filled(lit, 0.0, color);
            }
        }
        let peak = meter.peaks[side];
        if peak > FLOOR {
            let color = if peak > LOUD { LOUD_FILL } else { QUIET_FILL };
            painter.hline(x_range, y_of(peak.min(0.0)), Stroke::new(2.0, color));
        }
    }

    let font = FontId::proportional(10.0);
    for db in (0..=54).step_by(6) {
        let y = y_of(-(db as f32));
        painter.hline(
            bars_left - 4.0..=bars_left - 1.0,
            y,
            (1.0, theme::WEAK_TEXT),
        );
        let label = if db == 0 {
            "0".to_owned()
        } else {
            format!("-{db}")
        };
        painter.text(
            egui::pos2(bars_left - 6.0, y),
            Align2::RIGHT_CENTER,
            label,
            font.clone(),
            theme::WEAK_TEXT,
        );
    }
    for (side, name) in ["L", "R"].into_iter().enumerate() {
        let x = bars_left + side as f32 * (BAR_WIDTH + 2.0) + BAR_WIDTH / 2.0;
        painter.text(
            egui::pos2(x, rect.bottom()),
            Align2::CENTER_BOTTOM,
            name,
            font.clone(),
            theme::WEAK_TEXT,
        );
    }
    painter.text(
        egui::pos2(bars_left - 6.0, lights.center().y),
        Align2::RIGHT_CENTER,
        "dB",
        font,
        theme::WEAK_TEXT,
    );
}
