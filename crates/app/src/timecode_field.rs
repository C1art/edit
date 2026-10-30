use egui::{Id, RichText, TextEdit};
use timeline::{FrameRate, Ticks, TimeDisplay, Timebase, Timecode, TypedTimecode};

use crate::theme;

#[derive(Clone)]
struct Typing {
    text: String,
    focused: bool,
}

/// What a playhead's timecode did this frame.
pub struct Playhead {
    /// Where typing said to go.
    pub seek: Option<Ticks>,
    /// The timecode, or the field typing in it, for hanging a context menu from.
    pub response: egui::Response,
}

/// A playhead's timecode, counted from `start`, which a click turns into a field for
/// typing where to go: Enter goes there, Escape or clicking away leaves it be.
pub fn playhead(
    ui: &mut egui::Ui,
    id: Id,
    (position, start): (Ticks, Ticks),
    (timebase, rate, display): (Timebase, FrameRate, TimeDisplay),
    size: f32,
) -> Playhead {
    let shown = Timecode::shown(position + start, timebase, rate, display).to_string();
    let font = egui::FontId::monospace(size);
    let typing = ui.data(|data| data.get_temp::<Typing>(id));
    let Some(Typing { mut text, focused }) = typing else {
        let label = ui
            .add(
                egui::Label::new(RichText::new(&shown).font(font).color(theme::ACCENT))
                    .sense(egui::Sense::click()),
            )
            .on_hover_cursor(egui::CursorIcon::Text)
            .on_hover_text("Click to type a timecode, or +/- frames to move by");
        if label.clicked() {
            let typing = Typing {
                text: shown,
                focused: false,
            };
            ui.data_mut(|data| data.insert_temp(id, typing));
        }
        return Playhead {
            seek: None,
            response: label,
        };
    };

    let width = egui::WidgetText::from(RichText::new(&shown).font(font.clone()))
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        )
        .size()
        .x;
    let field = ui.add(
        TextEdit::singleline(&mut text)
            .id(id)
            .font(font)
            .text_color(theme::ACCENT)
            .desired_width(width)
            .margin(egui::vec2(0.0, 0.0)),
    );
    // The field can't take focus before it exists, so it asks on its first frame.
    if !focused {
        field.request_focus();
    }
    if field.gained_focus() {
        select_all(ui, id, &text);
    }
    if !focused || ui.memory(|memory| memory.has_focus(id)) {
        let typing = Typing {
            text,
            focused: true,
        };
        ui.data_mut(|data| data.insert_temp(id, typing));
        return Playhead {
            seek: None,
            response: field,
        };
    }
    ui.data_mut(|data| data.remove::<Typing>(id));
    let entered = ui.input(|input| input.key_pressed(egui::Key::Enter));
    let seek = entered
        .then(|| TypedTimecode::parse(&text, rate, display))
        .flatten()
        .map(|typed| match typed {
            TypedTimecode::At(frame) => frame - rate.frame_at(start, timebase),
            TypedTimecode::By(frames) => rate.frame_at(position, timebase).saturating_add(frames),
        })
        .map(|frame| rate.frame_start(frame.max(0), timebase));
    Playhead {
        seek,
        response: field,
    }
}

/// Selects the whole timecode as typing starts, so what's typed replaces it.
fn select_all(ui: &egui::Ui, id: Id, text: &str) {
    let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
    let all = egui::text::CCursorRange::select_all_str(text);
    state.cursor.set_char_range(Some(all));
    state.store(ui.ctx(), id);
}

pub fn frames_text(
    frames: i64,
    (timebase, rate, display): (Timebase, FrameRate, TimeDisplay),
) -> String {
    let ticks = rate.frame_start(frames, timebase);
    Timecode::shown(ticks, timebase, rate, display).to_string()
}

/// A count of frames shown as timecode, which takes timecode when typed into too.
pub fn frames(
    frames: &mut i64,
    (timebase, rate, display): (Timebase, FrameRate, TimeDisplay),
) -> egui::DragValue<'_> {
    egui::DragValue::new(frames)
        .speed(0.5)
        .custom_formatter(move |frames, _| frames_text(frames as i64, (timebase, rate, display)))
        .custom_parser(
            move |text| match TypedTimecode::parse(text, rate, display)? {
                TypedTimecode::At(frames) => Some(frames as f64),
                TypedTimecode::By(_) => None,
            },
        )
}
