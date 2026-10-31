use egui::{Align2, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use timeline::{
    Generator, Graphic, GraphicContent, GraphicLayer, SetClipGenerator, SetClipLook, Shape,
    ShapeKind, Ticks, Title,
};

use crate::editor::Editor;
use crate::effects::keyframe_time;
use crate::placing;
use crate::timeline_view::Tool;

const OUTLINE: Color32 = Color32::from_rgb(220, 60, 60);
/// Drags shorter than this, in sequence pixels, draw nothing.
const SMALLEST_SHAPE: f32 = 4.0;

/// Text being typed on the program picture, placed as a title when typing ends.
pub(crate) struct Typing {
    /// Where the text's middle goes, from the middle of the frame in sequence pixels.
    at: Vec2,
    text: String,
    opened: bool,
}

/// The program picture fitted into a monitor's area.
struct Fitted {
    area: Rect,
    /// Screen points to a sequence pixel.
    scale: f32,
}

impl Fitted {
    fn new(editor: &Editor, area: Rect) -> Self {
        let format = editor.timeline.sequence().format;
        let canvas = Vec2::new(format.width as f32, format.height as f32);
        let scale = (area.width() / canvas.x).min(area.height() / canvas.y);
        Self {
            area: Rect::from_center_size(area.center(), canvas * scale),
            scale,
        }
    }

    fn to_sequence(&self, pos: Pos2) -> Vec2 {
        (pos - self.area.center()) / self.scale.max(f32::EPSILON)
    }

    fn to_screen(&self, offset: Vec2) -> Pos2 {
        self.area.center() + offset * self.scale
    }
}

/// Lets the Type tool start a title where the program picture is clicked, and the
/// shape tools draw a shape where it is dragged. Returns whether a graphics tool is
/// in use, which leaves the picture to it.
pub(crate) fn overlay(editor: &mut Editor, ui: &mut egui::Ui, area: Rect) -> bool {
    let tool = editor.view.tool;
    if !matches!(tool, Tool::Type | Tool::Shape(_)) && editor.typing.is_none() {
        return false;
    }
    let fitted = Fitted::new(editor, area);
    if editor.typing.is_some() {
        type_text(editor, ui, &fitted);
        return true;
    }
    let id = ui.make_persistent_id("graphics_tool");
    let response = ui.interact(fitted.area, id, Sense::click_and_drag());
    match tool {
        Tool::Type => {
            if response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::Text);
            }
            if let Some(pos) = response
                .interact_pointer_pos()
                .filter(|_| response.clicked())
            {
                editor.typing = Some(Typing {
                    at: fitted.to_sequence(pos),
                    text: String::new(),
                    opened: true,
                });
            }
        }
        Tool::Shape(kind) => draw_shape(editor, ui, &fitted, &response, kind),
        _ => {}
    }
    true
}

fn type_text(editor: &mut Editor, ui: &mut egui::Ui, fitted: &Fitted) {
    let Some(typing) = &mut editor.typing else {
        return;
    };
    let title = Title::default();
    let font = FontId::proportional((title.size as f32 * fitted.scale * 0.8).max(8.0));
    let mut done = false;
    egui::Area::new(ui.make_persistent_id("typing"))
        .fixed_pos(fitted.to_screen(typing.at))
        .pivot(Align2::CENTER_CENTER)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            let field = egui::TextEdit::multiline(&mut typing.text)
                .font(font)
                .text_color(Color32::WHITE)
                .align(Align2::CENTER_TOP)
                .desired_rows(1)
                .desired_width(fitted.area.width() * 0.8)
                .frame(egui::Frame::NONE.stroke(Stroke::new(1.0, OUTLINE)))
                .background_color(Color32::from_black_alpha(96))
                .hint_text("Type, then Esc or click away");
            let response = ui.add(field);
            if std::mem::take(&mut typing.opened) {
                response.request_focus();
            }
            done = response.lost_focus();
        });
    if !done {
        return;
    }
    let Some(typing) = editor.typing.take() else {
        return;
    };
    editor.view.tool = Tool::Selection;
    if typing.text.trim().is_empty() {
        return;
    }
    let title = Title {
        text: typing.text,
        ..title
    };
    place(editor, Generator::Title(title), typing.at);
}

fn draw_shape(
    editor: &mut Editor,
    ui: &egui::Ui,
    fitted: &Fitted,
    response: &egui::Response,
    kind: ShapeKind,
) {
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
    }
    // Kept apart from the pointer's own, which is gone by the frame the button is let go.
    if response.drag_started() {
        if let Some(origin) = ui.input(|input| input.pointer.press_origin()) {
            ui.data_mut(|data| data.insert_temp(response.id, origin));
        }
    }
    let origin = ui.data(|data| data.get_temp::<Pos2>(response.id));
    let (pointer, shift) = ui.input(|input| (input.pointer.latest_pos(), input.modifiers.shift));
    let (Some(origin), Some(pointer)) = (origin, pointer) else {
        return;
    };
    if !(response.dragged() || response.drag_stopped()) {
        return;
    }
    if response.drag_stopped() {
        ui.data_mut(|data| data.remove::<Pos2>(response.id));
    }
    let from = fitted.to_sequence(origin);
    let mut size = fitted.to_sequence(pointer) - from;
    if shift {
        let side = size.x.abs().max(size.y.abs());
        size = Vec2::new(side.copysign(size.x), side.copysign(size.y));
    }
    let drawn = Rect::from_two_pos(from.to_pos2(), (from + size).to_pos2());
    if response.dragged() {
        let screen = Rect::from_two_pos(
            fitted.to_screen(drawn.min.to_vec2()),
            fitted.to_screen(drawn.max.to_vec2()),
        );
        let painter = ui.painter_at(fitted.area);
        let stroke = Stroke::new(1.5, OUTLINE);
        match kind {
            ShapeKind::Rectangle => {
                painter.rect_stroke(screen, 0.0, stroke, egui::StrokeKind::Middle);
            }
            ShapeKind::Ellipse => {
                painter.add(egui::Shape::ellipse_stroke(
                    screen.center(),
                    screen.size() / 2.0,
                    stroke,
                ));
            }
        }
        return;
    }
    if drawn.width() < SMALLEST_SHAPE || drawn.height() < SMALLEST_SHAPE {
        return;
    }
    let shape = Shape {
        width: drawn.width().round() as u32,
        height: drawn.height().round() as u32,
        ..Shape::new(kind)
    };
    place(editor, Generator::Shape(shape), drawn.center().to_vec2());
}

/// Places a new graphic at the playhead with its middle `at` from the frame's middle.
fn place(editor: &mut Editor, generator: Generator, at: Vec2) {
    if add_layer_to_selected(editor, &generator, at) {
        return;
    }
    let Some(clip) = placing::new_generated(editor, generator) else {
        return;
    };
    let Some((_, placed)) = editor.timeline.sequence().clip(clip) else {
        return;
    };
    let mut look = placed.look.clone();
    let [x, y] = &mut look.motion.position;
    x.set_at(Ticks::ZERO, at.x.round());
    y.set_at(Ticks::ZERO, at.y.round());
    editor.apply(SetClipLook { clip, look });
}

/// With one graphic, still title or shape selected, draws `generator` as its new top
/// layer instead of as a clip of its own, as Premiere's tools do.
fn add_layer_to_selected(editor: &mut Editor, generator: &Generator, at: Vec2) -> bool {
    let &[clip] = editor.view.selection.as_slice() else {
        return false;
    };
    let sequence = editor.timeline.sequence();
    let Some((_, placed)) = sequence.clip(clip) else {
        return false;
    };
    let Some(drawn) = sequence
        .media_item(placed.source)
        .and_then(|item| item.generator.as_ref())
    else {
        return false;
    };
    let graphic = match drawn {
        Generator::Graphic(graphic) => Some(graphic.clone()),
        other => Graphic::layered(other),
    };
    let content = match generator {
        Generator::Title(title) => GraphicContent::Text(title.clone()),
        Generator::Shape(shape) => GraphicContent::Shape(*shape),
        _ => return false,
    };
    let Some(mut graphic) = graphic else {
        return false;
    };
    // Into the graphic's own pixels, undoing its position and scale; turns are left as
    // they are, since layers don't turn.
    let time = keyframe_time(editor, placed);
    let motion = &placed.look.motion;
    let [x, y] = motion.position.each_ref().map(|axis| axis.value_at(time));
    let [width, height] = motion
        .scale_at(time)
        .map(|percent| (percent / 100.0).max(0.01));
    let offset = [(at.x - x) / width, (at.y - y) / height].map(|value| value.round() as i32);
    graphic.layers.insert(
        0,
        GraphicLayer {
            offset,
            ..GraphicLayer::new(content)
        },
    );
    editor.apply(SetClipGenerator {
        clip: placed.id,
        generator: Generator::Graphic(graphic),
    });
    true
}
