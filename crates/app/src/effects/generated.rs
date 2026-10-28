use timeline::{Generator, Rgb, TextAlign, Title, TitleFont};

use crate::mixer::{response_state, Change};

/// What a title or color matte draws. Typing previews live and commits when the text
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
