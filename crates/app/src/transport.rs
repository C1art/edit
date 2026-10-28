use egui::{Button, Key, Slider};
use playback::Player;
use timeline::{FrameRate, Ticks, Timebase, Timecode};

/// Rate used to display timecode for media without video.
const FALLBACK_RATE: FrameRate = match FrameRate::new(30, 1) {
    Some(rate) => rate,
    None => unreachable!(),
};

pub fn frame_rate(player: &Player) -> FrameRate {
    player
        .info()
        .and_then(|info| info.video.as_ref()?.frame_rate)
        .unwrap_or(FALLBACK_RATE)
}

/// Play/pause, timecode and a scrub bar for the open media.
pub fn ui(ui: &mut egui::Ui, player: &mut Player, timebase: Timebase) {
    let rate = frame_rate(player);
    let position = player.position();
    ui.horizontal(|ui| {
        let label = if player.is_playing() { "Pause" } else { "Play" };
        if ui
            .add(Button::new(label).min_size([56.0, 0.0].into()))
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
        let timecode = Timecode::new(position, timebase, rate);
        let duration = player
            .duration()
            .map(|duration| Timecode::new(duration, timebase, rate).to_string())
            .unwrap_or_else(|| "--:--:--:--".to_owned());
        ui.monospace(format!("{timecode} / {duration}"));

        if let Some(duration) = player.duration() {
            let mut scrub = position.0;
            ui.spacing_mut().slider_width = ui.available_width();
            let response = ui.add(
                Slider::new(&mut scrub, 0..=duration.0)
                    .show_value(false)
                    .trailing_fill(true),
            );
            if response.changed() {
                player.seek(Ticks(scrub));
            }
        }
    });
}

/// Editor-wide playback shortcuts. Skipped while a text field has focus.
pub fn handle_shortcuts(ctx: &egui::Context, player: &mut Player) {
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    let (toggle, back, forward, home, end, pause, play) = ctx.input(|input| {
        (
            input.key_pressed(Key::Space),
            input.key_pressed(Key::ArrowLeft),
            input.key_pressed(Key::ArrowRight),
            input.key_pressed(Key::Home),
            input.key_pressed(Key::End),
            input.key_pressed(Key::K),
            input.key_pressed(Key::L),
        )
    });
    if toggle {
        player.toggle();
    }
    if pause {
        player.pause();
    }
    if play {
        player.play();
    }
    if back {
        player.step(-1);
    }
    if forward {
        player.step(1);
    }
    if home {
        player.seek(Ticks::ZERO);
    }
    if end {
        if let Some(duration) = player.duration() {
            player.seek(duration);
        }
    }
}
