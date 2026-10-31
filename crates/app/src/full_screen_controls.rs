//! What a video player shows over the picture in full screen: a seek bar, play/pause,
//! skips back and forward, the time, the speed and the way out. They get out of the way
//! while the pointer rests and the video plays.

use egui::{Color32, CursorIcon, Mesh, Rect, RichText, Sense, UiBuilder};
use playback::{PlayRate, Player};
use timeline::{Marks, Ticks, Timebase};

use crate::icons::{self, Icon};

pub(crate) const SKIP_SECONDS: i64 = 5;
const SPEEDS: [i32; 7] = [25, 50, 75, 100, 125, 150, 200];
/// Seconds without input before the controls and pointer hide.
const IDLE: f64 = 2.5;
const BAR_HEIGHT: f32 = 84.0;
const BUTTON: egui::Vec2 = egui::vec2(36.0, 30.0);

const SECONDS: Timebase = match Timebase::new(1, 1) {
    Some(timebase) => timebase,
    None => unreachable!(),
};

/// Draws the controls along the bottom of `area` when they are wanted, and says whether
/// leaving full screen was asked for.
pub(crate) fn show(ui: &mut egui::Ui, area: Rect, player: &mut Player) -> bool {
    let bar = Rect::from_min_max(
        egui::pos2(area.left(), area.bottom() - BAR_HEIGHT),
        area.max,
    );
    let last_input = ui.scope_id().with("full_screen_last_input");
    if !wanted(ui.ctx(), last_input, bar, player) {
        ui.ctx().set_cursor_icon(CursorIcon::None);
        return false;
    }
    shade(ui.painter(), bar);
    // Clicks between the controls stay off the picture, which would play or pause.
    ui.interact(bar, ui.scope_id().with("full_screen_bar"), Sense::click());
    let inner = bar.shrink2(egui::vec2(24.0, 10.0));
    ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
        ui.add_space(8.0);
        crate::transport::scrub_bar(ui, player, Marks::default());
        ui.add_space(4.0);
        ui.horizontal(|ui| buttons(ui, player)).inner
    })
    .inner
}

/// Whether the controls show: while paused, while the pointer is over them, and for a
/// moment after any input.
fn wanted(ctx: &egui::Context, id: egui::Id, bar: Rect, player: &Player) -> bool {
    let (now, input) = ctx.input(|input| {
        let pressed = input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::Key { pressed: true, .. }));
        let pointer = input.pointer.delta() != egui::Vec2::ZERO || input.pointer.any_down();
        (input.time, pressed || pointer)
    });
    let last = ctx.data_mut(|data| {
        let last = data.get_temp_mut_or(id, now);
        if input {
            *last = now;
        }
        *last
    });
    let over = ctx
        .pointer_hover_pos()
        .is_some_and(|pointer| bar.contains(pointer));
    !player.is_playing() || over || now - last < IDLE
}

/// A dark fade up from the bottom edge, so the controls read over a bright picture.
fn shade(painter: &egui::Painter, bar: Rect) {
    let mut mesh = Mesh::default();
    let dark = Color32::from_black_alpha(200);
    mesh.colored_vertex(bar.left_top(), Color32::TRANSPARENT);
    mesh.colored_vertex(bar.right_top(), Color32::TRANSPARENT);
    mesh.colored_vertex(bar.right_bottom(), dark);
    mesh.colored_vertex(bar.left_bottom(), dark);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
}

fn buttons(ui: &mut egui::Ui, player: &mut Player) -> bool {
    let play = if player.is_playing() {
        Icon::Pause
    } else {
        Icon::Play
    };
    if icons::button(ui, play, BUTTON)
        .on_hover_text("Play/pause (Space)")
        .clicked()
    {
        player.toggle();
    }
    for (label, seconds, tip) in [
        ("−5 s", -SKIP_SECONDS, "Back 5 seconds (Left)"),
        ("+5 s", SKIP_SECONDS, "Forward 5 seconds (Right)"),
    ] {
        let button = egui::Button::new(RichText::new(label).size(14.0)).min_size(BUTTON);
        if ui.add(button).on_hover_text(tip).clicked() {
            skip(player, seconds);
        }
    }
    ui.add_space(8.0);
    let timebase = player.sequence().timebase;
    let long = player.duration().rescale(timebase, SECONDS).0 >= 3600;
    let time = format!(
        "{} / {}",
        clock(player.position(), timebase, long),
        clock(player.duration(), timebase, long)
    );
    ui.label(
        RichText::new(time)
            .monospace()
            .size(14.0)
            .color(Color32::WHITE),
    );
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let leave = icons::button(ui, Icon::Close, BUTTON)
            .on_hover_text("Leave full screen (Esc)")
            .clicked();
        speed_menu(ui, player);
        leave
    })
    .inner
}

fn speed_menu(ui: &mut egui::Ui, player: &mut Player) {
    let current = player.speed();
    let label = RichText::new(speed_text(current.0)).size(14.0);
    ui.menu_button(label, |ui| {
        for speed in SPEEDS {
            if ui
                .selectable_label(current.0 == speed, speed_text(speed))
                .clicked()
            {
                player.set_speed(PlayRate(speed));
                ui.close();
            }
        }
    })
    .response
    .on_hover_text("Playback speed");
}

fn speed_text(percent: i32) -> String {
    let speed = format!("{:.2}", f64::from(percent) / 100.0);
    format!("{}×", speed.trim_end_matches('0').trim_end_matches('.'))
}

pub(crate) fn skip(player: &mut Player, seconds: i64) {
    let timebase = player.sequence().timebase;
    let by = Ticks(seconds).rescale(SECONDS, timebase);
    player.seek(Ticks(player.position().0 + by.0));
}

/// `time` as a player shows it: minutes and seconds, with hours when `long`.
fn clock(time: Ticks, timebase: Timebase, long: bool) -> String {
    let seconds = time.0.max(0) * i64::from(timebase.num()) / i64::from(timebase.den());
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if long {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{}:{seconds:02}", hours * 60 + minutes)
    }
}
