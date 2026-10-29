use egui::{Color32, Rect, RichText, Sense, Stroke};
use playback::Player;
use timeline::{Marks, Ticks, Timebase, Timecode};

use crate::icons::{self, Icon, Look};
use crate::keymap::{Command, Keymap};
use crate::theme;

/// Height of the scrub bar under each monitor.
const SCRUB_HEIGHT: f32 = 18.0;
const BUTTON: egui::Vec2 = egui::vec2(30.0, 24.0);

/// How long Play Around plays before the playhead, and after it.
const PREROLL_SECONDS: u32 = 2;

const PLAYBACK_BUTTONS: usize = 4;

/// A button a monitor adds beside the transport, and what clicking it asks for.
pub struct Extra<A> {
    pub icon: Icon,
    pub tip: String,
    pub action: A,
}

/// The monitor's buttons before and after the transport, and whether it has a settings
/// menu.
pub struct Buttons<'a, A> {
    pub before: &'a [Extra<A>],
    pub after: &'a [Extra<A>],
    pub settings: bool,
}

pub struct Pressed<A> {
    pub action: Option<A>,
    /// The settings button, for the monitor to hang its menu from.
    pub settings: Option<egui::Response>,
}

/// A scrub bar with the playhead and marked range, then buttons and timecode.
pub fn ui<A: Copy>(
    ui: &mut egui::Ui,
    player: &mut Player,
    marks: Option<Marks>,
    keymap: &Keymap,
    buttons: Buttons<'_, A>,
) -> Pressed<A> {
    let marks = marks.unwrap_or_default();
    if player.is_looping() {
        // The marks can move while looping, and the loop follows them.
        player.set_looping(Some(marks.range(player.duration())));
    }
    scrub_bar(ui, player, marks);
    let sequence = player.sequence();
    let (timebase, rate) = (sequence.timebase, sequence.format.frame_rate);
    let timecode = RichText::new(Timecode::new(player.position(), timebase, rate).to_string())
        .monospace()
        .size(14.0)
        .color(theme::ACCENT);
    let spacing = ui.spacing().item_spacing.x;
    let width_of =
        |count: usize| count as f32 * BUTTON.x + count.saturating_sub(1) as f32 * spacing;
    let extras = buttons.before.len() + buttons.after.len() + usize::from(buttons.settings);
    // On a narrow monitor the monitor's own buttons take a row under the transport,
    // rather than squeezing the timecode out.
    let one_row = text_width(ui, &timecode) + spacing + width_of(PLAYBACK_BUTTONS + extras)
        <= ui.available_width();
    let mut pressed = Pressed {
        action: None,
        settings: None,
    };
    ui.horizontal(|ui| {
        let row = ui.available_rect_before_wrap();
        if let Some(position) = crate::timecode_field::playhead(
            ui,
            ui.scope_id().with("timecode"),
            player.position(),
            (timebase, rate),
            14.0,
        ) {
            player.seek(position);
        }
        let duration = Timecode::new(player.duration(), timebase, rate);
        let mut times = vec![(
            RichText::new(duration.to_string())
                .monospace()
                .color(theme::WEAK_TEXT),
            "Duration",
        )];
        if let Marks {
            mark_in: Some(mark_in),
            mark_out: Some(mark_out),
        } = marks
        {
            let marked = Timecode::new(mark_out - mark_in, timebase, rate);
            let text = RichText::new(format!("In-Out {marked}")).monospace().weak();
            times.push((text, "Duration of the marked range"));
        }
        let times_width: f32 = times
            .iter()
            .map(|(text, _)| text_width(ui, text) + spacing)
            .sum();
        let group = width_of(PLAYBACK_BUTTONS + if one_row { extras } else { 0 });
        // The buttons sit in the middle, unless that would put them over the times.
        let start = (row.center().x - group / 2.0)
            .min(row.right() - times_width - group)
            .max(ui.cursor().left());
        ui.add_space(start - ui.cursor().left());
        if one_row {
            pressed.action = monitor_buttons(ui, buttons.before);
        }
        playback_buttons(ui, player, marks, keymap);
        if one_row {
            pressed.action = pressed.action.or(monitor_buttons(ui, buttons.after));
            pressed.settings = buttons.settings.then(|| settings_button(ui));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // A narrow monitor drops the In-Out duration first, then the whole one.
            for (text, hover) in times {
                if text_width(ui, &text) > ui.available_width() {
                    break;
                }
                ui.label(text).on_hover_text(hover);
            }
        });
    });
    if !one_row && extras > 0 {
        ui.horizontal(|ui| {
            let gap = 3.0 * spacing;
            let width = width_of(extras) + gap;
            let start =
                (ui.available_rect_before_wrap().center().x - width / 2.0).max(ui.cursor().left());
            ui.add_space(start - ui.cursor().left());
            pressed.action = monitor_buttons(ui, buttons.before);
            ui.add_space(gap);
            pressed.action = pressed.action.or(monitor_buttons(ui, buttons.after));
            pressed.settings = buttons.settings.then(|| settings_button(ui));
        });
    }
    pressed
}

fn playback_buttons(ui: &mut egui::Ui, player: &mut Player, marks: Marks, keymap: &Keymap) {
    if icons::button(ui, Icon::StepBack, BUTTON)
        .on_hover_text(keymap.tip("Previous frame", Command::StepBack))
        .clicked()
    {
        player.step(-1);
    }
    let play = if player.is_playing() {
        Icon::Pause
    } else {
        Icon::Play
    };
    if icons::button(ui, play, BUTTON)
        .on_hover_text(keymap.tip("Play/pause", Command::PlayStop))
        .clicked()
    {
        player.toggle();
    }
    if icons::button(ui, Icon::StepForward, BUTTON)
        .on_hover_text(keymap.tip("Next frame", Command::StepForward))
        .clicked()
    {
        player.step(1);
    }
    if icons::sized_toggle(ui, player.is_looping(), Icon::Loop, Look::Filled, BUTTON)
        .on_hover_text(keymap.tip(
            "Loop playback of the marked range, or all of it",
            Command::ToggleLoop,
        ))
        .clicked()
    {
        handle_key(player, TransportKey::ToggleLoop, marks);
    }
}

fn monitor_buttons<A: Copy>(ui: &mut egui::Ui, buttons: &[Extra<A>]) -> Option<A> {
    let mut pressed = None;
    for button in buttons {
        if icons::button(ui, button.icon, BUTTON)
            .on_hover_text(&button.tip)
            .clicked()
        {
            pressed = Some(button.action);
        }
    }
    pressed
}

fn settings_button(ui: &mut egui::Ui) -> egui::Response {
    icons::button(ui, Icon::Settings, BUTTON).on_hover_text("Settings")
}

fn text_width(ui: &egui::Ui, text: &RichText) -> f32 {
    egui::WidgetText::from(text.clone())
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        )
        .size()
        .x
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
    let track = Rect::from_center_size(rect.center(), egui::vec2(rect.width(), 4.0));
    painter.rect_filled(track, 2.0, visuals.widgets.inactive.bg_fill);
    let duration = player.duration();
    if duration <= Ticks::ZERO {
        return;
    }
    let to_x =
        |time: Ticks| rect.left() + rect.width() * (time.0 as f64 / duration.0 as f64) as f32;
    let playhead = to_x(player.position());
    let played = Rect::from_x_y_ranges(track.left()..=playhead, track.y_range());
    painter.rect_filled(played, 2.0, theme::ACCENT.gamma_multiply(0.5));
    // Over the played part, which would hide them otherwise.
    if marks.is_set() {
        let marked = marks.range(duration);
        let (left, right) = (to_x(marked.start), to_x(marked.end));
        let band = Rect::from_x_y_ranges(left..=right, rect.y_range().shrink(3.0));
        painter.rect_filled(band, 1.0, Color32::from_white_alpha(40));
        let stroke = Stroke::new(1.5, Color32::from_gray(220));
        let bracket = |x: f32, toward: f32| {
            let (top, bottom) = (band.top(), band.bottom());
            let points = vec![
                egui::pos2(x + toward, top),
                egui::pos2(x, top),
                egui::pos2(x, bottom),
                egui::pos2(x + toward, bottom),
            ];
            egui::Shape::line(points, stroke)
        };
        if marks.mark_in.is_some() {
            painter.add(bracket(left, 4.0));
        }
        if marks.mark_out.is_some() {
            painter.add(bracket(right, -4.0));
        }
    }
    painter.line_segment(
        [
            egui::pos2(playhead, rect.top() + 2.0),
            egui::pos2(playhead, rect.bottom() - 2.0),
        ],
        Stroke::new(2.0, theme::ACCENT),
    );
    painter.circle_filled(egui::pos2(playhead, rect.center().y), 4.5, theme::ACCENT);
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
    /// Plays the marked range once.
    PlayInToOut,
    /// Plays from a little before the playhead to a little after, then goes back to it.
    PlayAround,
    ToggleLoop,
}

/// Acts on `player`, whose monitor has `marks`.
pub fn handle_key(player: &mut Player, key: TransportKey, marks: Marks) {
    match key {
        TransportKey::PlayInToOut => {
            player.play_range(marks.range(player.duration()), None);
        }
        TransportKey::PlayAround => {
            let timebase = player.sequence().timebase;
            let second = Timebase::new(1, 1).map_or(Ticks::ZERO, |seconds| {
                Ticks(i64::from(PREROLL_SECONDS)).rescale(seconds, timebase)
            });
            let position = player.position();
            let start = (position - second).max(Ticks::ZERO);
            let end = (position + second).min(player.duration());
            player.play_range(start..end, Some(position));
        }
        TransportKey::ToggleLoop => {
            let range = (!player.is_looping()).then(|| marks.range(player.duration()));
            player.set_looping(range);
        }
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
