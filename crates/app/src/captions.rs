use egui::text::{CCursor, CCursorRange};
use egui::{Color32, RichText};
use timeline::{preview, Caption, Captions, Rgb, SetCaptions, Ticks, Timebase, Timecode};

use crate::editor::Editor;
use crate::mixer::{response_state, Change};
use crate::project_files::PathPurpose;

/// How long a caption added at the playhead lasts, unless the next one starts sooner.
const NEW_CAPTION_SECONDS: i64 = 3;

#[derive(Default)]
pub struct CaptionPanel {
    /// Captions being typed or dragged, shown in the program but kept out of the undo
    /// history until the change ends, with the timeline revision they were taken from.
    draft: Option<(u64, Captions)>,
    /// The caption whose text field takes focus next frame, by start.
    focus: Option<Ticks>,
}

pub fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    let revision = editor.timeline.revision();
    let sequence = editor.timeline.sequence();
    let (timebase, rate, display) = (
        sequence.timebase,
        sequence.format.frame_rate,
        sequence.time_display,
    );
    let start = sequence.start_time;
    let mut captions = match &editor.captions.draft {
        // An undo or another edit since the draft began wins over it.
        Some((taken, draft)) if *taken == revision => draft.clone(),
        _ => sequence.captions.clone(),
    };
    let playhead = editor.playhead();
    let mut change = None;
    // The panel is often narrow; rows wrap rather than run off its edge.
    ui.horizontal_wrapped(|ui| {
        let mut shown = !captions.hidden;
        if ui.checkbox(&mut shown, "Show captions").changed() {
            captions.hidden = !shown;
            change = Some(Change::Commit);
        }
        if ui.button("Import SRT…").clicked() {
            editor.prompt_path(PathPurpose::ImportCaptions);
        }
        let export = ui.add_enabled(!captions.items.is_empty(), egui::Button::new("Export SRT…"));
        if export.clicked() {
            editor.prompt_path(PathPurpose::ExportCaptions);
        }
    });
    change = style_controls(ui, &mut captions).max(change);
    if ui.button("Add Caption at Playhead").clicked() {
        match add_caption(&mut captions, playhead, timebase) {
            Some(start) => {
                editor.captions.focus = Some(start);
                change = Some(Change::Commit);
            }
            None => editor.notice("A caption already shows at the playhead"),
        }
    }
    ui.separator();
    if captions.items.is_empty() {
        ui.label(
            RichText::new("No captions. Add one at the playhead or import an SRT file.").weak(),
        );
    }
    let mut seek = None;
    let mut removed = None;
    let focus = editor.captions.focus.take();
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            for (index, caption) in captions.items.iter_mut().enumerate() {
                let showing = caption.start <= playhead && playhead < caption.end;
                let fill = if showing {
                    ui.visuals().selection.bg_fill.gamma_multiply(0.35)
                } else {
                    Color32::TRANSPARENT
                };
                egui::Frame::group(ui.style()).fill(fill).show(ui, |ui| {
                    ui.push_id(index, |ui| {
                        let row = caption_row(ui, caption, playhead, |time| {
                            Timecode::shown(time + start, timebase, rate, display).to_string()
                        });
                        match row {
                            Some(RowAction::Seek(time)) => seek = Some(time),
                            Some(RowAction::Retimed) => change = Some(Change::Commit),
                            Some(RowAction::Remove) => removed = Some(index),
                            None => {}
                        }
                        let mut field = egui::TextEdit::multiline(&mut caption.text)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY)
                            .show(ui);
                        let text = field.response;
                        if focus == Some(caption.start) {
                            text.request_focus();
                            // A new caption's placeholder is selected so typing replaces it.
                            let all = CCursorRange::two(
                                CCursor::new(0),
                                CCursor::new(caption.text.chars().count()),
                            );
                            field.state.cursor.set_char_range(Some(all));
                            field.state.store(ui.ctx(), text.id);
                        }
                        if text.lost_focus() {
                            change = Some(Change::Commit);
                        } else if text.changed() {
                            change = Some(Change::Live).max(change);
                        }
                    });
                });
            }
        });
    if let Some(index) = removed {
        captions.items.remove(index);
        change = Some(Change::Commit);
    }
    match change {
        Some(Change::Live) => {
            let edit = SetCaptions {
                captions: captions.clone(),
            };
            if let Ok(sequence) = preview(editor.timeline.sequence(), &edit) {
                editor.show_program(sequence);
            }
            editor.captions.draft = Some((revision, captions));
        }
        Some(Change::Commit) => {
            editor.captions.draft = None;
            editor.apply(SetCaptions { captions });
        }
        None => {}
    }
    if let Some(time) = seek {
        editor.seek_program(time);
    }
}

enum RowAction {
    Seek(Ticks),
    Retimed,
    Remove,
}

fn caption_row(
    ui: &mut egui::Ui,
    caption: &mut Caption,
    playhead: Ticks,
    timecode: impl Fn(Ticks) -> String,
) -> Option<RowAction> {
    ui.horizontal_wrapped(|ui| {
        let mut action = None;
        let start = ui.link(RichText::new(timecode(caption.start)).monospace());
        if start.on_hover_text("Go to the caption's start").clicked() {
            action = Some(RowAction::Seek(caption.start));
        }
        ui.label("–");
        let end = ui.link(RichText::new(timecode(caption.end)).monospace());
        if end.on_hover_text("Go to the caption's end").clicked() {
            action = Some(RowAction::Seek(caption.end));
        }
        let set_in = ui.small_button("In").on_hover_text("Start at the playhead");
        if set_in.clicked() {
            caption.start = playhead;
            action = Some(RowAction::Retimed);
        }
        let set_out = ui.small_button("Out").on_hover_text("End at the playhead");
        if set_out.clicked() {
            caption.end = playhead;
            action = Some(RowAction::Retimed);
        }
        if ui.small_button("Delete").clicked() {
            action = Some(RowAction::Remove);
        }
        action
    })
    .inner
}

fn style_controls(ui: &mut egui::Ui, captions: &mut Captions) -> Option<Change> {
    let style = &mut captions.style;
    egui::Grid::new("caption style")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Size");
            let size = ui.add(
                egui::DragValue::new(&mut style.size)
                    .range(12..=200)
                    .suffix(" px"),
            );
            let mut change = response_state(&size, None);
            ui.end_row();
            ui.label("Color");
            change = color_button(ui, &mut style.color).max(change);
            ui.end_row();
            ui.label("Background");
            ui.horizontal(|ui| {
                let mut boxed = style.background.is_some();
                if ui.checkbox(&mut boxed, "").changed() {
                    style.background = boxed.then_some(Rgb::BLACK);
                    change = Some(Change::Commit);
                }
                if let Some(background) = &mut style.background {
                    change = color_button(ui, background).max(change);
                }
            });
            ui.end_row();
            change
        })
        .inner
}

fn color_button(ui: &mut egui::Ui, color: &mut Rgb) -> Option<Change> {
    let mut rgb = [color.r, color.g, color.b];
    ui.color_edit_button_srgb(&mut rgb).changed().then(|| {
        *color = Rgb::new(rgb[0], rgb[1], rgb[2]);
        Change::Commit
    })
}

/// Inserts an empty-looking caption at `at`, cut short by the next one, returning its
/// start; nothing when one already shows there.
fn add_caption(captions: &mut Captions, at: Ticks, timebase: Timebase) -> Option<Ticks> {
    if captions.at(at).is_some() {
        return None;
    }
    let index = captions.index_after(at);
    let length = Ticks(NEW_CAPTION_SECONDS * i64::from(timebase.den()) / i64::from(timebase.num()));
    let end = captions
        .items
        .get(index)
        .map_or(at + length, |next| next.start.min(at + length));
    captions.items.insert(
        index,
        Caption {
            start: at,
            end,
            text: "New caption".to_owned(),
        },
    );
    Some(at)
}
