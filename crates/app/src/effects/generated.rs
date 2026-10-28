use timeline::{
    Generator, Rgb, Scroll, Shape, ShapeKind, TextAlign, TextStroke, Title, TitleFont, TitleMotion,
};

use crate::mixer::{response_state, Change};

/// What a title, shape or color matte draws. Typing previews live and commits when the text
/// field loses focus.
pub(super) fn controls(ui: &mut egui::Ui, generator: &mut Generator) -> Option<Change> {
    if *generator == Generator::AdjustmentLayer {
        return None;
    }
    let mut change = None;
    egui::CollapsingHeader::new(generator.kind_name())
        .default_open(true)
        .show(ui, |ui| match generator {
            Generator::ColorMatte { color } => {
                change = color_row(ui, "Color", color);
            }
            Generator::Title(title) => change = title_controls(ui, title),
            Generator::AdjustmentLayer => {}
            Generator::Shape(shape) => change = shape_controls(ui, shape),
        });
    change
}

fn title_controls(ui: &mut egui::Ui, title: &mut Title) -> Option<Change> {
    let mut change = None;
    let text = ui.add(
        egui::TextEdit::multiline(&mut title.text)
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );
    if text.changed() {
        change = Some(Change::Live);
    }
    if text.lost_focus() {
        change = Some(Change::Commit);
    }
    ui.horizontal(|ui| {
        ui.label("Size");
        let size = ui.add(
            egui::DragValue::new(&mut title.size)
                .range(4..=1000)
                .suffix(" px"),
        );
        change = response_state(&size, change);
        egui::ComboBox::from_id_salt("title_font")
            .selected_text(font_name(title.font))
            .show_ui(ui, |ui| {
                for font in [TitleFont::Sans, TitleFont::Mono] {
                    if ui
                        .selectable_value(&mut title.font, font, font_name(font))
                        .changed()
                    {
                        change = Some(Change::Commit);
                    }
                }
            });
        for (on, label) in [
            (&mut title.bold, egui::RichText::new("B").strong()),
            (&mut title.italic, egui::RichText::new("I").italics()),
        ] {
            if ui.toggle_value(on, label).changed() {
                change = Some(Change::Commit);
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Tracking");
        let tracking = ui.add(egui::DragValue::new(&mut title.tracking).range(-200..=1000));
        change = response_state(&tracking, change);
        ui.label("Leading");
        let leading = ui.add(
            egui::DragValue::new(&mut title.leading)
                .range(-500..=500)
                .suffix(" px"),
        );
        change = response_state(&leading, change);
    });
    ui.horizontal(|ui| {
        ui.label("Align");
        for (align, label) in [
            (TextAlign::Left, "Left"),
            (TextAlign::Center, "Center"),
            (TextAlign::Right, "Right"),
        ] {
            if ui
                .selectable_value(&mut title.align, align, label)
                .changed()
            {
                change = Some(Change::Commit);
            }
        }
    });
    change = color_row(ui, "Fill", &mut title.color).or(change);
    change = stroke_row(ui, &mut title.stroke).or(change);
    if ui.checkbox(&mut title.shadow, "Drop shadow").changed() {
        change = Some(Change::Commit);
    }
    ui.horizontal(|ui| {
        let mut boxed = title.background.is_some();
        if ui.checkbox(&mut boxed, "Background").changed() {
            title.background = boxed.then_some(Rgb::BLACK);
            change = Some(Change::Commit);
        }
        if let Some(background) = &mut title.background {
            change = color_button(ui, background).or(change);
        }
    });
    motion_controls(ui, &mut title.motion).or(change)
}

/// Roll and crawl, with where the title starts and ends and its timing in frames.
fn motion_controls(ui: &mut egui::Ui, motion: &mut TitleMotion) -> Option<Change> {
    let mut change = None;
    let scroll = motion.scroll().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Motion");
        egui::ComboBox::from_id_salt("title_motion")
            .selected_text(motion.name())
            .show_ui(ui, |ui| {
                for choice in [
                    TitleMotion::Still,
                    TitleMotion::Roll(scroll),
                    TitleMotion::CrawlLeft(scroll),
                    TitleMotion::CrawlRight(scroll),
                ] {
                    if ui.selectable_value(motion, choice, choice.name()).changed() {
                        change = Some(Change::Commit);
                    }
                }
            });
    });
    let Some(mut scroll) = motion.scroll() else {
        return change;
    };
    ui.horizontal(|ui| {
        for (on, label) in [
            (&mut scroll.start_off_screen, "Start Off Screen"),
            (&mut scroll.end_off_screen, "End Off Screen"),
        ] {
            if ui.checkbox(on, label).changed() {
                change = Some(Change::Commit);
            }
        }
    });
    egui::Grid::new("title_scroll_timing").show(ui, |ui| {
        let Scroll {
            preroll,
            ease_in,
            ease_out,
            postroll,
            ..
        } = &mut scroll;
        for (index, (label, frames)) in [
            ("Preroll", preroll),
            ("Ease-In", ease_in),
            ("Ease-Out", ease_out),
            ("Postroll", postroll),
        ]
        .into_iter()
        .enumerate()
        {
            ui.label(label);
            let drag = ui.add(
                egui::DragValue::new(frames)
                    .range(0..=100_000)
                    .suffix(" fr"),
            );
            change = response_state(&drag, change);
            if index % 2 == 1 {
                ui.end_row();
            }
        }
    });
    *motion = motion.with_scroll(scroll);
    change
}

fn shape_controls(ui: &mut egui::Ui, shape: &mut Shape) -> Option<Change> {
    let mut change = None;
    ui.horizontal(|ui| {
        for kind in ShapeKind::ALL {
            if ui
                .selectable_value(&mut shape.kind, kind, kind.name())
                .changed()
            {
                change = Some(Change::Commit);
            }
        }
    });
    ui.horizontal(|ui| {
        for (label, length) in [("Width", &mut shape.width), ("Height", &mut shape.height)] {
            ui.label(label);
            let drag = ui.add(egui::DragValue::new(length).range(1..=8000).suffix(" px"));
            change = response_state(&drag, change);
        }
    });
    if shape.kind == ShapeKind::Rectangle {
        ui.horizontal(|ui| {
            ui.label("Corner radius");
            let limit = shape.width.min(shape.height) / 2;
            let radius = ui.add(
                egui::DragValue::new(&mut shape.corner_radius)
                    .range(0..=limit)
                    .suffix(" px"),
            );
            change = response_state(&radius, change);
        });
    }
    ui.horizontal(|ui| {
        let mut filled = shape.fill.is_some();
        if ui.checkbox(&mut filled, "Fill").changed() {
            shape.fill = filled.then_some(Rgb::WHITE);
            change = Some(Change::Commit);
        }
        if let Some(fill) = &mut shape.fill {
            change = color_button(ui, fill).or(change);
        }
    });
    change = stroke_row(ui, &mut shape.stroke).or(change);
    if ui.checkbox(&mut shape.shadow, "Drop shadow").changed() {
        change = Some(Change::Commit);
    }
    change
}

fn stroke_row(ui: &mut egui::Ui, stroke: &mut Option<TextStroke>) -> Option<Change> {
    let mut change = None;
    ui.horizontal(|ui| {
        let mut stroked = stroke.is_some();
        if ui.checkbox(&mut stroked, "Stroke").changed() {
            *stroke = stroked.then(TextStroke::default);
            change = Some(Change::Commit);
        }
        if let Some(stroke) = stroke {
            change = color_button(ui, &mut stroke.color).or(change);
            let width = ui.add(
                egui::DragValue::new(&mut stroke.width)
                    .range(1..=100)
                    .suffix(" px"),
            );
            change = response_state(&width, change);
        }
    });
    change
}

fn color_row(ui: &mut egui::Ui, label: &str, color: &mut Rgb) -> Option<Change> {
    ui.horizontal(|ui| {
        ui.label(label);
        color_button(ui, color)
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

fn font_name(font: TitleFont) -> &'static str {
    match font {
        TitleFont::Sans => "Sans",
        TitleFont::Mono => "Mono",
    }
}
