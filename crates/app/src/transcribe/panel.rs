//! The Text panel: the sequence's words as paragraphs, to follow along, jump by
//! clicking, cut by deleting words, and make captions from.

use std::ops::Range;

use egui::{Align, Key, Modifiers, ProgressBar, ScrollArea, Sense, TextEdit, Vec2};
use timeline::{
    captions_from_words, spoken_words, CaptionRules, Captions, DeleteText, Marks, SetCaptions,
    SetMarks, Ticks, Timebase, Timecode,
};

use super::text_layout::TextLayout;
use crate::editor::Editor;
use crate::theme;

/// Room left after the last word kept by a deletion, in milliseconds, so the cut
/// doesn't clip it.
const KEEP_AFTER: i64 = 250;

#[derive(Default)]
pub(crate) struct TextPanel {
    layout: Option<TextLayout>,
    /// Word indices.
    selection: Option<Range<usize>>,
    anchor: Option<usize>,
    search: String,
    /// The panel was clicked last, so Delete deletes its words.
    focused: bool,
    /// The word last scrolled into view while playing.
    followed: Option<usize>,
    /// Drawn since the editor last took keys: a hidden panel keeps its selection and
    /// focus but mustn't take Delete from the timeline.
    drawn: bool,
}

pub(crate) fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    header(editor, ui);
    ui.separator();
    editor.text_panel.drawn = true;
    let revision = editor.timeline.revision();
    let width = (ui.available_width() - 16.0).max(80.0);
    let stale = editor
        .text_panel
        .layout
        .as_ref()
        .is_none_or(|layout| layout.revision != revision || layout.width != width);
    if stale {
        let sequence = editor.timeline.sequence();
        let (timebase, rate) = (sequence.timebase, sequence.format.frame_rate);
        let (start, display) = (sequence.start_time, sequence.time_display);
        let label =
            |time: Ticks| Timecode::shown(start + time, timebase, rate, display).to_string();
        let colors = (theme::TEXT, theme::WEAK_TEXT);
        let layout = TextLayout::new(ui.painter(), sequence, (revision, width), colors, &label);
        let panel = &mut editor.text_panel;
        if panel.layout.as_ref().map(|old| old.words.len()) != Some(layout.words.len()) {
            panel.selection = None;
            panel.anchor = None;
        }
        panel.layout = Some(layout);
    }
    let words = editor
        .text_panel
        .layout
        .as_ref()
        .map_or(0, |l| l.words.len());
    if words == 0 {
        ui.add_space(8.0);
        ui.weak(if editor.transcribing.is_some() {
            "The words appear here once the media is transcribed."
        } else {
            "Transcribe the sequence to read along with it, cut it by deleting words, and \
             make captions from what's said."
        });
        return;
    }
    if editor.text_panel.selection.is_some() {
        selection_actions(editor, ui);
    }
    body(editor, ui);
}

fn header(editor: &mut Editor, ui: &mut egui::Ui) {
    if let Some(transcribing) = &editor.transcribing {
        let (status, progress) = transcribing.status(editor);
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                editor.cancel_transcribing();
            }
            ui.add(ProgressBar::new(progress).show_percentage().text(status));
        });
        return;
    }
    ui.horizontal(|ui| {
        if ui
            .button("Transcribe…")
            .on_hover_text("Turn what's said in the sequence into text, on this computer")
            .clicked()
        {
            editor.open_transcribe();
        }
        let has_words = editor
            .text_panel
            .layout
            .as_ref()
            .is_some_and(|layout| !layout.words.is_empty());
        if ui
            .add_enabled(has_words, egui::Button::new("Create Captions"))
            .on_hover_text("Make the sequence's captions from its words, replacing any it has")
            .clicked()
        {
            editor.create_captions();
        }
        ui.add(
            TextEdit::singleline(&mut editor.text_panel.search)
                .hint_text("Find")
                .desired_width(f32::INFINITY),
        );
    });
}

fn selection_actions(editor: &mut Editor, ui: &mut egui::Ui) {
    let Some(selection) = editor.text_panel.selection.clone() else {
        return;
    };
    ui.horizontal(|ui| {
        let count = selection.len();
        ui.weak(if count == 1 {
            "1 word".to_owned()
        } else {
            format!("{count} words")
        });
        if ui
            .button("Delete")
            .on_hover_text("Cut these words out of the sequence and close the gap (Delete)")
            .clicked()
        {
            editor.delete_words(selection.clone());
        }
        if ui
            .button("Mark In and Out")
            .on_hover_text("Set the sequence's In and Out around these words")
            .clicked()
        {
            editor.mark_words(selection.clone());
        }
    });
}

fn body(editor: &mut Editor, ui: &mut egui::Ui) {
    let playhead = editor.playhead();
    let playing = editor
        .program
        .as_ref()
        .is_some_and(|program| program.player.is_playing());
    let pressed_anywhere = ui.input(|input| input.pointer.any_pressed());
    let mut seek = None;
    let panel = &mut editor.text_panel;
    let Some(layout) = &panel.layout else {
        return;
    };
    let playing_word = layout.word_playing(playhead);
    let needle = panel.search.trim().to_lowercase();
    let output = ScrollArea::vertical()
        .auto_shrink(false)
        .show_viewport(ui, |ui, viewport| {
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), layout.height),
                Sense::click_and_drag(),
            );
            let origin = rect.min + Vec2::new(4.0, 0.0);
            let word_under = |at: egui::Pos2| layout.word_at((at - origin).to_pos2());
            if let Some(at) = response.interact_pointer_pos() {
                if response.drag_started() || response.clicked() {
                    let extend = ui.input(|input| input.modifiers.shift) && panel.anchor.is_some();
                    match word_under(at) {
                        Some(_) if extend => {}
                        Some(word) => panel.anchor = Some(word),
                        None => {
                            panel.anchor = None;
                            panel.selection = None;
                        }
                    }
                }
                if let (Some(anchor), Some(word)) = (panel.anchor, word_under(at)) {
                    panel.selection = Some(anchor.min(word)..anchor.max(word) + 1);
                    if response.clicked() {
                        seek = Some(layout.words[word].start);
                    }
                }
            }
            if playing && playing_word != panel.followed {
                panel.followed = playing_word;
                if let Some(rect) = playing_word.and_then(|word| layout.word_rect(word)) {
                    ui.scroll_to_rect(rect.translate(origin.to_vec2()), Some(Align::Center));
                }
            }
            let painter = ui.painter_at(rect);
            let visible = viewport.translate(rect.min.to_vec2());
            let first = layout
                .paragraphs
                .partition_point(|paragraph| rect.min.y + paragraph.bottom() < visible.min.y);
            for paragraph in &layout.paragraphs[first..] {
                if rect.min.y + paragraph.top > visible.max.y {
                    break;
                }
                let mark = |word: usize, color| {
                    if let Some(word_rect) = layout.word_rect(word) {
                        painter.rect_filled(word_rect.translate(origin.to_vec2()), 2.0, color);
                    }
                };
                for word in paragraph.words.clone() {
                    if panel
                        .selection
                        .as_ref()
                        .is_some_and(|range| range.contains(&word))
                    {
                        mark(word, theme::ACCENT_FILL);
                    } else if !needle.is_empty() && layout.words[word].lowercase.contains(&needle) {
                        mark(word, egui::Color32::from_rgb(110, 90, 20));
                    } else if playing_word == Some(word) {
                        mark(word, theme::RAISED);
                    }
                }
                painter.galley(
                    origin + Vec2::new(0.0, paragraph.top),
                    paragraph.time.clone(),
                    theme::WEAK_TEXT,
                );
                painter.galley(
                    origin + Vec2::new(0.0, paragraph.text_top()),
                    paragraph.text.clone(),
                    theme::TEXT,
                );
            }
            response
        });
    let response = output.inner;
    if pressed_anywhere {
        panel.focused = response.is_pointer_button_down_on() || response.clicked();
    }
    if let Some(time) = seek {
        editor.seek_program(time);
    }
}

/// Delete and Escape for the panel's selected words, taken before the editor's own
/// keys so they don't also act on the timeline.
pub(crate) fn take_keys(editor: &mut Editor, ctx: &egui::Context) {
    let revision = editor.timeline.revision();
    let panel = &mut editor.text_panel;
    let drawn = std::mem::take(&mut panel.drawn);
    // Word indices only mean the words laid out at the current revision.
    let current = panel
        .layout
        .as_ref()
        .is_some_and(|layout| layout.revision == revision);
    let Some(selection) = panel
        .selection
        .clone()
        .filter(|_| panel.focused && drawn && current)
    else {
        return;
    };
    let deleting = ctx.input_mut(|input| {
        input.consume_key(Modifiers::NONE, Key::Delete)
            | input.consume_key(Modifiers::NONE, Key::Backspace)
    });
    if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
        panel.selection = None;
    } else if deleting {
        editor.delete_words(selection);
    }
}

impl Editor {
    pub(crate) fn create_captions(&mut self) {
        let sequence = self.timeline.sequence();
        let seconds = |count: i64| {
            Ticks(count).rescale(
                Timebase::new(1, 1).unwrap_or(Timebase::HZ_90K),
                sequence.timebase,
            )
        };
        let rules = CaptionRules {
            line_chars: 42,
            lines: 2,
            longest: seconds(6),
            pause: seconds(1),
        };
        let items = captions_from_words(&spoken_words(sequence), rules);
        if items.is_empty() {
            self.notice("No words to make captions from");
            return;
        }
        let captions = Captions {
            items,
            ..sequence.captions.clone()
        };
        if self.apply(SetCaptions { captions }) {
            self.notice("Captions created from the transcript");
        }
    }

    /// Cuts the stretch where `words` are said out of every track and closes it up,
    /// keeping the frames where the words around them are said.
    pub(crate) fn delete_words(&mut self, words: Range<usize>) {
        let Some(span) = self.words_span(words) else {
            return;
        };
        let sequence = self.timeline.sequence();
        let tracks = sequence.tracks().map(|(track, _)| track).collect();
        self.text_panel.selection = None;
        self.apply(DeleteText {
            spans: vec![span],
            tracks,
        });
    }

    pub(crate) fn mark_words(&mut self, words: Range<usize>) {
        let Some(span) = self.words_span(words) else {
            return;
        };
        let marks = Marks {
            mark_in: Some(span.start),
            mark_out: Some(span.end),
        };
        self.apply_levels(SetMarks { marks });
    }

    /// Whole frames from the first word's start to the last's end, running on into the
    /// pause after it for a moment unless the next word starts sooner.
    fn words_span(&self, words: Range<usize>) -> Option<Range<Ticks>> {
        let layout = self.text_panel.layout.as_ref()?;
        let first = layout.words.get(words.start)?;
        let last = layout.words.get(words.end.checked_sub(1)?)?;
        let sequence = self.timeline.sequence();
        let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
        let millisecond = Timebase::new(1, 1_000).unwrap_or(Timebase::HZ_90K);
        let mut end = last.end + Ticks(KEEP_AFTER).rescale(millisecond, timebase);
        if let Some(next) = layout.words.get(words.end) {
            end = end.min(next.start).max(last.end);
        }
        let start = rate.frame_start(rate.frame_at(first.start, timebase), timebase);
        let end_frame = rate.frame_at(end, timebase);
        let end = if rate.frame_start(end_frame, timebase) < end {
            rate.frame_start(end_frame + 1, timebase)
        } else {
            end
        };
        (start < end).then_some(start..end)
    }
}
