use serde::{Deserialize, Serialize};

use crate::{Keyframe, Param, Ticks};

/// A point of a Bezier mask's outline.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct PathVertex {
    /// Percent of the picture's width and height from its top left.
    pub position: [Param; 2],
    /// In the same percent: the curve leaves the vertex along this and arrives along its
    /// reverse, so zero makes a corner.
    pub handle: [Param; 2],
}

/// A vertex's position and handle at one time, as fractions of the picture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VertexAt {
    pub position: [f32; 2],
    pub handle: [f32; 2],
}

impl PathVertex {
    pub fn new(position: [f32; 2], handle: [f32; 2]) -> Self {
        let percent = |value: f32| Param::from_value(value * 100.0);
        Self {
            position: position.map(percent),
            handle: handle.map(percent),
        }
    }

    pub fn at(&self, time: Ticks) -> VertexAt {
        let fraction = |param: &Param| param.value_at(time) / 100.0;
        VertexAt {
            position: self.position.each_ref().map(fraction),
            handle: self.handle.each_ref().map(fraction),
        }
    }

    pub fn params(&self) -> impl Iterator<Item = &Param> {
        self.position.iter().chain(&self.handle)
    }

    pub fn params_mut(&mut self) -> impl Iterator<Item = &mut Param> {
        self.position.iter_mut().chain(&mut self.handle)
    }
}

impl VertexAt {
    fn is_corner(&self) -> bool {
        self.handle == [0.0; 2]
    }
}

/// The closed outline through `vertices` as at most `budget` points, the points left
/// over after the vertices shared out among the curved sides.
pub fn flatten_path(vertices: &[VertexAt], budget: usize) -> Vec<[f32; 2]> {
    let vertices = &vertices[..vertices.len().min(budget)];
    let count = vertices.len();
    let sides = || (0..count).map(|index| (vertices[index], vertices[(index + 1) % count]));
    let curved = sides()
        .filter(|(from, to)| !from.is_corner() || !to.is_corner())
        .count();
    let steps = (budget - count).checked_div(curved).unwrap_or(0);
    let mut points = Vec::with_capacity(budget);
    for (from, to) in sides() {
        points.push(from.position);
        if from.is_corner() && to.is_corner() {
            continue;
        }
        points.extend((1..=steps).map(|step| {
            let along = step as f32 / (steps + 1) as f32;
            path_side_point(from, to, along)
        }));
    }
    points
}

/// The point `along` (0 to 1) the cubic Bezier from `from` to `to`.
pub fn path_side_point(from: VertexAt, to: VertexAt, along: f32) -> [f32; 2] {
    let rest = 1.0 - along;
    let weights = [
        rest * rest * rest,
        3.0 * rest * rest * along,
        3.0 * rest * along * along,
        along * along * along,
    ];
    std::array::from_fn(|axis| {
        let controls = [
            from.position[axis],
            from.position[axis] + from.handle[axis],
            to.position[axis] - to.handle[axis],
            to.position[axis],
        ];
        weights.iter().zip(controls).map(|(w, c)| w * c).sum()
    })
}

/// The handle of a vertex placed `along` a side, so the outline keeps roughly its shape.
fn side_handle(from: VertexAt, to: VertexAt, along: f32) -> [f32; 2] {
    if from.is_corner() && to.is_corner() {
        return [0.0; 2];
    }
    // A sixth of the tangent: a third, for the halves either side of the new vertex.
    let delta = 1e-3;
    let [before, after] = [along - delta, along + delta].map(|t| path_side_point(from, to, t));
    std::array::from_fn(|axis| (after[axis] - before[axis]) / (2.0 * delta) / 6.0)
}

/// Adds a vertex `along` the side leaving `path[side]`. On an animated path it is keyed
/// wherever the path is, on that side's curve as it is then.
pub fn split_path_side(path: &mut Vec<PathVertex>, side: usize, along: f32) {
    let count = path.len();
    let next = (side + 1) % count;
    let vertex_at = |time: Ticks| {
        let (from, to) = (path[side].at(time), path[next].at(time));
        VertexAt {
            position: path_side_point(from, to, along),
            handle: side_handle(from, to, along),
        }
    };
    let mut times: Vec<Ticks> = path
        .iter()
        .flat_map(PathVertex::params)
        .flat_map(|param| param.keys.iter().map(|key| key.time))
        .collect();
    times.sort_unstable();
    times.dedup();
    let still = vertex_at(Ticks::ZERO);
    let mut vertex = PathVertex::new(still.position, still.handle);
    for &time in &times {
        let shape = vertex_at(time);
        let values = shape.position.into_iter().chain(shape.handle);
        for (param, value) in vertex.params_mut().zip(values) {
            let hundredths = Param::from_value(value * 100.0).hundredths;
            param.keys.push(Keyframe::new(time, hundredths));
        }
    }
    path.insert(side + 1, vertex);
}

/// Keys every vertex of an animated path at `time`, so changing one there changes the
/// path's shape at that time rather than one coordinate's.
pub fn key_path(path: &mut [PathVertex], time: Ticks) {
    if !path_is_animated(path) {
        return;
    }
    for param in path.iter_mut().flat_map(PathVertex::params_mut) {
        if !param.has_key_at(time) {
            param.toggle_key(time);
        }
    }
}

pub fn path_is_animated(path: &[PathVertex]) -> bool {
    path.iter()
        .flat_map(PathVertex::params)
        .any(Param::is_animated)
}

/// Starts animating the whole path with a keyframe at `time`, or stops it there.
pub fn toggle_path_animation(path: &mut [PathVertex], time: Ticks) {
    let animated = path_is_animated(path);
    for param in path.iter_mut().flat_map(PathVertex::params_mut) {
        if param.is_animated() == animated {
            param.toggle_animation(time);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corner(x: f32, y: f32) -> VertexAt {
        VertexAt {
            position: [x, y],
            handle: [0.0; 2],
        }
    }

    #[test]
    fn a_curved_path_shares_its_spare_points_among_its_curved_sides() {
        let mut square = [
            corner(0.0, 0.0),
            corner(1.0, 0.0),
            corner(1.0, 1.0),
            corner(0.0, 1.0),
        ];
        assert_eq!(flatten_path(&square, 16).len(), 4);
        square[1].handle = [0.0, 0.2];
        let outline = flatten_path(&square, 16);
        // Two sides meet the smooth vertex: twelve spare points, six on each.
        assert_eq!(outline.len(), 16);
        assert_eq!(outline[7], [1.0, 0.0]);
        // The side into the smooth vertex arrives from above, bowing out past the top.
        assert!(outline[4][1] < 0.0);
    }

    #[test]
    fn a_vertex_added_to_an_animated_path_follows_the_side_at_every_key() {
        let mut path: Vec<PathVertex> = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]
            .into_iter()
            .map(|position| PathVertex::new(position, [0.0; 2]))
            .collect();
        toggle_path_animation(&mut path, Ticks(0));
        key_path(&mut path, Ticks(0));
        path[1].position[0].set_at(Ticks(100), 50.0);
        key_path(&mut path, Ticks(100));
        split_path_side(&mut path, 0, 0.5);
        assert_eq!(path.len(), 4);
        assert_eq!(path[1].at(Ticks(0)).position, [0.5, 0.0]);
        assert_eq!(path[1].at(Ticks(100)).position, [0.25, 0.0]);
    }
}
