//! The checkerboard a monitor shows through a picture's transparent parts.

use egui::{Color32, ColorImage, Rect, TextureHandle, TextureOptions, TextureWrapMode};

/// The side of one square, in points.
const SQUARE: f32 = 8.0;
const LIGHT: Color32 = Color32::from_gray(204);
const DARK: Color32 = Color32::from_gray(153);

pub(crate) fn paint(painter: &egui::Painter, rect: Rect) {
    let texture = texture(painter.ctx());
    let repeats = rect.size() / (2.0 * SQUARE);
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), repeats.to_pos2());
    painter.image(texture.id(), rect, uv, Color32::WHITE);
}

/// Two squares by two, drawn once and tiled.
fn texture(ctx: &egui::Context) -> TextureHandle {
    let id = egui::Id::unique("transparency grid");
    if let Some(texture) = ctx.data(|data| data.get_temp::<TextureHandle>(id)) {
        return texture;
    }
    let image = ColorImage::new([2, 2], vec![LIGHT, DARK, DARK, LIGHT]);
    let options = TextureOptions {
        wrap_mode: TextureWrapMode::Repeat,
        ..TextureOptions::NEAREST
    };
    let texture = ctx.load_texture("transparency_grid", image, options);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}
