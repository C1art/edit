use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use egui::{
    pos2, vec2, Align, Align2, Color32, Key, Layout, Modifiers, Rect, ScrollArea, Sense, TextEdit,
    Ui,
};

use super::{FileBrowser, Intent, Outcome, SortBy, AUDIO_FILES, STILL_IMAGES, VIDEO_FILES};
use crate::icons::{self, Icon};
use crate::metadata_panel::size;
use crate::theme;

const WIDTH: f32 = 760.0;
const ROW_HEIGHT: f32 = 22.0;
const ICON: f32 = 14.0;
/// Where names start, after their icons.
const NAME_LEFT: f32 = 30.0;
const PLACES_WIDTH: f32 = 140.0;
const SIZE_WIDTH: f32 = 84.0;
const AGE_WIDTH: f32 = 112.0;

impl FileBrowser {
    /// Draws the browser, with `extra` left of the buttons, and says what was chosen.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        action: &str,
        extra: impl FnOnce(&mut Ui),
    ) -> Option<Outcome> {
        ui.set_width(WIDTH);
        let mut outcome = self.take_found();
        if self.looking.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(30));
        }
        // Taken before the text fields see them, which would move their cursors.
        let (down, up) = ui.input_mut(|input| {
            (
                input.consume_key(Modifiers::NONE, Key::ArrowDown),
                input.consume_key(Modifiers::NONE, Key::ArrowUp),
            )
        });
        if down || up {
            self.step(down);
        }
        let (enter, escape) = ui.input(|input| {
            (
                input.key_pressed(Key::Enter),
                input.key_pressed(Key::Escape),
            )
        });
        self.location_bar(ui, enter);
        let list_height = (ui.ctx().content_rect().height() * 0.45).clamp(180.0, 440.0);
        ui.horizontal_top(|ui| {
            self.places_list(ui, list_height);
            // A bare separator would stretch to all the height the window offers.
            ui.add_sized([8.0, list_height], egui::Separator::default().vertical());
            ui.vertical(|ui| {
                if let Some(opened) = self.entries_list(ui, list_height) {
                    outcome = Some(opened);
                }
            });
        });
        let name_entered = self.name_row(ui, enter);
        if let Some(replaced) = self.replace_question(ui) {
            outcome = Some(replaced);
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        let mut submitted = name_entered;
        let mut cancelled = escape && self.replacing.take().is_none();
        ui.horizontal(|ui| {
            extra(ui);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                cancelled |= ui.button("Cancel").clicked();
                submitted |= ui.button(action).clicked();
            });
        });
        let nothing_focused = ui.memory(|memory| memory.focused().is_none());
        if submitted || (enter && nothing_focused && outcome.is_none()) {
            outcome = outcome.or_else(|| self.submit());
            self.focus_name = outcome.is_none();
        }
        if cancelled {
            outcome = Some(Outcome::Cancelled);
        }
        outcome
    }

    fn location_bar(&mut self, ui: &mut Ui, enter: bool) {
        ui.horizontal(|ui| {
            let size = vec2(26.0, 22.0);
            let back = ui.add_enabled_ui(!self.back.is_empty(), |ui| {
                icons::button(ui, Icon::Back, size).on_hover_text("Back")
            });
            if back.inner.clicked() {
                self.go_back();
            }
            let has_parent = self.folder.as_deref().and_then(Path::parent).is_some();
            let up = ui.add_enabled_ui(has_parent, |ui| {
                icons::button(ui, Icon::Up, size).on_hover_text("Up one folder")
            });
            if up.inner.clicked() {
                self.go_up();
            }
            let location = ui.add(
                TextEdit::singleline(&mut self.location)
                    .hint_text("Folder")
                    .desired_width(f32::INFINITY),
            );
            if location.lost_focus() && enter {
                let typed = PathBuf::from(self.location.trim().trim_matches('"'));
                if !typed.as_os_str().is_empty() {
                    self.look(typed, Intent::Show);
                }
            }
        });
    }

    fn places_list(&mut self, ui: &mut Ui, height: f32) {
        let mut go = None;
        ScrollArea::vertical()
            .id_salt("places")
            .max_height(height)
            .min_scrolled_height(height)
            .auto_shrink([false, false])
            .max_width(PLACES_WIDTH)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.set_width(PLACES_WIDTH);
                    for place in &self.places {
                        let here = self.folder.as_ref() == Some(&place.path);
                        let label = ui.selectable_label(here, &place.name);
                        if label
                            .on_hover_text(place.path.display().to_string())
                            .clicked()
                        {
                            go = Some(place.path.clone());
                        }
                    }
                });
            });
        if let Some(go) = go {
            self.look(go, Intent::Show);
        }
    }

    fn entries_list(&mut self, ui: &mut Ui, height: f32) -> Option<Outcome> {
        ui.spacing_mut().item_spacing.y = 0.0;
        self.header(ui);
        let mut scroll = ScrollArea::vertical()
            .id_salt("files")
            .max_height(height - ROW_HEIGHT)
            .min_scrolled_height(height - ROW_HEIGHT)
            .auto_shrink([false, false]);
        if let Some(row) = self.scroll_to.take() {
            let (offset, view) = self.view;
            let (top, bottom) = (row as f32 * ROW_HEIGHT, (row + 1) as f32 * ROW_HEIGHT);
            if top < offset {
                scroll = scroll.vertical_scroll_offset(top);
            } else if bottom > offset + view {
                scroll = scroll.vertical_scroll_offset(bottom - view);
            }
        }
        let mut opened = None;
        let now = SystemTime::now();
        let output = scroll.show_rows(ui, ROW_HEIGHT, self.shown.len(), |ui, rows| {
            for row in rows {
                let (rect, response) =
                    ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
                let index = self.shown[row];
                let entry = &self.entries[index];
                let selected = self.selected.contains(&index);
                let visuals = ui.visuals();
                let text_color = if selected {
                    visuals.selection.stroke.color
                } else {
                    visuals.text_color()
                };
                if selected {
                    ui.painter()
                        .rect_filled(rect, 3.0, visuals.selection.bg_fill);
                } else if response.hovered() {
                    ui.painter()
                        .rect_filled(rect, 3.0, visuals.widgets.hovered.weak_bg_fill);
                }
                let font = egui::TextStyle::Body.resolve(ui.style());
                let name_width = rect.width() - SIZE_WIDTH - AGE_WIDTH;
                let (icon, tint) = icon(entry.folder, &entry.path);
                let glyph = Rect::from_center_size(
                    rect.left_center() + vec2(8.0 + ICON / 2.0, 0.0),
                    vec2(ICON, ICON),
                );
                icons::paint(ui.painter(), glyph, icon, tint);
                let name_column = Rect::from_min_size(rect.min, vec2(name_width - 8.0, ROW_HEIGHT));
                ui.painter()
                    .with_clip_rect(rect.intersect(name_column))
                    .text(
                        rect.left_center() + vec2(NAME_LEFT, 0.0),
                        Align2::LEFT_CENTER,
                        &entry.name,
                        font.clone(),
                        text_color,
                    );
                let weak = if selected {
                    text_color
                } else {
                    visuals.weak_text_color()
                };
                if !entry.folder {
                    let at = rect.left_center() + vec2(name_width + SIZE_WIDTH - 12.0, 0.0);
                    ui.painter().text(
                        at,
                        Align2::RIGHT_CENTER,
                        size(entry.size),
                        font.clone(),
                        weak,
                    );
                }
                if let Some(modified) = entry.modified {
                    let at = rect.right_center() - vec2(AGE_WIDTH - 8.0, 0.0);
                    ui.painter()
                        .text(at, Align2::LEFT_CENTER, age(modified, now), font, weak);
                }
                if response.double_clicked() {
                    opened = Some(row);
                } else if response.clicked() {
                    let modifiers = ui.input(|input| input.modifiers);
                    self.select(row, modifiers.command, modifiers.shift);
                }
            }
        });
        self.view = (output.state.clamped_offset().y, output.inner_rect.height());
        if self.shown.is_empty() {
            let message = match (&self.looking, &self.folder) {
                (Some(_), _) => "Reading the folder…",
                (None, None) => "",
                (None, Some(_)) if self.entries.is_empty() => "This folder is empty.",
                (None, Some(_)) => "No files of this type here.",
            };
            ui.painter().text(
                output.inner_rect.center_top() + vec2(0.0, 24.0),
                Align2::CENTER_CENTER,
                message,
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().weak_text_color(),
            );
        }
        opened.and_then(|row| self.open_row(row))
    }

    fn header(&mut self, ui: &mut Ui) {
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
        let name_width = rect.width() - SIZE_WIDTH - AGE_WIDTH;
        let columns = [
            (
                SortBy::Name,
                "Name",
                rect.left(),
                name_width,
                Align2::LEFT_CENTER,
            ),
            (
                SortBy::Size,
                "Size",
                rect.left() + name_width,
                SIZE_WIDTH,
                Align2::RIGHT_CENTER,
            ),
            (
                SortBy::Modified,
                "Modified",
                rect.right() - AGE_WIDTH,
                AGE_WIDTH,
                Align2::LEFT_CENTER,
            ),
        ];
        let font = egui::TextStyle::Body.resolve(ui.style());
        for (by, title, left, width, align) in columns {
            let (sorting, descending) = self.sort;
            let arrow = match (sorting == by, descending) {
                (false, _) => "",
                (true, false) => " ▲",
                (true, true) => " ▼",
            };
            let cell = Rect::from_min_size(pos2(left, rect.top()), vec2(width, ROW_HEIGHT));
            let response = ui.interact(cell, ui.scope_id().with(title), Sense::click());
            let color = if response.hovered() {
                ui.visuals().strong_text_color()
            } else {
                ui.visuals().weak_text_color()
            };
            let at = match align {
                Align2::RIGHT_CENTER => cell.right_center() - vec2(12.0, 0.0),
                _ if by == SortBy::Name => cell.left_center() + vec2(NAME_LEFT, 0.0),
                _ => cell.left_center() + vec2(8.0, 0.0),
            };
            ui.painter()
                .text(at, align, format!("{title}{arrow}"), font.clone(), color);
            if response.clicked() {
                // Newest and biggest first is what anyone sorting by them wants first.
                let first_descending = by != SortBy::Name;
                self.sort = (
                    by,
                    if sorting == by {
                        !descending
                    } else {
                        first_descending
                    },
                );
                self.sort_entries();
            }
        }
    }

    fn name_row(&mut self, ui: &mut Ui, enter: bool) -> bool {
        let mut entered = false;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("File name:");
            let mut filter = self.filter;
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                egui::ComboBox::from_id_salt("file filter")
                    .width(180.0)
                    .selected_text(self.filter().name)
                    .show_ui(ui, |ui| {
                        for (index, each) in self.filters.iter().enumerate() {
                            ui.selectable_value(&mut filter, index, each.name);
                        }
                    });
                let name =
                    ui.add(TextEdit::singleline(&mut self.name).desired_width(f32::INFINITY));
                if std::mem::take(&mut self.focus_name) {
                    name.request_focus();
                }
                if name.changed() {
                    self.name_from_selection = false;
                    self.replacing = None;
                }
                entered = name.lost_focus() && enter;
            });
            if filter != self.filter {
                self.filter = filter;
                self.sort_entries();
            }
        });
        entered
    }

    fn replace_question(&mut self, ui: &mut Ui) -> Option<Outcome> {
        let path = self.replacing.clone()?;
        let mut outcome = None;
        ui.horizontal(|ui| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or_default();
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!("{name} already exists. Replace it?"),
            );
            if ui.button("Replace").clicked() {
                outcome = Some(Outcome::Chosen(vec![path.clone()]));
            }
            if ui.button("Keep").clicked() {
                self.replacing = None;
            }
        });
        outcome
    }

    /// Moves the selection one row down or up, or onto the first row.
    fn step(&mut self, down: bool) {
        let Some(last) = self.shown.len().checked_sub(1) else {
            return;
        };
        let current = self
            .selected
            .last()
            .and_then(|index| self.shown.iter().position(|shown| shown == index));
        let row = match current {
            None => 0,
            Some(row) if down => (row + 1).min(last),
            Some(row) => row.saturating_sub(1),
        };
        self.select(row, false, false);
        self.scroll_to = Some(row);
    }
}

fn icon(folder: bool, path: &Path) -> (Icon, Color32) {
    if folder {
        (Icon::Folder, Color32::from_rgb(214, 172, 86))
    } else if VIDEO_FILES.shows(path) {
        (Icon::File, Color32::from_rgb(96, 146, 236))
    } else if AUDIO_FILES.shows(path) {
        (Icon::File, Color32::from_rgb(88, 186, 128))
    } else if STILL_IMAGES.shows(path) {
        (Icon::File, Color32::from_rgb(178, 128, 226))
    } else {
        (Icon::File, theme::WEAK_TEXT)
    }
}

/// How long ago, which needs no time zone, unlike a date would.
fn age(modified: SystemTime, now: SystemTime) -> String {
    let minutes = now.duration_since(modified).unwrap_or_default().as_secs() / 60;
    let (count, unit) = match minutes {
        0 => return "just now".to_owned(),
        1..60 => (minutes, "minute"),
        60..1_440 => (minutes / 60, "hour"),
        1_440..2_880 => return "yesterday".to_owned(),
        2_880..20_160 => (minutes / 1_440, "day"),
        20_160..86_400 => (minutes / 10_080, "week"),
        86_400..1_051_200 => (minutes / 43_200, "month"),
        _ => (minutes / 525_600, "year"),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}
