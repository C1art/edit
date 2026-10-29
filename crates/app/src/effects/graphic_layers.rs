use std::path::PathBuf;

use timeline::{
    Generator, Graphic, GraphicContent, GraphicLayer, Shape, ShapeKind, Title, TitleMotion,
};

use super::generated::{pairs, shape_controls, text_controls};
use crate::mixer::{response_state, Change};

/// A new text layer, a shape layer of a kind, or a picture from a file still to be
/// named.
#[derive(Clone, Copy)]
enum NewLayer {
    Text,
    Shape(ShapeKind),
    FromFile,
}

impl NewLayer {
    const ALL: [Self; 4] = [
        Self::Text,
        Self::Shape(ShapeKind::Rectangle),
        Self::Shape(ShapeKind::Ellipse),
        Self::FromFile,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Shape(kind) => kind.name(),
            Self::FromFile => "From File…",
        }
    }

    fn layer(self) -> Option<GraphicLayer> {
        let content = match self {
            Self::Text => GraphicContent::Text(Title::default()),
            Self::Shape(kind) => GraphicContent::Shape(Shape::new(kind)),
            Self::FromFile => return None,
        };
        Some(GraphicLayer::new(content))
    }
}

/// Whether "From File…" was picked this frame, which asks for the picture's path.
pub(super) fn take_picture_request(ui: &egui::Ui) -> bool {
    ui.data_mut(|data| data.remove_temp::<bool>(picture_request_id()))
        .unwrap_or(false)
}

fn picture_request_id() -> egui::Id {
    egui::Id::unique("graphic_picture_layer")
}

/// `generator` with `path`'s picture as its new top layer, turning a still title or a
/// shape into a graphic first.
pub(crate) fn with_picture(generator: &Generator, path: PathBuf) -> Option<Generator> {
    let mut graphic = match generator {
        Generator::Graphic(graphic) => graphic.clone(),
        other => Graphic::layered(other)?,
    };
    let layer = GraphicLayer::new(GraphicContent::Image(path));
    graphic.layers.insert(0, layer);
    Some(Generator::Graphic(graphic))
}

/// Offers a new layer over a still title or a shape, which turns it into a graphic.
pub(super) fn upgrade_row(ui: &mut egui::Ui, generator: &mut Generator) -> Option<Change> {
    let mut graphic = Graphic::layered(generator)?;
    let added = new_layer_menu(ui)?.layer()?;
    graphic.layers.insert(0, added);
    *generator = Generator::Graphic(graphic);
    Some(Change::Commit)
}

fn new_layer_menu(ui: &mut egui::Ui) -> Option<NewLayer> {
    let mut added = None;
    ui.menu_button("New Layer", |ui| {
        for kind in NewLayer::ALL {
            if ui.button(kind.name()).clicked() {
                added = Some(kind);
                ui.close();
            }
        }
    });
    if matches!(added, Some(NewLayer::FromFile)) {
        ui.data_mut(|data| data.insert_temp(picture_request_id(), true));
    }
    added
}

/// Essential Graphics' Edit tab: the layers, topmost first, then the chosen one's look.
pub(super) fn controls(ui: &mut egui::Ui, graphic: &mut Graphic, height: u32) -> Option<Change> {
    let chosen_id = ui.make_persistent_id("graphic_layer");
    let last = graphic.layers.len().saturating_sub(1);
    let mut chosen = ui
        .data(|data| data.get_temp::<usize>(chosen_id))
        .unwrap_or(0)
        .min(last);
    let mut change = None;
    ui.horizontal_wrapped(|ui| {
        if let Some(added) = new_layer_menu(ui).and_then(NewLayer::layer) {
            graphic.layers.insert(0, added);
            chosen = 0;
            change = Some(Change::Commit);
        }
        let mut restack = |down: bool, ui: &mut egui::Ui, label: &str| {
            let can = if down { chosen < last } else { chosen > 0 };
            if ui.add_enabled(can, egui::Button::new(label)).clicked() {
                if let Some(to) = graphic.restack(chosen, down) {
                    chosen = to;
                    change = Some(Change::Commit);
                }
            }
        };
        restack(false, ui, "Up");
        restack(true, ui, "Down");
        let removable = graphic.layers.len() > 1;
        if ui
            .add_enabled(removable, egui::Button::new("Delete"))
            .clicked()
        {
            graphic.layers.remove(chosen);
            chosen = chosen.min(graphic.layers.len().saturating_sub(1));
            change = Some(Change::Commit);
        }
    });
    egui::Frame::group(ui.style()).show(ui, |ui| {
        for (index, layer) in graphic.layers.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let mut shown = !layer.hidden;
                if ui
                    .checkbox(&mut shown, "")
                    .on_hover_text("Show layer")
                    .changed()
                {
                    layer.hidden = !shown;
                    change = Some(Change::Commit);
                }
                let kind = match layer.content {
                    GraphicContent::Text(_) => "T",
                    GraphicContent::Shape(_) => "◆",
                    GraphicContent::Image(_) => "▣",
                };
                ui.weak(kind);
                if ui.selectable_label(index == chosen, &layer.name).clicked() {
                    chosen = index;
                }
            });
        }
    });
    ui.data_mut(|data| data.insert_temp(chosen_id, chosen));
    let Some(layer) = graphic.layers.get_mut(chosen) else {
        return change;
    };
    change = layer_controls(ui, layer, height).or(change);
    change
}

fn layer_controls(ui: &mut egui::Ui, layer: &mut GraphicLayer, height: u32) -> Option<Change> {
    let mut change = None;
    ui.horizontal(|ui| {
        ui.label("Name");
        let name = ui.add(egui::TextEdit::singleline(&mut layer.name).desired_width(f32::INFINITY));
        if name.changed() {
            change = Some(Change::Live);
        }
        if name.lost_focus() {
            change = Some(Change::Commit);
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Position");
        for (axis, value) in ["X", "Y"].into_iter().zip(&mut layer.offset) {
            let drag = egui::DragValue::new(value)
                .range(-20_000..=20_000)
                .prefix(format!("{axis} "))
                .suffix(" px");
            change = response_state(&ui.add(drag), change);
        }
        if ui
            .button("Center")
            .on_hover_text("Centers the layer in the frame")
            .clicked()
        {
            layer.offset = [0, 0];
            change = Some(Change::Commit);
        }
    });
    pairs(ui, 300.0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Scale");
            let scale = egui::DragValue::new(&mut layer.scale)
                .range(1..=2000)
                .suffix(" %");
            change = response_state(&ui.add(scale), change);
        });
        ui.horizontal(|ui| {
            ui.label("Rotation");
            let rotation = egui::DragValue::new(&mut layer.rotation)
                .range(-3600..=3600)
                .suffix("°");
            change = response_state(&ui.add(rotation), change);
        });
        ui.horizontal(|ui| {
            ui.label("Opacity");
            let opacity = egui::DragValue::new(&mut layer.opacity)
                .range(0..=100)
                .suffix(" %");
            change = response_state(&ui.add(opacity), change);
        });
    });
    match &mut layer.content {
        GraphicContent::Text(title) => {
            // Named after its text until renamed, as Premiere names text layers.
            let named_by_text = layer.name == first_line(&title.text);
            change = text_controls(ui, title).or(change);
            title.motion = TitleMotion::Still;
            if named_by_text {
                layer.name = first_line(&title.text).to_owned();
            }
        }
        GraphicContent::Shape(shape) => change = shape_controls(ui, shape, height).or(change),
        GraphicContent::Image(path) => {
            ui.weak(path.display().to_string());
        }
    }
    change
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}
