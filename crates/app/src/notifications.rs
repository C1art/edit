use std::collections::VecDeque;

use egui::{Align2, Context, CornerRadius, Frame, Id, Label, Margin, Order, ScrollArea, Stroke};

use crate::editor::Editor;
use crate::icons::{self, Icon};
use crate::theme;

/// Seconds a message stays up while the pointer is elsewhere.
const SHOWN_FOR: f64 = 8.0;
/// Seconds of that spent fading out.
const FADE: f64 = 0.6;
const MAX_TOASTS: usize = 3;
const KEPT: usize = 100;
const TOAST_WIDTH: f32 = 360.0;

/// Messages for the user: each pops up in the corner for a few seconds, as Premiere's
/// notifications do, and stays listed under Window > Messages.
#[derive(Default)]
pub(crate) struct Notifications {
    /// Oldest first.
    entries: VecDeque<Entry>,
    pub(crate) history_open: bool,
}

struct Entry {
    text: String,
    shown_until: f64,
}

impl Notifications {
    fn take(&mut self, inbox: &mut Vec<String>, now: f64) {
        for text in inbox.drain(..) {
            self.entries.push_back(Entry {
                text,
                shown_until: now + SHOWN_FOR,
            });
        }
        let excess = self.entries.len().saturating_sub(KEPT);
        self.entries.drain(..excess);
    }
}

pub(crate) fn ui(editor: &mut Editor, ctx: &Context) {
    let now = ctx.input(|input| input.time);
    let notifications = &mut editor.notifications;
    notifications.take(&mut editor.notices, now);
    toasts(notifications, ctx, now);
    history(notifications, ctx);
}

fn toasts(notifications: &mut Notifications, ctx: &Context, now: f64) {
    let showing: Vec<&mut Entry> = notifications
        .entries
        .iter_mut()
        .filter(|entry| entry.shown_until > now)
        .collect();
    if showing.is_empty() {
        return;
    }
    let skip = showing.len().saturating_sub(MAX_TOASTS);
    let mut next_change = f64::INFINITY;
    egui::Area::new(Id::unique("toasts"))
        .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
        .order(Order::Foreground)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            for entry in showing.into_iter().skip(skip) {
                let left = entry.shown_until - now;
                ui.scope(|ui| {
                    ui.multiply_opacity((left / FADE).clamp(0.0, 1.0) as f32);
                    let toast = toast(ui, &entry.text);
                    if toast.closed {
                        entry.shown_until = now;
                    } else if toast.hovered {
                        // Held while read, with time to move away before it fades.
                        entry.shown_until = entry.shown_until.max(now + FADE + 1.0);
                    }
                });
                let fade_from = entry.shown_until - FADE;
                next_change = next_change.min(if now < fade_from { fade_from } else { now });
            }
            if skip > 0 {
                let more = if skip == 1 {
                    "1 more message".to_owned()
                } else {
                    format!("{skip} more messages")
                };
                if ui.link(more).clicked() {
                    notifications.history_open = true;
                }
            }
        });
    if next_change.is_finite() {
        ctx.request_repaint_after_secs((next_change - now).max(0.0) as f32);
    }
}

struct Toast {
    hovered: bool,
    closed: bool,
}

fn toast(ui: &mut egui::Ui, text: &str) -> Toast {
    let mut closed = false;
    let frame = Frame::new()
        .fill(theme::RAISED)
        .stroke(Stroke::new(
            1.0,
            ui.visuals().warn_fg_color.gamma_multiply(0.6),
        ))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(TOAST_WIDTH);
            ui.horizontal_top(|ui| {
                ui.allocate_ui(egui::vec2(TOAST_WIDTH - 26.0, 0.0), |ui| {
                    ui.set_min_width(TOAST_WIDTH - 26.0);
                    ui.add(Label::new(text).wrap());
                });
                closed = icons::button(ui, Icon::Close, egui::vec2(18.0, 18.0))
                    .on_hover_text("Dismiss")
                    .clicked();
            });
        });
    Toast {
        hovered: frame.response.contains_pointer(),
        closed,
    }
}

fn history(notifications: &mut Notifications, ctx: &Context) {
    let mut open = notifications.history_open;
    let mut clear = false;
    crate::dialog::window(ctx, "Messages")
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_width(TOAST_WIDTH + 40.0);
            if notifications.entries.is_empty() {
                ui.weak("No messages.");
            }
            ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for entry in notifications.entries.iter().rev() {
                    ui.add(Label::new(&entry.text).wrap());
                    ui.separator();
                }
            });
            crate::dialog::footer(ui, |ui| {
                let any = !notifications.entries.is_empty();
                clear = ui.add_enabled(any, egui::Button::new("Clear")).clicked();
            });
        });
    if clear {
        notifications.entries.clear();
    }
    notifications.history_open = open;
}
