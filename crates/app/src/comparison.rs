use std::sync::Arc;

use eframe::egui_wgpu::RenderState;
use egui::{Align2, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use playback::FrameProbe;
use timeline::{Sequence, Ticks, Timecode};

use crate::editor::Editor;
use crate::monitor::Monitor;
use crate::viewer::Viewer;

/// Space the reference controls take below the picture.
pub(crate) const CONTROLS_HEIGHT: f32 = 26.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompareLayout {
    SideBySide,
    /// The two pictures meet at a line across, the reference on the left.
    VerticalSplit,
    /// The two pictures meet at a line down, the reference on top.
    HorizontalSplit,
}

impl CompareLayout {
    const ALL: [Self; 3] = [Self::SideBySide, Self::VerticalSplit, Self::HorizontalSplit];

    fn name(self) -> &'static str {
        match self {
            Self::SideBySide => "Side by Side",
            Self::VerticalSplit => "Vertical Split",
            Self::HorizontalSplit => "Horizontal Split",
        }
    }
}

/// Which side of the comparison the reference frame takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReferenceSide {
    First,
    Second,
}

/// Premiere's comparison view: a still reference frame of the sequence beside the
/// current one, for matching shots' color. The reference is composed on a worker.
pub(crate) struct Comparison {
    probe: FrameProbe,
    reference: Monitor,
    time: Ticks,
    layout: CompareLayout,
    side: ReferenceSide,
    /// The share of a split picture the first side covers.
    split: f32,
    /// What the reference was last composed from, so it is asked for once per change.
    asked: Option<(Ticks, Arc<Sequence>)>,
    waiting: bool,
    shown: Option<Ticks>,
}

impl Comparison {
    fn new(render_state: &RenderState, time: Ticks) -> Option<Self> {
        Some(Self {
            probe: FrameProbe::new().ok()?,
            reference: Monitor::new(render_state.clone()),
            time,
            layout: CompareLayout::SideBySide,
            side: ReferenceSide::First,
            split: 0.5,
            asked: None,
            waiting: false,
            shown: None,
        })
    }

    pub(crate) fn reference_time(&self) -> Ticks {
        self.time
    }

    /// Keeps the reference frame up to date with `sequence` and the reference time.
    fn refresh(&mut self, sequence: &Arc<Sequence>) {
        let fresh = self
            .asked
            .as_ref()
            .is_some_and(|(time, asked)| *time == self.time && Arc::ptr_eq(asked, sequence));
        if !fresh {
            self.asked = Some((self.time, sequence.clone()));
            self.probe.request(sequence.clone(), vec![self.time]);
            self.waiting = true;
        }
        let canvas = (sequence.format.width, sequence.format.height);
        if let Some(frame) = self
            .probe
            .latest()
            .and_then(|frames| frames.into_iter().next())
        {
            self.waiting = false;
            if self.reference.present(&frame, canvas).is_ok() {
                self.shown = Some(frame.pts);
            }
        }
    }

    /// Moves the reference by `frames`, within the sequence.
    fn step(&mut self, sequence: &Sequence, frames: i64) {
        let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
        let frame = rate.frame_at(self.time, timebase) + frames;
        self.set_time(sequence, rate.frame_start(frame, timebase));
    }

    fn set_time(&mut self, sequence: &Sequence, time: Ticks) {
        let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
        let time = time.clamp(Ticks::ZERO, sequence.duration());
        self.time = rate.frame_start(rate.frame_at(time, timebase), timebase);
    }

    /// Moves the reference to the next edit after it, or the last one before it.
    fn jump_to_edit(&mut self, sequence: &Sequence, forward: bool) {
        let points = sequence.edit_points();
        let target = if forward {
            points.into_iter().find(|&point| point > self.time)
        } else {
            points.into_iter().rev().find(|&point| point < self.time)
        };
        if let Some(target) = target {
            self.set_time(sequence, target);
        }
    }

    /// Draws the reference and `program`'s current picture into `area`.
    fn picture(&mut self, ui: &egui::Ui, area: Rect, program: &Viewer, playhead: Ticks) {
        let sequence = program.player.sequence().clone();
        self.refresh(&sequence);
        // A frame that can't be shown leaves the last one, as when presenting.
        let _ = self.reference.catch_up(ui.ctx());
        if self.waiting {
            ui.ctx().request_repaint();
        }
        let painter = ui.painter_at(area);
        painter.rect_filled(area, 0.0, Color32::BLACK);
        let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
        let timecode = |time| Timecode::new(time, timebase, rate).to_string();
        let reference_label = format!("Reference {}", timecode(self.shown.unwrap_or(self.time)));
        let current_label = format!("Current {}", timecode(playhead));
        let (first, second) = match self.side {
            ReferenceSide::First => (Picture::Reference, Picture::Current),
            ReferenceSide::Second => (Picture::Current, Picture::Reference),
        };
        let cells = match self.layout {
            CompareLayout::SideBySide => {
                let half = Vec2::new(area.width() / 2.0, area.height());
                let left = Rect::from_min_size(area.min, half);
                let right = Rect::from_min_size(area.min + Vec2::new(half.x, 0.0), half);
                [(left.shrink(2.0), left), (right.shrink(2.0), right)]
            }
            CompareLayout::VerticalSplit | CompareLayout::HorizontalSplit => {
                let [first_part, second_part] = self.split_parts(area);
                [(area, first_part), (area, second_part)]
            }
        };
        let corners = self.label_corners(area);
        let pictures = cells.into_iter().zip(corners).zip([first, second]);
        for (((frame, clip), (at, align)), shown) in pictures {
            let clipped = painter.with_clip_rect(clip);
            match shown {
                Picture::Reference if self.shown.is_some() => {
                    self.reference.paint(&clipped, frame);
                }
                Picture::Reference => {}
                Picture::Current => program.paint(&clipped, frame),
            }
            let label = match shown {
                Picture::Reference => &reference_label,
                Picture::Current => &current_label,
            };
            label_on(&painter, at, align, label);
        }
        if self.layout != CompareLayout::SideBySide {
            self.split_handle(ui, area);
        }
    }

    /// Where each side's label goes: under its picture side by side, and in the outer
    /// corners of a split, where it stays whole however far the split is moved.
    fn label_corners(&self, area: Rect) -> [(Pos2, Align2); 2] {
        let inset = Vec2::splat(6.0);
        match self.layout {
            CompareLayout::SideBySide => {
                let y = area.bottom() - inset.y;
                let quarter = area.width() / 4.0;
                [area.left() + quarter, area.right() - quarter]
                    .map(|x| (Pos2::new(x, y), Align2::CENTER_BOTTOM))
            }
            CompareLayout::VerticalSplit => [
                (
                    area.left_bottom() + Vec2::new(inset.x, -inset.y),
                    Align2::LEFT_BOTTOM,
                ),
                (area.right_bottom() - inset, Align2::RIGHT_BOTTOM),
            ],
            CompareLayout::HorizontalSplit => [
                (area.left_top() + inset, Align2::LEFT_TOP),
                (
                    area.left_bottom() + Vec2::new(inset.x, -inset.y),
                    Align2::LEFT_BOTTOM,
                ),
            ],
        }
    }

    /// The parts of `area` either side of the split.
    fn split_parts(&self, area: Rect) -> [Rect; 2] {
        let mut first = area;
        let mut second = area;
        if self.layout == CompareLayout::VerticalSplit {
            let x = area.left() + area.width() * self.split;
            first.max.x = x;
            second.min.x = x;
        } else {
            let y = area.top() + area.height() * self.split;
            first.max.y = y;
            second.min.y = y;
        }
        [first, second]
    }

    /// The line between the split pictures, which can be dragged to move it.
    fn split_handle(&mut self, ui: &egui::Ui, area: Rect) {
        let [first, _] = self.split_parts(area);
        let vertical = self.layout == CompareLayout::VerticalSplit;
        let (line, cursor) = if vertical {
            let x = first.right();
            let line = [Pos2::new(x, area.top()), Pos2::new(x, area.bottom())];
            (line, CursorIcon::ResizeHorizontal)
        } else {
            let y = first.bottom();
            let line = [Pos2::new(area.left(), y), Pos2::new(area.right(), y)];
            (line, CursorIcon::ResizeVertical)
        };
        let grip = Rect::from_two_pos(line[0], line[1]).expand(4.0);
        let response = ui.interact(
            grip,
            ui.make_persistent_id("comparison split"),
            Sense::drag(),
        );
        if let Some(pos) = response
            .interact_pointer_pos()
            .filter(|_| response.dragged())
        {
            let share = if vertical {
                (pos.x - area.left()) / area.width()
            } else {
                (pos.y - area.top()) / area.height()
            };
            self.split = share.clamp(0.0, 1.0);
        }
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(cursor);
        }
        ui.painter()
            .line_segment(line, Stroke::new(1.5, Color32::from_rgb(250, 210, 60)));
    }

    /// The reference's scrubber and stepping buttons, its layout and swapping sides.
    fn controls(&mut self, ui: &mut egui::Ui, sequence: &Sequence) {
        ui.horizontal(|ui| {
            if ui
                .button("|<")
                .on_hover_text("Reference to previous edit")
                .clicked()
            {
                self.jump_to_edit(sequence, false);
            }
            if ui
                .button("<")
                .on_hover_text("Reference back one frame")
                .clicked()
            {
                self.step(sequence, -1);
            }
            if ui
                .button(">")
                .on_hover_text("Reference forward one frame")
                .clicked()
            {
                self.step(sequence, 1);
            }
            if ui
                .button(">|")
                .on_hover_text("Reference to next edit")
                .clicked()
            {
                self.jump_to_edit(sequence, true);
            }
            let swap = ui
                .button("Swap")
                .on_hover_text("Swap the reference and current sides");
            if swap.clicked() {
                self.side = match self.side {
                    ReferenceSide::First => ReferenceSide::Second,
                    ReferenceSide::Second => ReferenceSide::First,
                };
            }
            egui::ComboBox::from_id_salt("comparison layout")
                .selected_text(self.layout.name())
                .show_ui(ui, |ui| {
                    for layout in CompareLayout::ALL {
                        ui.selectable_value(&mut self.layout, layout, layout.name());
                    }
                });
            let mut time = self.time.0;
            let end = sequence.duration().0.max(1);
            ui.spacing_mut().slider_width = ui.available_width().max(40.0);
            let scrub = ui.add(egui::Slider::new(&mut time, 0..=end).show_value(false));
            if scrub.changed() {
                self.set_time(sequence, Ticks(time));
            }
        });
    }
}

#[derive(Debug, Clone, Copy)]
enum Picture {
    Reference,
    Current,
}

fn label_on(painter: &egui::Painter, at: Pos2, align: Align2, text: &str) {
    let galley = painter.layout_no_wrap(text.to_owned(), FontId::monospace(12.0), Color32::WHITE);
    let rect = align.anchor_size(at, galley.size()).expand(3.0);
    painter.rect_filled(rect, 2.0, Color32::from_black_alpha(160));
    painter.galley(rect.min + Vec2::splat(3.0), galley, Color32::WHITE);
}

impl Editor {
    pub(crate) fn toggle_comparison_view(&mut self) {
        if self.comparison.take().is_some() {
            return;
        }
        let playhead = self.playhead();
        self.comparison = self
            .render_state
            .as_ref()
            .and_then(|render_state| Comparison::new(render_state, playhead));
    }
}

/// The comparison in place of the program picture, with its controls below, when it
/// is on.
pub(crate) fn ui(editor: &mut Editor, ui: &mut egui::Ui, size: Vec2) -> Option<egui::Response> {
    let playhead = editor.playhead();
    let (Some(comparison), Some(program)) = (&mut editor.comparison, &editor.program) else {
        return None;
    };
    let size = Vec2::new(size.x, (size.y - CONTROLS_HEIGHT).max(40.0));
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    comparison.picture(ui, rect, program, playhead);
    let sequence = program.player.sequence().clone();
    comparison.controls(ui, &sequence);
    Some(response)
}
