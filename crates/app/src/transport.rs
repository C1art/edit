use egui::{Button, Color32, Rect, Sense, Stroke};
use playback::Player;
use timeline::{Marks, Ticks, Timecode};

/// Height of the scrub bar under each monitor.
const SCRUB_HEIGHT: f32 = 18.0;

/// A scrub bar with the playhead and marked range, then buttons and timecode.
pub fn ui(ui: &mut egui::Ui, player: &mut Player, marks: Option<Marks>) {
    scrub_bar(ui, player, marks.unwrap_or_default());
    let sequence = player.sequence();
    let (timebase, rate) = (sequence.timebase, sequence.format.frame_rate);
    let position = player.position();
    ui.horizontal(|ui| {
        let label = if player.is_playing() { "Pause" } else { "Play" };
        if ui
            .add(Button::new(label).min_size([56.0, 0.0].into()))
            .on_hover_text("Play/pause (Space)")
            .clicked()
        {
            player.toggle();
        }
        if ui
            .button("< Frame")
            .on_hover_text("Previous frame (←)")
            .clicked()
        {
            player.step(-1);
        }
        if ui
            .button("Frame >")
            .on_hover_text("Next frame (→)")
            .clicked()
        {
            player.step(1);
        }
        ui.monospace(format!(
            "{} / {}",
            Timecode::new(position, timebase, rate),
            Timecode::new(player.duration(), timebase, rate)
        ));
        if let Some(Marks {
            mark_in: Some(mark_in),
            mark_out: Some(mark_out),
        }) = marks
        {
            ui.monospace(format!(
                "In-Out {}",
                Timecode::new(mark_out - mark_in, timebase, rate)
            ));
        }
    });
}

/// Where an out mark set now falls: after the frame on screen, which it includes, as in
/// other editors.
pub fn out_point(player: &Player) -> Ticks {
    let rate = player.frame_rate();
    let timebase = player.sequence().timebase;
    let frame = rate.frame_at(player.position(), timebase);
    rate.frame_start(frame + 1, timebase).min(player.duration())
}

fn scrub_bar(ui: &mut egui::Ui, player: &mut Player, marks: Marks) {
    let size = egui::vec2(ui.available_width(), SCRUB_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    painter.rect_filled(rect, 2.0, visuals.extreme_bg_color);
    let duration = player.duration();
    if duration <= Ticks::ZERO {
        return;
    }
    let to_x =
        |time: Ticks| rect.left() + rect.width() * (time.0 as f64 / duration.0 as f64) as f32;
    if marks.is_set() {
        let marked = marks.range(duration);
        let range = Rect::from_x_y_ranges(to_x(marked.start)..=to_x(marked.end), rect.y_range());
        painter.rect_filled(range, 0.0, visuals.selection.bg_fill.gamma_multiply(0.6));
    }
    let playhead = to_x(player.position());
    painter.line_segment(
        [
            egui::pos2(playhead, rect.top()),
            egui::pos2(playhead, rect.bottom()),
        ],
        Stroke::new(2.0, Color32::from_rgb(80, 160, 255)),
    );
    if let Some(pointer) = response.interact_pointer_pos() {
        let fraction = f64::from((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        player.seek(Ticks((fraction * duration.0 as f64).round() as i64));
    }
}

/// What a transport key asks of the focused viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKey {
    Toggle,
    /// Plays forwards, faster on each press.
    Forward,
    /// Plays backwards, faster on each press.
    Reverse,
    Pause,
    Step(i64),
    Start,
    End,
    PreviousEdit,
    NextEdit,
}

pub fn handle_key(player: &mut Player, key: TransportKey) {
    match key {
        TransportKey::Toggle => player.toggle(),
        TransportKey::Forward => player.shuttle(faster(player.rate(), 1)),
        TransportKey::Reverse => player.shuttle(faster(player.rate(), -1)),
        TransportKey::Pause => player.pause(),
        TransportKey::Step(frames) => player.step(frames),
        TransportKey::Start => player.seek(Ticks::ZERO),
        TransportKey::End => player.seek(player.duration()),
        TransportKey::PreviousEdit | TransportKey::NextEdit => {
            let position = player.position();
            let points = player.sequence().edit_points();
            let target = if key == TransportKey::NextEdit {
                points.into_iter().find(|&point| point > position)
            } else {
                points.into_iter().rev().find(|&point| point < position)
            };
            if let Some(target) = target {
                player.pause();
                player.seek(target);
            }
        }
    }
}

/// The shuttle rate after pressing towards `direction` (1 or -1): normal speed from a
/// stop or the other way, then doubling up to the fastest.
fn faster(rate: i32, direction: i32) -> i32 {
    const FASTEST: i32 = 8;
    if rate.signum() == direction {
        (rate * 2).clamp(-FASTEST, FASTEST)
    } else {
        direction
    }
}
