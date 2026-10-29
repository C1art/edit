use timeline::{
    CountingLeader, Generator, Rgb, Scroll, ShadowStyle, Shape, ShapeKind, TextAlign, TextStroke,
    Title, TitleFont, TitleMotion,
};

use super::graphic_layers;
use crate::installed_fonts;
use crate::mixer::{response_state, Change};

/// What a title, shape or color matte draws on a frame `height` pixels tall. Typing
/// previews live and commits when the text field loses focus.
pub(super) fn controls(
    ui: &mut egui::Ui,
    generator: &mut Generator,
    height: u32,
) -> Option<Change> {
    if matches!(
        generator,
        Generator::AdjustmentLayer
            | Generator::BarsAndTone
            | Generator::BlackVideo
            | Generator::TransparentVideo
    ) {
        return None;
    }
    let mut change = None;
    egui::CollapsingHeader::new(generator.kind_name())
        .default_open(true)
        .show(ui, |ui| match generator {
            Generator::ColorMatte { color } => {
                change = color_row(ui, "Color", color);
            }
            Generator::Title(title) => {
                change = title_controls(ui, title);
                change = graphic_layers::upgrade_row(ui, generator).or(change);
            }
            Generator::AdjustmentLayer
            | Generator::BarsAndTone
            | Generator::BlackVideo
            | Generator::TransparentVideo => {}
            Generator::Shape(shape) => {
                change = shape_controls(ui, shape, height);
                change = graphic_layers::upgrade_row(ui, generator).or(change);
            }
            Generator::Graphic(graphic) => change = graphic_layers::controls(ui, graphic, height),
            Generator::CountingLeader(leader) => change = leader_controls(ui, leader),
        });
    change
}

fn title_controls(ui: &mut egui::Ui, title: &mut Title) -> Option<Change> {
    let change = text_controls(ui, title);
    motion_controls(ui, &mut title.motion).or(change)
}

/// A title's look, without its motion.
pub(super) fn text_controls(ui: &mut egui::Ui, title: &mut Title) -> Option<Change> {
    let mut change = None;
    // Effect Controls is often narrow, so rows here wrap rather than run off its edge.
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
    pairs(ui, 245.0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Size");
            let size = ui.add(
                egui::DragValue::new(&mut title.size)
                    .range(4..=1000)
                    .suffix(" px"),
            );
            change = response_state(&size, change);
        });
        ui.horizontal(|ui| {
            if font_picker(ui, &mut title.font) {
                change = Some(Change::Commit);
            }
            for (on, label) in [
                (&mut title.bold, egui::RichText::new("B").strong()),
                (&mut title.italic, egui::RichText::new("I").italics()),
            ] {
                if ui.toggle_value(on, label).changed() {
                    change = Some(Change::Commit);
                }
            }
        });
    });
    pairs(ui, 200.0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Tracking");
            let tracking = ui.add(egui::DragValue::new(&mut title.tracking).range(-200..=1000));
            change = response_state(&tracking, change);
        });
        ui.horizontal(|ui| {
            ui.label("Leading");
            let leading = ui.add(
                egui::DragValue::new(&mut title.leading)
                    .range(-500..=500)
                    .suffix(" px"),
            );
            change = response_state(&leading, change);
        });
    });
    ui.horizontal_wrapped(|ui| {
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
    let usual = title.usual_shadow();
    change = shadow_rows(ui, &mut title.shadow, &mut title.shadow_style, usual).or(change);
    ui.horizontal_wrapped(|ui| {
        let mut boxed = title.background.is_some();
        if ui.checkbox(&mut boxed, "Background").changed() {
            title.background = boxed.then_some(Rgb::BLACK);
            change = Some(Change::Commit);
        }
        if let Some(background) = &mut title.background {
            change = color_button(ui, background).or(change);
        }
    });
    change
}

/// Roll and crawl, with where the title starts and ends and its timing in frames.
fn motion_controls(ui: &mut egui::Ui, motion: &mut TitleMotion) -> Option<Change> {
    let mut change = None;
    let scroll = motion.scroll().unwrap_or_default();
    ui.horizontal_wrapped(|ui| {
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
    ui.horizontal_wrapped(|ui| {
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

pub(super) fn shape_controls(ui: &mut egui::Ui, shape: &mut Shape, height: u32) -> Option<Change> {
    let mut change = None;
    ui.horizontal_wrapped(|ui| {
        for kind in ShapeKind::ALL {
            if ui
                .selectable_value(&mut shape.kind, kind, kind.name())
                .changed()
            {
                change = Some(Change::Commit);
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        for (label, length) in [("Width", &mut shape.width), ("Height", &mut shape.height)] {
            ui.label(label);
            let drag = ui.add(egui::DragValue::new(length).range(1..=8000).suffix(" px"));
            change = response_state(&drag, change);
        }
    });
    if shape.kind == ShapeKind::Rectangle {
        ui.horizontal_wrapped(|ui| {
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
    ui.horizontal_wrapped(|ui| {
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
    let usual = Shape::usual_shadow(height);
    change = shadow_rows(ui, &mut shape.shadow, &mut shape.shadow_style, usual).or(change);
    change
}

fn stroke_row(ui: &mut egui::Ui, stroke: &mut Option<TextStroke>) -> Option<Change> {
    let mut change = None;
    ui.horizontal_wrapped(|ui| {
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

/// Essential Graphics' shadow: until one of its controls is changed, the `usual` look
/// that scales with what casts it.
fn shadow_rows(
    ui: &mut egui::Ui,
    shadow: &mut bool,
    style: &mut Option<ShadowStyle>,
    usual: ShadowStyle,
) -> Option<Change> {
    let mut change = None;
    let mut edited = style.unwrap_or(usual);
    ui.horizontal_wrapped(|ui| {
        if ui.checkbox(shadow, "Drop shadow").changed() {
            change = Some(Change::Commit);
        }
        if *shadow {
            change = color_button(ui, &mut edited.color).or(change);
            let opacity = egui::DragValue::new(&mut edited.opacity)
                .range(0..=100)
                .suffix("%");
            change = response_state(&ui.add(opacity).on_hover_text("Opacity"), change);
            let angle = egui::DragValue::new(&mut edited.angle)
                .range(-360..=360)
                .suffix("°");
            change = response_state(&ui.add(angle).on_hover_text("Angle"), change);
        }
    });
    if *shadow {
        pairs(ui, 255.0, |ui| {
            for (value, name) in [
                (&mut edited.distance, "Distance"),
                (&mut edited.size, "Size"),
                (&mut edited.blur, "Blur"),
            ] {
                ui.horizontal(|ui| {
                    ui.label(name);
                    let drag = egui::DragValue::new(value).range(0..=200).suffix(" px");
                    change = response_state(&ui.add(drag), change);
                });
            }
        });
    }
    if edited != style.unwrap_or(usual) {
        *style = Some(edited);
    }
    change
}

/// Lays out groups of controls side by side, or one under another when the panel is
/// narrower than `wide`; a wrapping row would part a label from its value.
fn pairs(ui: &mut egui::Ui, wide: f32, add: impl FnOnce(&mut egui::Ui)) {
    if ui.available_width() >= wide {
        ui.horizontal(add);
    } else {
        ui.vertical(add);
    }
}

fn leader_controls(ui: &mut egui::Ui, leader: &mut CountingLeader) -> Option<Change> {
    let mut change = None;
    for (label, color) in [
        ("Wipe Color", &mut leader.wipe_color),
        ("Background Color", &mut leader.background_color),
        ("Line Color", &mut leader.line_color),
        ("Target Color", &mut leader.target_color),
        ("Numeral Color", &mut leader.numeral_color),
    ] {
        change = color_row(ui, label, color).or(change);
    }
    for (on, label) in [
        (&mut leader.blip_on_two, "Cue Blip on 2"),
        (
            &mut leader.blip_every_second,
            "Cue Blip at all Second Starts",
        ),
    ] {
        if ui.checkbox(on, label).changed() {
            change = Some(Change::Commit);
        }
    }
    change
}

fn color_row(ui: &mut egui::Ui, label: &str, color: &mut Rgb) -> Option<Change> {
    ui.horizontal_wrapped(|ui| {
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

/// egui's two fonts, then every installed one; true when another was picked.
fn font_picker(ui: &mut egui::Ui, font: &mut TitleFont) -> bool {
    let name = |font: &TitleFont| match font {
        TitleFont::File(path) => {
            installed_fonts::name_of(path).map_or_else(|| font.name(), str::to_owned)
        }
        _ => font.name(),
    };
    let mut picked = false;
    egui::ComboBox::from_id_salt("title_font")
        .selected_text(name(font))
        .height(400.0)
        .show_ui(ui, |ui| {
            for builtin in [TitleFont::Sans, TitleFont::Mono] {
                let label = name(&builtin);
                picked |= ui.selectable_value(font, builtin, label).changed();
            }
            ui.separator();
            match installed_fonts::list() {
                Some(installed) => {
                    for installed in installed {
                        let file = TitleFont::File(installed.path.clone());
                        picked |= ui.selectable_value(font, file, &installed.name).changed();
                    }
                }
                None => {
                    ui.weak("Finding installed fonts…");
                    ui.ctx().request_repaint();
                }
            }
        });
    picked
}
