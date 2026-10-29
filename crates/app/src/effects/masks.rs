use egui::RichText;
use media::TrackDirection;
use render::MAX_MASKS;
use timeline::{
    key_path, path_is_animated, toggle_path_animation, Mask, MaskShape, MaskTracking, Ticks,
};

use super::look::{icon_button, Icon, Rows, ANIMATED};
use crate::mask_tracking::TrackSpan;

/// The masks under Opacity: each shape's geometry and edge, and buttons to add more.
pub(super) fn controls(ui: &mut egui::Ui, rows: &mut Rows, masks: &mut Vec<Mask>) {
    let mut removed = None;
    for (index, mask) in masks.iter_mut().enumerate() {
        ui.push_id(("mask", index), |ui| {
            ui.horizontal(|ui| {
                let title = format!("Mask ({}) – {}", index + 1, mask.shape.name());
                ui.label(RichText::new(title).strong());
                if ui.small_button("Remove").clicked() {
                    removed = Some(index);
                }
            });
            if mask.shape == MaskShape::Path {
                path_rows(ui, rows, mask);
            } else {
                let [x, y] = &mut mask.center;
                rows.param(ui, "Center X", x, -100.0..=200.0, "%");
                rows.param(ui, "Center Y", y, -100.0..=200.0, "%");
                let [width, height] = &mut mask.size;
                rows.param(ui, "Width", width, 0.0..=400.0, "%");
                rows.param(ui, "Height", height, 0.0..=400.0, "%");
                rows.param(ui, "Rotation", &mut mask.rotation, -3_600.0..=3_600.0, "°");
            }
            tracking_rows(ui, rows, index, mask);
            rows.param(ui, "Feather", &mut mask.feather, 0.0..=1_000.0, " px");
            rows.param(ui, "Opacity", &mut mask.opacity, 0.0..=100.0, "%");
            rows.param(
                ui,
                "Expansion",
                &mut mask.expansion,
                -1_000.0..=1_000.0,
                " px",
            );
            if ui.checkbox(&mut mask.inverted, "Inverted").changed() {
                rows.commit();
            }
        });
    }
    if let Some(index) = removed {
        masks.remove(index);
        rows.commit();
    }
    ui.horizontal_wrapped(|ui| {
        for shape in MaskShape::ALL {
            let add = egui::Button::new(format!("Add {} Mask", shape.name())).small();
            if ui.add_enabled(masks.len() < MAX_MASKS, add).clicked() {
                masks.push(Mask::new(shape));
                rows.commit();
            }
        }
    });
}

/// Buttons that key the mask along the picture under it from the playhead, one frame
/// or to the clip's end either way, and what of the picture's motion it follows.
fn tracking_rows(ui: &mut egui::Ui, rows: &mut Rows, index: usize, mask: &mut Mask) {
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        ui.add_sized([72.0, 18.0], egui::Label::new("Track"));
        if let Some((_, done)) = rows.tracking.filter(|(tracked, _)| *tracked == index) {
            let text = format!("Tracking… {:.0}%", done * 100.0);
            ui.add(egui::ProgressBar::new(done).desired_width(120.0).text(text));
            rows.output.stop_tracking = ui.small_button("Stop").clicked();
            return;
        }
        let trackable = rows.tracking.is_none() && (mask.shape != MaskShape::Path || mask.closed);
        ui.spacing_mut().item_spacing.x = 4.0;
        let buttons = [
            (
                TrackDirection::Backward,
                TrackSpan::OneFrame,
                "backward one frame",
            ),
            (TrackDirection::Backward, TrackSpan::ToEnd, "backward"),
            (TrackDirection::Forward, TrackSpan::ToEnd, "forward"),
            (
                TrackDirection::Forward,
                TrackSpan::OneFrame,
                "forward one frame",
            ),
        ];
        for (direction, span, way) in buttons {
            let button = icon_button(ui, Icon::Track(direction, span), false, trackable)
                .on_hover_text(format!("Track the mask {way}"));
            if button.clicked() {
                rows.output.track_mask = Some((index, direction, span));
            }
        }
    });
    rows.options(
        ui,
        "Tracking",
        "mask_tracking",
        &mut mask.tracking,
        MaskTracking::ALL,
        MaskTracking::name,
    );
}

/// A path's keyframes, which hold every vertex at once, and how to draw and edit it.
fn path_rows(ui: &mut egui::Ui, rows: &mut Rows, mask: &mut Mask) {
    let time = rows.time();
    ui.horizontal(|ui| {
        let animated = path_is_animated(&mask.path);
        let stopwatch = icon_button(ui, Icon::Stopwatch, animated, !mask.path.is_empty())
            .on_hover_text("Toggle path animation");
        if stopwatch.clicked() {
            toggle_path_animation(&mut mask.path, time);
            rows.commit();
        }
        let text = RichText::new("Mask Path");
        let text = if animated { text.color(ANIMATED) } else { text };
        ui.add_sized([72.0, 18.0], egui::Label::new(text));
        if !animated {
            return;
        }
        let mut keys: Vec<Ticks> = mask
            .params()
            .flat_map(|param| param.keys.iter().map(|key| key.time))
            .collect();
        keys.sort_unstable();
        keys.dedup();
        ui.spacing_mut().item_spacing.x = 2.0;
        let previous = keys.iter().rev().copied().find(|&key| key < time);
        let back = icon_button(ui, Icon::Previous, false, previous.is_some())
            .on_hover_text("Previous keyframe");
        if back.clicked() {
            rows.output.seek = previous;
        }
        let keyed = keys.contains(&time);
        let key = icon_button(ui, Icon::Key(Default::default()), keyed, true)
            .on_hover_text("Add or remove the path's keyframe");
        if key.clicked() {
            if keyed {
                let params = mask.path.iter_mut().flat_map(|vertex| vertex.params_mut());
                for param in params.filter(|param| param.has_key_at(time)) {
                    param.toggle_key(time);
                }
            } else {
                key_path(&mut mask.path, time);
            }
            rows.commit();
        }
        let next = keys.iter().copied().find(|&key| key > time);
        let forward =
            icon_button(ui, Icon::Next, false, next.is_some()).on_hover_text("Next keyframe");
        if forward.clicked() {
            rows.output.seek = next;
        }
    });
    let hint = if mask.closed {
        "Drag vertices and handles on the picture. Ctrl-click a side to add a vertex or a \
         vertex to delete it; Alt-drag a vertex to curve it, Alt-click to make a corner."
    } else {
        "Click the picture to add vertices, dragging to curve them; click the first vertex \
         to close the path."
    };
    ui.label(RichText::new(hint).weak().small());
    if !mask.closed {
        let close = egui::Button::new("Close Path").small();
        if ui.add_enabled(mask.path.len() >= 3, close).clicked() {
            mask.closed = true;
            rows.commit();
        }
    }
}
