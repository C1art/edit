//! What every dialog shares, so they all open in the same place and end the same way.

use egui::{Align, Align2, Button, Color32, Context, Id, Layout, RichText, Ui, WidgetText, Window};

use crate::theme;

/// A window centered over the editor when it first opens, sized to what it holds.
pub(crate) fn window<'open>(ctx: &Context, title: impl Into<WidgetText>) -> Window<'open> {
    Window::new(title)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
}

/// The row of buttons at a dialog's foot, kept to its right edge. They are laid out from
/// the right, so add Cancel first and the main action after it.
pub(crate) fn footer<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.add_space(4.0);
    // As wide as what's above, or the buttons if they're wider, rather than all the space
    // left: an auto-sized window would grow into that every frame.
    let id = ui.make_persistent_id("dialog footer");
    let buttons = ui.data(|data| data.get_temp::<f32>(id)).unwrap_or_default();
    let size = egui::vec2(
        ui.min_rect().width().max(buttons),
        ui.spacing().interact_size.y,
    );
    let (inner, used) = ui
        .allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), |ui| {
            let inner = add(ui);
            (inner, ui.min_rect().width())
        })
        .inner;
    let frame = ui.ctx().cumulative_frame_nr();
    ui.data_mut(|data| {
        data.insert_temp(id, used);
        data.insert_temp(modal_id(), frame);
    });
    inner
}

/// Whether a dialog was waiting on an answer last frame, so editor keys leave it be.
pub(crate) fn waiting(ctx: &Context) -> bool {
    let shown = ctx.data(|data| data.get_temp::<u64>(modal_id()));
    shown.is_some_and(|frame| frame + 1 >= ctx.cumulative_frame_nr())
}

fn modal_id() -> Id {
    Id::unique("dialog waiting")
}

/// The button for what the dialog is for, set apart in the accent color.
pub(crate) fn primary(text: &str) -> Button<'static> {
    Button::new(RichText::new(text).color(Color32::WHITE)).fill(theme::ACCENT_FILL)
}

/// Focuses the `DragValue` added next with its text selected, so typing replaces the
/// value. It only selects its text when focus arrives before it's drawn, and a window's
/// first, sizing pass takes no focus, so wait until `!ui.is_sizing_pass()`.
pub(crate) fn focus_next_value(ui: &Ui) {
    let id = ui.next_auto_id();
    ui.memory_mut(|memory| memory.request_focus(id));
}
