use media::VideoFrame;

use super::Flow;

/// Motion is searched on pictures no wider than this; finer detail costs time without
/// moving the vectors much.
const WORKING_WIDTH: usize = 480;
/// Side of the square blocks matched, in pixels of the level searched.
const BLOCK: usize = 8;
/// The coarsest level is searched this far each way; finer ones only refine.
const COARSE_REACH: i32 = 6;
const FINE_REACH: i32 = 2;
/// A level is halved again while wider than this.
const COARSEST_WIDTH: usize = 60;
/// How much each pixel of difference from the coarser level's guess costs, per pixel
/// of the block. Keeps flat areas, where every vector matches, from wandering.
const STRAY_COST: u32 = 2;

/// Brightness at a reduced size, one byte a pixel.
struct Luma {
    width: usize,
    height: usize,
    values: Vec<u8>,
}

impl Luma {
    /// `frame` shrunk by a whole `factor`, averaging each square of pixels.
    fn of(frame: &VideoFrame, factor: usize) -> Self {
        let (full_width, full_height) = (frame.width as usize, frame.height as usize);
        let (width, height) = (
            full_width.div_ceil(factor).max(1),
            full_height.div_ceil(factor).max(1),
        );
        let mut sums = vec![(0u32, 0u32); width * height];
        let (pixels, _) = frame.pixels.as_chunks::<4>();
        for (at, [r, g, b, _]) in pixels.iter().enumerate().take(full_width * full_height) {
            let (x, y) = (at % full_width / factor, at / full_width / factor);
            let luma = u32::from(*r) * 54 + u32::from(*g) * 183 + u32::from(*b) * 19;
            let cell = &mut sums[y * width + x];
            *cell = (cell.0 + (luma >> 8), cell.1 + 1);
        }
        let values = sums
            .into_iter()
            .map(|(sum, count)| (sum / count.max(1)) as u8)
            .collect();
        Self {
            width,
            height,
            values,
        }
    }

    fn halved(&self) -> Self {
        let (width, height) = (self.width.div_ceil(2), self.height.div_ceil(2));
        let values = (0..width * height)
            .map(|at| {
                let (x, y) = (at % width * 2, at / width * 2);
                let sum: u32 = [(0, 0), (1, 0), (0, 1), (1, 1)]
                    .iter()
                    .map(|&(dx, dy)| u32::from(self.at(x as i32 + dx, y as i32 + dy)))
                    .sum();
                (sum / 4) as u8
            })
            .collect();
        Self {
            width,
            height,
            values,
        }
    }

    /// The value at `(x, y)`, edges repeating outward.
    fn at(&self, x: i32, y: i32) -> u8 {
        let x = x.clamp(0, self.width as i32 - 1) as usize;
        let y = y.clamp(0, self.height as i32 - 1) as usize;
        self.values[y * self.width + x]
    }
}

/// Where each block of `earlier` has moved to in `later`.
pub(super) fn estimate(earlier: &VideoFrame, later: &VideoFrame) -> Flow {
    let factor = (earlier.width as usize).div_ceil(WORKING_WIDTH).max(1);
    let mut levels = vec![(Luma::of(earlier, factor), Luma::of(later, factor))];
    while let Some((from, to)) = levels
        .last()
        .filter(|(from, _)| from.width > COARSEST_WIDTH)
    {
        let halved = (from.halved(), to.halved());
        levels.push(halved);
    }
    let mut vectors: Option<Grid> = None;
    for (depth, (from, to)) in levels.iter().enumerate().rev() {
        let reach = if depth == levels.len() - 1 {
            COARSE_REACH
        } else {
            FINE_REACH
        };
        vectors = Some(search(from, to, vectors.as_ref(), reach));
    }
    // Smoothing coarse levels would wipe out anything only a block or two across.
    let grid = vectors.map_or_else(|| Grid::still(1, 1), |grid| smoothed(&grid));
    let scale = factor as f32;
    Flow {
        columns: grid.columns,
        rows: grid.rows,
        step: (BLOCK * factor) as f32,
        vectors: grid
            .vectors
            .into_iter()
            .map(|[x, y]| [x * scale, y * scale])
            .collect(),
    }
}

/// Block motion in pixels of one level.
struct Grid {
    columns: usize,
    rows: usize,
    vectors: Vec<[f32; 2]>,
}

impl Grid {
    fn still(columns: usize, rows: usize) -> Self {
        Self {
            columns,
            rows,
            vectors: vec![[0.0; 2]; columns * rows],
        }
    }

    fn at(&self, column: usize, row: usize) -> [f32; 2] {
        let column = column.min(self.columns - 1);
        let row = row.min(self.rows - 1);
        self.vectors[row * self.columns + column]
    }
}

/// Each block's best match within `reach` of the coarser level's guess, doubled to
/// this level's pixels.
fn search(from: &Luma, to: &Luma, coarser: Option<&Grid>, reach: i32) -> Grid {
    let (columns, rows) = (from.width.div_ceil(BLOCK), from.height.div_ceil(BLOCK));
    let mut grid = Grid::still(columns, rows);
    for (at, vector) in grid.vectors.iter_mut().enumerate() {
        let (column, row) = (at % columns, at / columns);
        let guess = coarser.map_or([0, 0], |coarser| {
            coarser
                .at(column / 2, row / 2)
                .map(|part| (part * 2.0).round() as i32)
        });
        let origin = [(column * BLOCK) as i32, (row * BLOCK) as i32];
        let cost = |dx: i32, dy: i32| {
            let stray = (dx.abs_diff(guess[0]) + dy.abs_diff(guess[1])) * STRAY_COST;
            block_difference(from, to, origin, [dx, dy]) + stray * (BLOCK * BLOCK) as u32 / 8
        };
        let candidates = (-reach..=reach)
            .flat_map(|dy| (-reach..=reach).map(move |dx| (guess[0] + dx, guess[1] + dy)));
        let best = candidates
            .chain([(0, 0)])
            .min_by_key(|&(dx, dy)| cost(dx, dy))
            .unwrap_or((0, 0));
        // A parabola through the neighbours finds the low point between whole pixels.
        let refine = |below: u32, here: u32, above: u32| {
            let curve = below as f32 - 2.0 * here as f32 + above as f32;
            if curve > 0.0 {
                ((below as f32 - above as f32) / (2.0 * curve)).clamp(-0.5, 0.5)
            } else {
                0.0
            }
        };
        let here = cost(best.0, best.1);
        // An exact match is exact; uneven neighbours would only nudge it off.
        if block_difference(from, to, origin, [best.0, best.1]) == 0 {
            *vector = [best.0 as f32, best.1 as f32];
            continue;
        }
        *vector = [
            best.0 as f32 + refine(cost(best.0 - 1, best.1), here, cost(best.0 + 1, best.1)),
            best.1 as f32 + refine(cost(best.0, best.1 - 1), here, cost(best.0, best.1 + 1)),
        ];
    }
    grid
}

fn block_difference(from: &Luma, to: &Luma, [x, y]: [i32; 2], [dx, dy]: [i32; 2]) -> u32 {
    let mut sum = 0;
    for row in y..y + BLOCK as i32 {
        for column in x..x + BLOCK as i32 {
            let a = from.at(column, row);
            let b = to.at(column + dx, row + dy);
            sum += u32::from(a.abs_diff(b));
        }
    }
    sum
}

/// Each vector replaced by the median of it and its neighbours, which drops lone
/// mismatches.
fn smoothed(grid: &Grid) -> Grid {
    let mut out = Grid::still(grid.columns, grid.rows);
    for (at, vector) in out.vectors.iter_mut().enumerate() {
        let (column, row) = (at % grid.columns, at / grid.columns);
        let around: Vec<[f32; 2]> = (0..3)
            .flat_map(|dy| (0..3).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| {
                grid.at(
                    (column + dx).saturating_sub(1),
                    (row + dy).saturating_sub(1),
                )
            })
            .collect();
        *vector = [0, 1].map(|axis| {
            let mut parts: Vec<f32> = around.iter().map(|vector| vector[axis]).collect();
            parts.sort_unstable_by(f32::total_cmp);
            parts[parts.len() / 2]
        });
    }
    out
}
