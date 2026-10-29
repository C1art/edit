use egui::{Color32, Modifiers, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use timeline::{
    flatten_path, key_path, path_side_point, split_path_side, Mask, PathVertex, Ticks, VertexAt,
};

use super::{Grip, GRAB_RADIUS, HANDLE_RADIUS, OUTLINE};
use crate::effects::picture_frame::PictureFrame;

/// Points drawn along each side of a path, and tried when looking for one under the
/// pointer.
const SIDE_STEPS: usize = 24;
/// Points the outline a closed path is hit-tested against has, as the shader draws it.
const OUTLINE_POINTS: usize = render::MAX_OUTLINE_POINTS;

/// A path mask at one time, in picture pixels from the picture's center.
pub(super) struct PathGeometry {
    vertices: Vec<VertexAt>,
    closed: bool,
    fitted: Vec2,
}

impl PathGeometry {
    pub(super) fn new(mask: &Mask, time: Ticks, fitted: Vec2) -> Self {
        Self {
            vertices: mask.path.iter().map(|vertex| vertex.at(time)).collect(),
            closed: mask.closed,
            fitted,
        }
    }

    fn position(&self, index: usize) -> Vec2 {
        to_picture(self.vertices[index].position, self.fitted)
    }

    fn handle(&self, index: usize) -> Vec2 {
        Vec2::from(self.vertices[index].handle) * self.fitted
    }

    /// Each side as the vertices it joins; an open path has no side back to the first.
    fn sides(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        let count = self.vertices.len();
        let sides = if self.closed {
            count
        } else {
            count.saturating_sub(1)
        };
        (0..sides).map(move |side| (side, (side + 1) % count))
    }

    fn side_point(&self, (from, to): (usize, usize), along: f32) -> Vec2 {
        let point = path_side_point(self.vertices[from], self.vertices[to], along);
        to_picture(point, self.fitted)
    }

    pub(super) fn contains(&self, picture: Vec2) -> bool {
        if !self.closed || self.vertices.len() < 3 {
            return false;
        }
        let outline: Vec<Vec2> = flatten_path(&self.vertices, OUTLINE_POINTS)
            .into_iter()
            .map(|point| to_picture(point, self.fitted))
            .collect();
        let mut inside = false;
        for (index, &to) in outline.iter().enumerate() {
            let from = outline[(index + outline.len() - 1) % outline.len()];
            if (from.y > picture.y) != (to.y > picture.y)
                && picture.x < from.x + (picture.y - from.y) / (to.y - from.y) * (to.x - from.x)
            {
                inside = !inside;
            }
        }
        inside
    }

    /// The first vertex, the anchor the whole path moves by.
    pub(super) fn anchor(&self) -> Option<Vec2> {
        (!self.vertices.is_empty()).then(|| self.position(0))
    }

    fn vertex_near(&self, frame: &PictureFrame, pointer: Pos2) -> Option<usize> {
        (0..self.vertices.len())
            .rev()
            .find(|&index| frame.to_screen(self.position(index)).distance(pointer) <= GRAB_RADIUS)
    }

    /// A smooth vertex's handle ends, out along the handle (1) and in against it (-1).
    fn handle_ends(&self) -> impl Iterator<Item = (usize, f32, Vec2)> + '_ {
        (0..self.vertices.len())
            .filter(|&index| self.vertices[index].handle != [0.0; 2])
            .flat_map(move |index| {
                [1.0, -1.0].map(|sign| {
                    (
                        index,
                        sign,
                        self.position(index) + self.handle(index) * sign,
                    )
                })
            })
    }

    /// A handle end or vertex at `pointer`, ends first so a short handle can still be
    /// pulled; with Alt a vertex gives its handle, to pull one out of a corner.
    pub(super) fn grab(&self, frame: &PictureFrame, pointer: Pos2, alt: bool) -> Option<Grip> {
        let end = self
            .handle_ends()
            .find(|&(_, _, end)| frame.to_screen(end).distance(pointer) <= GRAB_RADIUS)
            .map(|(index, sign, _)| Grip::Handle(index, sign));
        end.or_else(|| {
            let vertex = self.vertex_near(frame, pointer)?;
            Some(if alt {
                Grip::Handle(vertex, 1.0)
            } else {
                Grip::Vertex(vertex)
            })
        })
    }

    fn side_near(&self, frame: &PictureFrame, pointer: Pos2) -> Option<(usize, f32)> {
        self.sides()
            .flat_map(|side| {
                (1..SIDE_STEPS).map(move |step| (side, step as f32 / SIDE_STEPS as f32))
            })
            .map(|(side, along)| {
                let distance = frame
                    .to_screen(self.side_point(side, along))
                    .distance(pointer);
                (side.0, along, distance)
            })
            .filter(|&(_, _, distance)| distance <= GRAB_RADIUS)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(side, along, _)| (side, along))
    }

    pub(super) fn paint(&self, painter: &Painter, frame: &PictureFrame, hovered: Option<Pos2>) {
        let stroke = Stroke::new(1.5, OUTLINE);
        let line: Vec<Pos2> = self
            .sides()
            .flat_map(|side| {
                (0..SIDE_STEPS).map(move |step| (side, step as f32 / SIDE_STEPS as f32))
            })
            .map(|(side, along)| frame.to_screen(self.side_point(side, along)))
            .chain(
                (!self.closed)
                    .then(|| self.vertices.len().checked_sub(1))
                    .flatten()
                    .map(|last| frame.to_screen(self.position(last))),
            )
            .collect();
        if self.closed {
            painter.add(Shape::closed_line(line, stroke));
        } else {
            painter.add(Shape::line(line, stroke));
        }
        let thin = Stroke::new(1.0, OUTLINE);
        let black = Stroke::new(1.0, Color32::BLACK);
        for (index, _, end) in self.handle_ends() {
            let (vertex, end) = (frame.to_screen(self.position(index)), frame.to_screen(end));
            painter.line_segment([vertex, end], thin);
            painter.circle(end, HANDLE_RADIUS, OUTLINE, black);
        }
        let closes = hovered.is_some_and(|pointer| self.closes_at(frame, pointer));
        for index in 0..self.vertices.len() {
            let center = frame.to_screen(self.position(index));
            let side = if index == 0 && closes { 4.0 } else { 2.5 };
            let square = Rect::from_center_size(center, Vec2::splat(side * 2.0));
            let fill = if index == 0 && !self.closed {
                Color32::WHITE
            } else {
                OUTLINE
            };
            painter.rect(square, 0.0, fill, black, egui::StrokeKind::Middle);
        }
    }

    /// Whether a pen click at `pointer` lands on the first vertex, closing the path.
    fn closes_at(&self, frame: &PictureFrame, pointer: Pos2) -> bool {
        !self.closed
            && self.vertices.len() >= 3
            && frame.to_screen(self.position(0)).distance(pointer) <= GRAB_RADIUS
    }
}

/// A pen press on a path still being drawn: closes it on the first vertex, grabs
/// another vertex, or adds one whose handle a drag pulls out.
pub(super) fn press(
    mask: &mut Mask,
    shape: &PathGeometry,
    frame: &PictureFrame,
    pointer: Pos2,
    time: Ticks,
) -> Option<Grip> {
    if shape.closes_at(frame, pointer) {
        mask.closed = true;
        return None;
    }
    if let Some(vertex) = shape.vertex_near(frame, pointer) {
        return Some(Grip::Vertex(vertex));
    }
    key_path(&mut mask.path, time);
    let position = to_fraction(frame.to_picture(pointer), shape.fitted);
    let mut vertex = PathVertex::new(position, [0.0; 2]);
    if timeline::path_is_animated(&mask.path) {
        for param in vertex.params_mut() {
            param.toggle_animation(time);
        }
    }
    mask.path.push(vertex);
    Some(Grip::Handle(mask.path.len() - 1, 1.0))
}

/// A modified click on a closed path: Ctrl deletes a vertex or adds one on a side, and
/// Alt turns a smooth vertex into a corner. Returns whether the path changed.
pub(super) fn click(
    mask: &mut Mask,
    shape: &PathGeometry,
    frame: &PictureFrame,
    pointer: Pos2,
    modifiers: Modifiers,
    time: Ticks,
) -> bool {
    let vertex = shape.vertex_near(frame, pointer);
    if modifiers.command {
        if let Some(vertex) = vertex {
            if mask.path.len() <= 3 {
                return false;
            }
            mask.path.remove(vertex);
            return true;
        }
        if let Some((side, along)) = shape.side_near(frame, pointer) {
            split_path_side(&mut mask.path, side, along);
            return true;
        }
    } else if let (true, Some(vertex)) = (modifiers.alt, vertex) {
        if shape.vertices[vertex].handle == [0.0; 2] {
            return false;
        }
        key_path(&mut mask.path, time);
        for param in &mut mask.path[vertex].handle {
            param.set_at(time, 0.0);
        }
        return true;
    }
    false
}

/// Moves what `grip` holds of a path to `to`, in picture pixels.
pub(super) fn drag(mask: &mut Mask, shape: &PathGeometry, grip: Grip, to: Vec2, time: Ticks) {
    if let Grip::Vertex(index) | Grip::Handle(index, _) = grip {
        if index >= mask.path.len() {
            return;
        }
    }
    key_path(&mut mask.path, time);
    let fitted = shape.fitted;
    let set = |params: &mut [timeline::Param; 2], value: [f32; 2]| {
        for (param, value) in params.iter_mut().zip(value) {
            param.set_at(time, value * 100.0);
        }
    };
    match grip {
        Grip::Vertex(index) => set(&mut mask.path[index].position, to_fraction(to, fitted)),
        Grip::Handle(index, sign) => {
            // The vertex may be newer than `shape`, placed by this press.
            let position = to_picture(mask.path[index].at(time).position, fitted);
            let handle = (to - position) * sign / fitted;
            set(&mut mask.path[index].handle, handle.into());
        }
        Grip::Body(offset) => {
            let Some(anchor) = shape.anchor() else {
                return;
            };
            let shift = (to - offset - anchor) / fitted;
            for (vertex, shown) in mask.path.iter_mut().zip(&shape.vertices) {
                let moved = Vec2::from(shown.position) + shift;
                set(&mut vertex.position, moved.into());
            }
        }
        Grip::Side(_) => {}
    }
}

fn to_picture(fraction: [f32; 2], fitted: Vec2) -> Vec2 {
    (Vec2::from(fraction) - Vec2::splat(0.5)) * fitted
}

fn to_fraction(picture: Vec2, fitted: Vec2) -> [f32; 2] {
    (picture / fitted + Vec2::splat(0.5)).into()
}
