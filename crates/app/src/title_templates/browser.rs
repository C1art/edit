//! The Graphics panel, as Essential Graphics' Browse tab: every template as a picture,
//! by kind, added at the playhead with a double-click.

use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, TextEdit};
use timeline::Generator;

use super::catalog::{Category, BUILTINS};
use super::previews::{Previews, PREVIEW_SIZE};
use crate::editor::Editor;
use crate::theme;

/// Cards grow from this to fill the panel's width.
const NARROWEST_CARD: f32 = 140.0;
const CAPTION_HEIGHT: f32 = 22.0;
/// A mid-gray scene, so light and dark templates both read against it.
const BACKDROP: Color32 = Color32::from_rgb(58, 62, 72);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Shelf {
    #[default]
    All,
    Kind(Category),
    Mine,
}

impl Shelf {
    fn name(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Kind(category) => category.name(),
            Self::Mine => "My Templates",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pick {
    Builtin(usize),
    Saved(usize),
}

#[derive(Default)]
pub(crate) struct GraphicsBrowser {
    search: String,
    shelf: Shelf,
    selected: Option<Pick>,
    previews: Option<Previews>,
    /// The preview worker couldn't start; cards show names only.
    unstarted: bool,
}

impl GraphicsBrowser {
    /// Takes in drawn previews; true when the panel should be drawn again.
    pub(crate) fn poll(&mut self, ctx: &egui::Context) -> bool {
        self.previews
            .as_mut()
            .is_some_and(|previews| previews.poll(ctx))
    }

    fn previews(&mut self) -> Option<&mut Previews> {
        if self.previews.is_none() && !self.unstarted {
            self.previews = Previews::start().ok();
            self.unstarted = self.previews.is_none();
        }
        self.previews.as_mut()
    }
}

/// What a card was asked to do.
enum Action {
    Add(Pick),
    Delete(usize),
}

pub(crate) fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    let format = editor.timeline.sequence().format;
    let browser = &mut editor.graphics;
    ui.add(
        TextEdit::singleline(&mut browser.search)
            .hint_text("Search templates")
            .desired_width(f32::INFINITY),
    );
    ui.horizontal_wrapped(|ui| {
        let shelves = [Shelf::All]
            .into_iter()
            .chain(Category::ALL.map(Shelf::Kind))
            .chain([Shelf::Mine]);
        for shelf in shelves {
            ui.selectable_value(&mut browser.shelf, shelf, shelf.name());
        }
    });
    ui.weak("Double-click to add at the playhead; edit the text in Effect Controls");
    ui.separator();

    let search = browser.search.trim().to_lowercase();
    let matches = |name: &str, category: &str| {
        search.is_empty()
            || name.to_lowercase().contains(&search)
            || category.to_lowercase().contains(&search)
    };
    let canvas = preview_canvas(format.width, format.height);
    let mut cards: Vec<(Pick, &str, Generator, Option<[i32; 2]>)> = Vec::new();
    if matches!(browser.shelf, Shelf::All | Shelf::Kind(_)) {
        for (index, builtin) in BUILTINS.iter().enumerate() {
            let shown =
                browser.shelf == Shelf::All || browser.shelf == Shelf::Kind(builtin.category);
            if shown && matches(builtin.name, builtin.category.name()) {
                let built = builtin.build(canvas[0], canvas[1]);
                cards.push((
                    Pick::Builtin(index),
                    builtin.name,
                    built.generator,
                    built.position,
                ));
            }
        }
    }
    if matches!(browser.shelf, Shelf::All | Shelf::Mine) {
        for (index, template) in editor.title_templates.iter().enumerate() {
            if matches(&template.name, "My Templates") {
                let generator = template.generator.clone();
                cards.push((Pick::Saved(index), &template.name, generator, None));
            }
        }
    }

    let mut action = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if cards.is_empty() {
                ui.weak(if browser.shelf == Shelf::Mine && search.is_empty() {
                    "Keep a title of yours here with Save as Title Template… on its clip"
                } else {
                    "No templates match"
                });
            }
            let gap = ui.spacing().item_spacing.x;
            let columns = ((ui.available_width() + gap) / (NARROWEST_CARD + gap)).floor();
            let card_width = ((ui.available_width() + gap) / columns.max(1.0) - gap).floor();
            ui.horizontal_wrapped(|ui| {
                for (pick, name, generator, position) in &cards {
                    // Saved templates are sized for the sequence, so they're drawn at
                    // its size and shrunk.
                    let drawn_at = match pick {
                        Pick::Builtin(_) => canvas,
                        Pick::Saved(_) => [format.width, format.height],
                    };
                    let selected = browser.selected == Some(*pick);
                    let picture = browser
                        .previews()
                        .and_then(|previews| previews.picture(generator, drawn_at))
                        .map(|texture| (texture.id(), drawn_at, *position));
                    let response = card(ui, name, picture, canvas, card_width, selected);
                    if response.clicked() {
                        browser.selected = Some(*pick);
                    }
                    if response.double_clicked() {
                        action = Some(Action::Add(*pick));
                    }
                    response.context_menu(|ui| {
                        if ui.button("Add to Timeline").clicked() {
                            action = Some(Action::Add(*pick));
                        }
                        if let Pick::Saved(index) = pick {
                            if ui.button("Delete").clicked() {
                                action = Some(Action::Delete(*index));
                            }
                        }
                    });
                }
            });
        });
    match action {
        Some(Action::Add(Pick::Builtin(index))) => {
            if let Some(builtin) = BUILTINS.get(index) {
                editor.new_from_builtin(builtin);
            }
        }
        Some(Action::Add(Pick::Saved(index))) => editor.new_from_saved(index),
        Some(Action::Delete(index)) => {
            editor.graphics.selected = None;
            editor.delete_title_template(index);
        }
        None => {}
    }
}

/// The frame templates are previewed in: the sequence's shape, its longer side the
/// preview size.
fn preview_canvas(width: u32, height: u32) -> [u32; 2] {
    let (width, height) = (width.max(1) as f32, height.max(1) as f32);
    let longer = width.max(height);
    [width, height].map(|side| ((side / longer * PREVIEW_SIZE as f32).round() as u32).max(1))
}

/// A template's card: its picture on a scene, offset as the clip will be, over its name.
/// `picture` is the drawn texture with the frame it was drawn in and the offset in it.
fn card(
    ui: &mut egui::Ui,
    name: &str,
    picture: Option<(egui::TextureId, [u32; 2], Option<[i32; 2]>)>,
    canvas: [u32; 2],
    width: f32,
    selected: bool,
) -> egui::Response {
    let aspect = canvas[1] as f32 / canvas[0].max(1) as f32;
    let picture_height = (width * aspect).min(width * 1.2);
    let size = egui::vec2(width, picture_height + CAPTION_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter_at(rect);
    let scene_width = picture_height / aspect;
    let scene = Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + picture_height / 2.0),
        egui::vec2(scene_width.min(width), picture_height),
    );
    painter.rect_filled(scene, 3.0, BACKDROP);
    match picture {
        Some((texture, drawn_at, offset)) => {
            let pixels = scene.width() / drawn_at[0].max(1) as f32;
            let shift = offset.map_or(egui::Vec2::ZERO, |[x, y]| {
                egui::vec2(x as f32, y as f32) * pixels
            });
            let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().with_clip_rect(scene).image(
                texture,
                scene.translate(shift),
                uv,
                Color32::WHITE,
            );
        }
        None => {
            painter.text(
                scene.center(),
                Align2::CENTER_CENTER,
                "…",
                FontId::proportional(14.0),
                ui.visuals().weak_text_color(),
            );
        }
    }
    let outline = if selected {
        Stroke::new(2.0, theme::ACCENT)
    } else if response.hovered() {
        Stroke::new(1.0, ui.visuals().widgets.hovered.bg_stroke.color)
    } else {
        Stroke::NONE
    };
    painter.rect_stroke(scene, 3.0, outline, StrokeKind::Inside);
    let caption = egui::pos2(rect.left() + 2.0, scene.bottom() + 4.0);
    let text = RichText::new(name).size(12.0);
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        width - 4.0,
        egui::TextStyle::Body,
    );
    painter.galley(caption, galley, ui.visuals().text_color());
    response.on_hover_text(name)
}
