use egui::{ComboBox, DragValue, Key};
use timeline::{
    AlphaInterpretation, ColorSpace, FieldOrder, FrameRate, InterpretFootage, PixelAspect, SourceId,
};

use crate::editor::Editor;

/// Rates footage is commonly shot or delivered at.
const COMMON_RATES: [(&str, u32, u32); 10] = [
    ("23.976", 24_000, 1_001),
    ("24", 24, 1),
    ("25", 25, 1),
    ("29.97", 30_000, 1_001),
    ("30", 30, 1),
    ("48", 48, 1),
    ("50", 50, 1),
    ("59.94", 60_000, 1_001),
    ("60", 60, 1),
    ("120", 120, 1),
];

/// How an item's pictures are read, chosen in Premiere's Interpret Footage dialog: the
/// frame rate and pixel shape the file gives or ones assumed in their place, and what
/// its alpha channel means and which color space its pictures are in.
pub(crate) struct InterpretDialog {
    source: SourceId,
    name: String,
    native: FrameRate,
    assume: bool,
    rate: FrameRate,
    native_aspect: PixelAspect,
    conform: bool,
    aspect: PixelAspect,
    native_fields: FieldOrder,
    conform_fields: bool,
    fields: FieldOrder,
    alpha: AlphaInterpretation,
    native_color: ColorSpace,
    override_color: bool,
    color: ColorSpace,
}

impl Editor {
    pub(crate) fn open_interpret_footage(&mut self, source: SourceId) {
        let Some(item) = self.timeline.project().media_item(source) else {
            return;
        };
        let Some(video) = item.video.filter(|_| item.is_file()) else {
            self.notice("Only video files can be interpreted");
            return;
        };
        self.interpret_dialog = Some(InterpretDialog {
            source,
            name: item.name.clone(),
            native: video.frame_rate,
            assume: item.interpreted_rate.is_some(),
            rate: item.interpreted_rate.unwrap_or(video.frame_rate),
            native_aspect: video.pixel_aspect,
            conform: item.interpreted_aspect.is_some(),
            aspect: item.pixel_aspect(),
            native_fields: video.field_order,
            conform_fields: item.interpreted_fields.is_some(),
            fields: item.field_order(),
            alpha: item.alpha,
            native_color: video.color_space,
            override_color: item.interpreted_color.is_some(),
            color: item.color_space(),
        });
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(dialog) = &mut editor.interpret_dialog else {
        return;
    };
    let mut done = false;
    let mut cancelled = false;
    egui::Window::new("Interpret Footage")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.strong(&dialog.name);
            ui.separator();
            ui.label("Frame Rate");
            let native = format!(
                "Use frame rate from file: {} fps",
                fps(dialog.native.approx_fps())
            );
            ui.radio_value(&mut dialog.assume, false, native);
            ui.horizontal(|ui| {
                ui.radio_value(&mut dialog.assume, true, "Assume this frame rate:");
                ui.add_enabled_ui(dialog.assume, |ui| rate_picker(ui, &mut dialog.rate));
            });
            ui.weak("Clips keep their frames, so they lengthen or shorten, and the sound follows.");
            ui.separator();
            ui.label("Pixel Aspect Ratio");
            let native = format!(
                "Use pixel aspect ratio from file: {}",
                aspect_name(dialog.native_aspect)
            );
            ui.radio_value(&mut dialog.conform, false, native);
            ui.horizontal(|ui| {
                ui.radio_value(&mut dialog.conform, true, "Conform to:");
                ui.add_enabled_ui(dialog.conform, |ui| aspect_picker(ui, &mut dialog.aspect));
            });
            ui.separator();
            ui.label("Field Order");
            let native = format!("Use field order from file: {}", dialog.native_fields.name());
            ui.radio_value(&mut dialog.conform_fields, false, native);
            ui.horizontal(|ui| {
                ui.radio_value(&mut dialog.conform_fields, true, "Conform to:");
                ui.add_enabled_ui(dialog.conform_fields, |ui| {
                    ComboBox::from_id_salt("conformed field order")
                        .selected_text(dialog.fields.name())
                        .show_ui(ui, |ui| {
                            for order in FieldOrder::ALL {
                                ui.selectable_value(&mut dialog.fields, order, order.name());
                            }
                        });
                });
            });
            ui.weak("Interlaced pictures are rebuilt from their first field.");
            ui.separator();
            ui.label("Alpha Channel");
            for alpha in AlphaInterpretation::ALL {
                ui.radio_value(&mut dialog.alpha, alpha, alpha.name());
            }
            ui.separator();
            ui.label("Color Management");
            let native = format!("Use color space from file: {}", dialog.native_color.name());
            ui.radio_value(&mut dialog.override_color, false, native);
            ui.horizontal(|ui| {
                ui.radio_value(&mut dialog.override_color, true, "Override to:");
                ui.add_enabled_ui(dialog.override_color, |ui| {
                    ComboBox::from_id_salt("interpreted color space")
                        .selected_text(dialog.color.name())
                        .show_ui(ui, |ui| {
                            for space in ColorSpace::ALL {
                                ui.selectable_value(&mut dialog.color, space, space.name());
                            }
                        });
                });
            });
            ui.weak("HDR pictures are tone mapped into the Rec. 709 working space.");
            ui.separator();
            ui.horizontal(|ui| {
                done = ui.button("OK").clicked() || ui.input(|input| input.key_pressed(Key::Enter));
                cancelled = ui.button("Cancel").clicked()
                    || ui.input(|input| input.key_pressed(Key::Escape));
            });
        });
    if !(done || cancelled) {
        return;
    }
    let Some(dialog) = editor.interpret_dialog.take() else {
        return;
    };
    if done {
        editor.interpret_footage(InterpretFootage {
            source: dialog.source,
            rate: dialog.assume.then_some(dialog.rate),
            aspect: dialog.conform.then_some(dialog.aspect),
            fields: dialog.conform_fields.then_some(dialog.fields),
            alpha: dialog.alpha,
            color: dialog.override_color.then_some(dialog.color),
        });
    }
}

pub(crate) fn rate_picker(ui: &mut egui::Ui, rate: &mut FrameRate) {
    let shown = COMMON_RATES
        .iter()
        .find(|&&(_, num, den)| FrameRate::new(num, den) == Some(*rate))
        .map_or("Custom", |(name, ..)| name);
    ComboBox::from_id_salt("interpreted rate")
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (name, num, den) in COMMON_RATES {
                if let Some(common) = FrameRate::new(num, den) {
                    ui.selectable_value(rate, common, name);
                }
            }
        });
    let mut whole = rate.approx_fps().round() as u32;
    let custom = DragValue::new(&mut whole).range(1..=480).suffix(" fps");
    if ui.add(custom).changed() {
        if let Some(chosen) = FrameRate::new(whole, 1) {
            *rate = chosen;
        }
    }
}

fn aspect_name(aspect: PixelAspect) -> String {
    PixelAspect::PRESETS
        .iter()
        .find(|&&(_, preset)| preset == aspect)
        .map_or_else(
            || format!("{:.4}", aspect.ratio()),
            |(name, _)| (*name).to_owned(),
        )
}

fn aspect_picker(ui: &mut egui::Ui, aspect: &mut PixelAspect) {
    ComboBox::from_id_salt("conformed pixel aspect")
        .selected_text(aspect_name(*aspect))
        .show_ui(ui, |ui| {
            for (name, preset) in PixelAspect::PRESETS {
                ui.selectable_value(aspect, preset, name);
            }
        });
}

/// Up to three decimals, without trailing zeros.
fn fps(rate: f64) -> String {
    let text = format!("{rate:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}
