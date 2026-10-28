use std::cmp::Ordering;

use egui::{Align2, Color32, FontId, Rect, Sense, Stroke};
use timeline::{MediaItem, Ticks, Timebase};

use super::rows::played_length;
use crate::clip_art::ClipArt;

const TILE_WIDTH: f32 = 128.0;
const PICTURE_HEIGHT: f32 = TILE_WIDTH * 9.0 / 16.0;
const CAPTION_HEIGHT: f32 = 32.0;
/// Hover-scrubbing shows this many evenly spaced pictures, so that their thumbnails
/// are made once and reused.
const SCRUB_STEPS: i64 = 32;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PoolLayout {
    #[default]
    List,
    Icons,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PoolSort {
    /// The order items were added in.
    #[default]
    Project,
    Name,
    Duration,
    Kind,
}

impl PoolSort {
    pub(super) const ALL: [Self; 4] = [Self::Project, Self::Name, Self::Duration, Self::Kind];

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Project => "Project Order",
            Self::Name => "Name",
            Self::Duration => "Duration",
            Self::Kind => "Type",
        }
    }

    pub(super) fn sorted<'a>(
        self,
        items: impl Iterator<Item = &'a MediaItem>,
    ) -> Vec<&'a MediaItem> {
        let mut items: Vec<_> = items.collect();
        let order: fn(&&MediaItem, &&MediaItem) -> Ordering = match self {
            Self::Project => return items,
            Self::Name => |_, _| Ordering::Equal,
            Self::Duration => |a, b| b.duration.cmp(&a.duration),
            Self::Kind => |a, b| kind_rank(a).cmp(&kind_rank(b)),
        };
        items.sort_by(|a, b| order(a, b).then_with(|| by_name(a, b)));
        items
    }
}

fn by_name(a: &&MediaItem, b: &&MediaItem) -> Ordering {
    a.name.to_lowercase().cmp(&b.name.to_lowercase())
}

fn kind_rank(item: &MediaItem) -> u8 {
    match (&item.nest, &item.generator, &item.subclip, &item.video) {
        (Some(_), ..) => 0,
        (None, None, None, Some(_)) => 1,
        (None, None, Some(_), _) => 2,
        (None, None, None, None) => 3,
        (None, Some(_), ..) => 4,
    }
}

fn kind_name(item: &MediaItem) -> &'static str {
    match (&item.nest, &item.generator, &item.video) {
        (Some(_), ..) => "Sequence",
        (None, Some(generator), _) => generator.kind_name(),
        (None, None, Some(_)) => "Video",
        (None, None, None) => "Audio",
    }
}

/// An item as a picture with its name and length below. Moving the pointer across the
/// picture scrubs through the item.
pub(super) fn tile(
    ui: &mut egui::Ui,
    item: &MediaItem,
    timebase: Timebase,
    art: &mut ClipArt,
    offline: bool,
) -> egui::Response {
    let size = egui::vec2(TILE_WIDTH, PICTURE_HEIGHT + CAPTION_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter_at(rect);
    let picture = Rect::from_min_size(rect.min, egui::vec2(TILE_WIDTH, PICTURE_HEIGHT));
    painter.rect_filled(picture, 2.0, Color32::from_gray(16));
    let range = item.default_range(timebase);
    let scrub = response
        .hover_pos()
        .map(|pos| ((pos.x - picture.left()) / picture.width()).clamp(0.0, 1.0));
    let step = scrub.map_or(0, |at| {
        ((at * SCRUB_STEPS as f32) as i64).min(SCRUB_STEPS - 1)
    });
    let at = Ticks(range.start.0 + (range.end.0 - range.start.0) * step / SCRUB_STEPS);
    let texture = match item.video {
        Some(video) if item.is_file() && !offline => {
            let path = item.proxy.as_deref().unwrap_or(&item.path);
            art.picture(path, at, timebase)
                .map(|texture| (texture, video))
        }
        _ => None,
    };
    let weak = ui.visuals().weak_text_color();
    match texture {
        Some((texture, video)) => {
            let aspect = video.width as f32 / video.height.max(1) as f32;
            let fitted = (picture.height() * aspect).min(picture.width());
            let shown =
                Rect::from_center_size(picture.center(), egui::vec2(fitted, fitted / aspect));
            let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.image(texture, shown, uv, Color32::WHITE);
        }
        None => {
            let (label, color) = if offline {
                ("Media Offline", ui.visuals().error_fg_color)
            } else {
                (kind_name(item), weak)
            };
            let font = FontId::proportional(12.0);
            painter.text(picture.center(), Align2::CENTER_CENTER, label, font, color);
        }
    }
    if let Some(at) = scrub.filter(|_| item.video.is_some() && item.is_file()) {
        let x = picture.left() + at * picture.width();
        let stroke = Stroke::new(1.0, Color32::from_rgb(250, 210, 60));
        painter.vline(x, picture.y_range(), stroke);
    }
    let text = ui.visuals().text_color();
    let name_at = egui::pos2(rect.left() + 2.0, picture.bottom() + 2.0);
    let name_color = if offline {
        ui.visuals().error_fg_color
    } else {
        text
    };
    painter.text(
        name_at,
        Align2::LEFT_TOP,
        &item.name,
        FontId::proportional(13.0),
        name_color,
    );
    painter.text(
        egui::pos2(rect.left() + 2.0, picture.bottom() + 17.0),
        Align2::LEFT_TOP,
        played_length(item, timebase).to_string(),
        FontId::proportional(11.0),
        weak,
    );
    response.on_hover_text(&item.name)
}
