use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke};
use timeline::{Param, Ticks};

use super::Lane;
use crate::effects::look::{Output, ANIMATED};

const HEIGHT: f32 = 40.0;
const MARGIN: f32 = 4.0;
/// How finely the slope is measured, in parts of a column.
const SAMPLES_PER_COLUMN: i64 = 8;

impl Lane {
    /// Premiere's velocity graph: how fast `param` changes at each moment of the clip, per
    /// second of the sequence, so eases show as rising and falling hills.
    pub(in crate::effects) fn show_velocity(
        &self,
        ui: &mut egui::Ui,
        param: &Param,
        time: Ticks,
        output: &mut Output,
    ) {
        ui.horizontal(|ui| {
            let left = self.left.max(ui.cursor().left() + 4.0);
            ui.add_space(left - ui.cursor().left());
            let size = egui::vec2(self.right - left, HEIGHT);
            let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
            if let Some(pointer) = response
                .interact_pointer_pos()
                .filter(|_| response.clicked() || response.dragged())
            {
                output.seek = Some(self.source_at(rect, pointer.x));
            }
            let speeds = self.speeds(rect, param);
            self.paint_velocity(ui, rect, &speeds, time);
        });
    }

    /// The rate of change at the left edge of each column, per sequence second, measured
    /// just before and just after and the gentler kept, so a hold keyframe's jump doesn't
    /// spike it.
    fn speeds(&self, rect: Rect, param: &Param) -> Vec<f32> {
        let columns = rect.width().max(1.0) as i64;
        let clip = &self.clip;
        let step = Ticks((clip.duration.0 / (columns * SAMPLES_PER_COLUMN)).max(1));
        let seconds =
            step.0 as f64 * f64::from(self.timebase.num()) / f64::from(self.timebase.den());
        let value_at = |position: Ticks| {
            let position = position.clamp(clip.start, clip.end() - Ticks(1));
            param.value_at(clip.source_time(position))
        };
        (0..=columns)
            .map(|column| {
                let offset = Ticks(clip.duration.0 * column / columns);
                let position = clip.start + offset;
                let at = value_at(position);
                let after = value_at(position + step) - at;
                let before = at - value_at(position - step);
                let rise = if after.abs() <= before.abs() {
                    after
                } else {
                    before
                };
                (f64::from(rise) / seconds) as f32
            })
            .collect()
    }

    fn paint_velocity(&self, ui: &egui::Ui, rect: Rect, speeds: &[f32], time: Ticks) {
        let painter = ui.painter_at(rect);
        let visuals = ui.visuals();
        painter.rect_filled(rect, 2.0, visuals.extreme_bg_color);
        let (low, high) = speeds
            .iter()
            .fold((0.0_f32, 0.0_f32), |(low, high), &speed| {
                (low.min(speed), high.max(speed))
            });
        let span = (high - low).max(f32::EPSILON);
        let y_of = |speed: f32| {
            rect.bottom() - MARGIN - (speed - low) / span * (rect.height() - 2.0 * MARGIN)
        };
        let grid = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
        painter.hline(rect.x_range(), y_of(0.0), grid);
        let font = FontId::proportional(10.0);
        let weak = visuals.weak_text_color();
        let top = Pos2::new(rect.left() + 3.0, rect.top() + 1.0);
        painter.text(
            top,
            Align2::LEFT_TOP,
            format!("{high:.1}/s"),
            font.clone(),
            weak,
        );
        if low < 0.0 {
            let bottom = Pos2::new(rect.left() + 3.0, rect.bottom() - 1.0);
            painter.text(
                bottom,
                Align2::LEFT_BOTTOM,
                format!("{low:.1}/s"),
                font,
                weak,
            );
        }
        let curve = speeds
            .iter()
            .enumerate()
            .map(|(column, &speed)| Pos2::new(rect.left() + column as f32, y_of(speed)))
            .collect();
        painter.add(Shape::line(curve, Stroke::new(1.5, ANIMATED)));
        painter.vline(
            self.x_of(rect, time),
            rect.y_range(),
            Stroke::new(1.0, Color32::from_rgb(230, 70, 70)),
        );
    }
}
